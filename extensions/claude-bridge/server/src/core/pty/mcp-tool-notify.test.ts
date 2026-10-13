// In-memory HTTP routing only: no Bridge listener or Claude process is started.
// Covers the server side of INC-2026-0042 catalog delivery: capability,
// GET SSE stream auth/ownership, list_changed on a semantic catalog change,
// single-stream replacement, pending delivery and teardown cleanup.
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { Hono } from 'hono'
import type { PtySession, ToolDefinition } from './types'

const mocks = vi.hoisted(() => ({ sessions: new Map<string, unknown>(), emit: vi.fn() }))
vi.mock('./session-manager', () => ({ getSession: (id: string) => mocks.sessions.get(id), sendMcpCall: vi.fn() }))
vi.mock('./session-ws', () => ({ getSessionWs: vi.fn() }))
vi.mock('./session-io', () => ({ sendToClient: vi.fn() }))
vi.mock('../events/bus', () => ({ eventBus: { emit: mocks.emit } }))
vi.mock('../logging', () => ({ debugLog: vi.fn(), warnLog: vi.fn(), errorLog: vi.fn() }))

import { handleMcpGet, handleMcpRequest } from './mcp-http-handler'
import {
  TOOLS_LIST_CHANGED,
  closeToolNotificationStream,
  hasToolNotificationStream,
  replaceRegisteredTools,
  toolCatalogFingerprint,
} from './mcp-tool-notify'

const app = new Hono().post('/mcp/:sessionId', handleMcpRequest).get('/mcp/:sessionId', handleMcpGet)
const decoder = new TextDecoder()

function makeSession(id: string): PtySession {
  const session = {
    id,
    mcpToken: `token-${id}`,
    mcpInitialized: false,
    mcpCallCount: 0,
    mcpLastError: null,
    mcpLog: [],
    lastActivityAt: new Date(0),
    registeredTools: [],
    registeredResources: [],
    registeredResourceTemplates: [],
    registeredPrompts: [],
  } as unknown as PtySession
  mocks.sessions.set(id, session)
  return session
}

function tool(
  name: string,
  description = `${name} tool`,
  inputSchema: Record<string, unknown> = { type: 'object' },
): ToolDefinition {
  return { name, description, inputSchema }
}

function openStream(sessionId: string, token = `token-${sessionId}`, accept = 'text/event-stream') {
  return app.request(`/mcp/${sessionId}`, {
    method: 'GET',
    headers: { accept, authorization: `Bearer ${token}`, 'mcp-session-id': sessionId },
  })
}

/** SSE reader that keeps one outstanding read() across timeouts, so a chunk
 *  arriving after a timed-out wait is not lost to an abandoned promise. */
function sse(res: Response) {
  const reader = res.body!.getReader()
  let inflight: Promise<ReadableStreamReadResult<Uint8Array>> | null = null
  return {
    async readUntil(predicate: (text: string) => boolean, ms = 500): Promise<{ text: string; done: boolean }> {
      let text = ''
      const deadline = Date.now() + ms
      while (Date.now() < deadline) {
        inflight ??= reader.read()
        const chunk = await Promise.race([
          inflight,
          new Promise<null>((resolve) => setTimeout(() => resolve(null), Math.max(1, deadline - Date.now()))),
        ])
        if (chunk === null) break
        inflight = null
        if (chunk.done) return { text, done: true }
        text += decoder.decode(chunk.value)
        if (predicate(text)) break
      }
      return { text, done: false }
    },
    cancel: () => reader.cancel(),
  }
}

const listChangedCount = (text: string) => text.split(TOOLS_LIST_CHANGED).length - 1

beforeEach(() => {
  mocks.sessions.clear()
})

afterEach(() => {
  for (const id of ['s1', 's2']) closeToolNotificationStream(id)
})

describe('MCP tools/list_changed delivery', () => {
  it('advertises tools.listChanged only for tools', async () => {
    makeSession('s1')
    const res = await app.request('/mcp/s1', {
      method: 'POST',
      headers: { 'content-type': 'application/json', authorization: 'Bearer token-s1' },
      body: JSON.stringify({ jsonrpc: '2.0', id: 1, method: 'initialize', params: { protocolVersion: '2025-06-18' } }),
    })
    const body = (await res.json()) as { result: { capabilities: Record<string, Record<string, unknown>> } }
    expect(body.result.capabilities.tools).toEqual({ listChanged: true })
    expect(body.result.capabilities.resources).toEqual({})
    expect(body.result.capabilities.prompts).toEqual({})
  })

  it('guards the GET stream like POST: auth, ownership, Accept, teardown', async () => {
    const session = makeSession('s1')
    expect((await openStream('s1', 'wrong')).status).toBe(401)
    expect((await openStream('missing')).status).toBe(404)
    expect((await openStream('s1', 'token-s1', 'application/json')).status).toBe(405)
    session.teardown = { startedAt: 0, timer: null, finalized: false }
    expect((await openStream('s1')).status).toBe(409)
    expect(hasToolNotificationStream('s1')).toBe(false)
  })

  it('pushes list_changed on a semantic catalog change and nothing otherwise', async () => {
    const session = makeSession('s1')
    session.mcpInitialized = true
    replaceRegisteredTools(session, [tool('a')], 'seed') // no stream yet → pending
    const res = await openStream('s1')
    expect(res.status).toBe(200)
    expect(res.headers.get('content-type')).toBe('text/event-stream')
    const reader = sse(res)
    // The pending change made before the stream opened is flushed first.
    expect(listChangedCount((await reader.readUntil((t) => t.includes(TOOLS_LIST_CHANGED))).text)).toBe(1)

    // Same catalog in a different order → no notification.
    replaceRegisteredTools(session, [tool('a')], 'same')
    expect(listChangedCount((await reader.readUntil(() => false, 80)).text)).toBe(0)

    // Same count, changed schema → notification (array length is not identity).
    expect(replaceRegisteredTools(session, [tool('a', 'a tool', { type: 'object', required: ['x'] })], 'schema')).toBe(
      true,
    )
    const changed = await reader.readUntil((t) => t.includes(TOOLS_LIST_CHANGED))
    expect(listChangedCount(changed.text)).toBe(1)
    expect(changed.text).toContain('"jsonrpc":"2.0"')
    expect(changed.text).not.toContain('"id"')

    // Removal to an empty snapshot is a change too.
    replaceRegisteredTools(session, [], 'removed')
    expect(listChangedCount((await reader.readUntil((t) => t.includes(TOOLS_LIST_CHANGED))).text)).toBe(1)
    await reader.cancel()
  })

  it('does not notify before the CLI initialized MCP', async () => {
    const session = makeSession('s1')
    const reader = sse(await openStream('s1'))
    expect(replaceRegisteredTools(session, [tool('a')], 'startup')).toBe(true)
    expect(listChangedCount((await reader.readUntil(() => false, 80)).text)).toBe(0)
    await reader.cancel()
  })

  it('keeps one stream per session and never delivers across sessions', async () => {
    const s1 = makeSession('s1')
    const s2 = makeSession('s2')
    s1.mcpInitialized = true
    s2.mcpInitialized = true
    const first = sse(await openStream('s1'))
    const second = sse(await openStream('s1'))
    const other = sse(await openStream('s2'))

    // The replaced stream is closed by the server.
    expect((await first.readUntil(() => false, 200)).done).toBe(true)

    replaceRegisteredTools(s1, [tool('only-s1')], 'change')
    expect(listChangedCount((await second.readUntil((t) => t.includes(TOOLS_LIST_CHANGED))).text)).toBe(1)
    expect(listChangedCount((await other.readUntil(() => false, 80)).text)).toBe(0)
    await second.cancel()
    await other.cancel()
  })

  it('teardown closes the stream and drops pending state', async () => {
    const session = makeSession('s1')
    session.mcpInitialized = true
    const reader = sse(await openStream('s1'))
    closeToolNotificationStream('s1')
    expect((await reader.readUntil(() => false, 200)).done).toBe(true)
    expect(hasToolNotificationStream('s1')).toBe(false)

    replaceRegisteredTools(session, [tool('late')], 'after-close') // pending
    closeToolNotificationStream('s1') // teardown forgets it
    const fresh = sse(await openStream('s1'))
    expect(listChangedCount((await fresh.readUntil(() => false, 80)).text)).toBe(0)
    await fresh.cancel()
  })

  it('fingerprint ignores order but sees descriptions and schemas', () => {
    expect(toolCatalogFingerprint([tool('a'), tool('b')])).toBe(toolCatalogFingerprint([tool('b'), tool('a')]))
    expect(toolCatalogFingerprint([tool('a', 'x')])).not.toBe(toolCatalogFingerprint([tool('a', 'y')]))
  })
})

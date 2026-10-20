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
import { warnLog } from '../logging'
import {
  KEEPALIVE_MS,
  MAX_PENDING_SESSIONS,
  STALLED_KEEPALIVE_TICKS,
  STREAM_QUEUE_FRAMES,
  TOOLS_LIST_CHANGED,
  closeToolNotificationStream,
  hasPendingToolNotification,
  hasToolNotificationStream,
  notifyToolListChanged,
  pendingToolNotificationCount,
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

function openStream(
  sessionId: string,
  token = `token-${sessionId}`,
  accept = 'text/event-stream',
  signal?: AbortSignal,
) {
  return app.request(`/mcp/${sessionId}`, {
    method: 'GET',
    headers: { accept, authorization: `Bearer ${token}`, 'mcp-session-id': sessionId },
    signal,
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
  vi.useRealTimers()
  for (const id of ['s1', 's2']) closeToolNotificationStream(id)
  for (let i = 0; i <= MAX_PENDING_SESSIONS; i++) closeToolNotificationStream(`p${i}`)
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

  it('bounds the queue of a reader that does not read: changes coalesce', async () => {
    makeSession('s1')
    const res = await openStream('s1')
    // 10 000 changes while nothing is read: only the queue's free slots are
    // written, everything else collapses into one owed notification.
    const results = { sent: 0, coalesced: 0, pending: 0, dropped: 0 }
    for (let i = 0; i < 10_000; i++) results[notifyToolListChanged('s1')] += 1
    expect(results.sent).toBe(STREAM_QUEUE_FRAMES - 1) // ': open' holds one slot
    expect(results.coalesced).toBe(10_000 - results.sent)
    expect(results.pending + results.dropped).toBe(0)

    // Draining delivers the queued frames plus the single coalesced one.
    const reader = sse(res)
    const text = (await reader.readUntil((t) => listChangedCount(t) >= STREAM_QUEUE_FRAMES, 500)).text
    expect(listChangedCount(text)).toBe(STREAM_QUEUE_FRAMES)
    expect(text.length).toBeLessThan(1_000)
    // Nothing else was owed.
    expect(listChangedCount((await reader.readUntil(() => false, 80)).text)).toBe(0)
    await reader.cancel()
  })

  it('closes a stalled reader with a warning and keeps its unsent change pending', async () => {
    vi.useFakeTimers({ toFake: ['setInterval', 'clearInterval'] })
    makeSession('s1')
    await openStream('s1') // never read
    for (let i = 0; i < STREAM_QUEUE_FRAMES; i++) notifyToolListChanged('s1') // fills the queue, last one coalesces
    vi.mocked(warnLog).mockClear()
    vi.advanceTimersByTime(KEEPALIVE_MS * (STALLED_KEEPALIVE_TICKS - 1))
    expect(hasToolNotificationStream('s1')).toBe(true)
    vi.advanceTimersByTime(KEEPALIVE_MS)
    expect(hasToolNotificationStream('s1')).toBe(false)
    expect(warnLog).toHaveBeenCalledWith('MCP notification stream stalled — closing', expect.anything())
    expect(hasPendingToolNotification('s1')).toBe(true)
    vi.useRealTimers()

    // The next stream receives exactly the one owed notification.
    const fresh = sse(await openStream('s1'))
    expect(listChangedCount((await fresh.readUntil((t) => t.includes(TOOLS_LIST_CHANGED))).text)).toBe(1)
    expect(hasPendingToolNotification('s1')).toBe(false)
    await fresh.cancel()
  })

  it('ends the stream when the request is aborted', async () => {
    makeSession('s1')
    const abort = new AbortController()
    const res = await openStream('s1', 'token-s1', 'text/event-stream', abort.signal)
    expect(res.status).toBe(200)
    expect(hasToolNotificationStream('s1')).toBe(true)
    abort.abort()
    await new Promise((r) => setTimeout(r, 10))
    expect(hasToolNotificationStream('s1')).toBe(false)
  })

  it('reader cancel keeps an owed change for the next stream; teardown forgets it', async () => {
    makeSession('s1')
    const res = await openStream('s1')
    for (let i = 0; i < STREAM_QUEUE_FRAMES; i++) notifyToolListChanged('s1') // last one is owed
    await res.body!.cancel()
    expect(hasToolNotificationStream('s1')).toBe(false)
    expect(hasPendingToolNotification('s1')).toBe(true)
    closeToolNotificationStream('s1')
    expect(hasPendingToolNotification('s1')).toBe(false)
  })

  it('caps sessions holding a pending flag and logs the drop', () => {
    const before = pendingToolNotificationCount()
    for (let i = before; i < MAX_PENDING_SESSIONS; i++) expect(notifyToolListChanged(`p${i}`)).toBe('pending')
    // Repeated changes of the same session stay one flag.
    expect(notifyToolListChanged('p0')).toBe('pending')
    expect(pendingToolNotificationCount()).toBe(MAX_PENDING_SESSIONS)
    vi.mocked(warnLog).mockClear()
    expect(notifyToolListChanged(`p${MAX_PENDING_SESSIONS}`)).toBe('dropped')
    expect(warnLog).toHaveBeenCalledWith('MCP tools/list_changed dropped: pending cap reached', expect.anything())
    expect(pendingToolNotificationCount()).toBe(MAX_PENDING_SESSIONS)
  })

  it('fingerprint ignores order but sees descriptions and schemas', () => {
    expect(toolCatalogFingerprint([tool('a'), tool('b')])).toBe(toolCatalogFingerprint([tool('b'), tool('a')]))
    expect(toolCatalogFingerprint([tool('a', 'x')])).not.toBe(toolCatalogFingerprint([tool('a', 'y')]))
  })
})

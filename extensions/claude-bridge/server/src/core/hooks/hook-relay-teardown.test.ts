// Route-level half of the teardown window: which relay calls an exiting
// session still answers, and what closes the window.
import 'reflect-metadata'

import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

vi.mock('./dispatcher', async (importOriginal) => ({
  ...(await importOriginal<typeof import('./dispatcher')>()),
  dispatchHook: vi.fn(async () => ({
    stdout: '',
    stderr: '',
    exitCode: 0,
    outcome: 'success' as const,
    durationMs: 1,
  })),
}))

import { destroySession, sendMcpCall, sessions } from '../pty/session-core'
import { handleMcpResponse, pendingMcpCalls, rejectAllPending } from '../pty/session-mcp-call'
import type { PtySession } from '../types/pty'
import { clearSession, registerSessionHooks } from './registry'
import { createHooksRoutes } from './routes'
import type { HookSettings } from './types'

const HOOKS: HookSettings = {
  SessionEnd: [{ hooks: [{ type: 'command', command: 'notify' }] }],
  PreToolUse: [{ matcher: 'Bash', hooks: [{ type: 'command', command: 'guard' }] }],
}

function seedSession(id: string): { session: PtySession; hookIdFor: (event: string) => string } {
  const session = {
    id,
    pty: { kill: vi.fn(), pid: 1 },
    ws: { readyState: 1, send: vi.fn() },
    userName: 'tester',
    tokenId: 'token-1',
    settingsDir: '',
    cwd: '/work',
    state: 'running',
    createdAt: new Date(),
    lastActivityAt: new Date(),
    mcpCallCount: 0,
    inputCount: 0,
    userMessages: 0,
    assistantMessages: 0,
    contextTokens: 0,
    totalTokens: 0,
    mcpLog: [],
    mcpInitialized: true,
    mcpLastError: null,
    mcpToken: 'mcp-secret',
    outputBuffer: [],
    outputBufferBytes: 0,
    lastResizeAt: 0,
    consoleShrunk: false,
    registeredTools: [],
    registeredResources: [],
    registeredResourceTemplates: [],
    registeredPrompts: [],
    cliConversationId: null,
    childSessions: [],
  } as unknown as PtySession
  sessions.set(id, session)
  const registered = registerSessionHooks(id, HOOKS, { kind: 'user' })
  const byEvent = new Map<string, string>()
  for (const [handler, reg] of registered) {
    void handler
    byEvent.set(reg.event, reg.id)
  }
  return { session, hookIdFor: (event: string) => byEvent.get(event) ?? '' }
}

function relayRequest(id: string, event: string, hookId: string, token = 'mcp-secret'): Request {
  return new Request(`http://127.0.0.1/api/hooks/${id}/${event}/${hookId}`, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      Authorization: `Bearer ${token}`,
      'X-Bridge-Hook-Relay': 'command',
    },
    body: JSON.stringify({ hook_event_name: event }),
  })
}

describe('hook relay during session teardown', () => {
  const app = createHooksRoutes()

  beforeEach(() => {
    vi.useFakeTimers()
  })

  afterEach(() => {
    rejectAllPending('test cleanup')
    vi.useRealTimers()
    for (const id of [...sessions.keys()]) {
      clearSession(id)
      sessions.delete(id)
    }
  })

  it('answers the exiting session own SessionEnd instead of Unknown session', async () => {
    const { hookIdFor } = seedSession('relay-end')
    destroySession('relay-end')

    const res = await app.request(relayRequest('relay-end', 'SessionEnd', hookIdFor('SessionEnd')))
    expect(res.status).toBe(200)
    // Serving it closes the window: the session is gone right after.
    expect(sessions.has('relay-end')).toBe(false)
  })

  it('refuses every other event once teardown started', async () => {
    const { hookIdFor } = seedSession('relay-refuse')
    destroySession('relay-refuse')

    const res = await app.request(relayRequest('relay-refuse', 'PreToolUse', hookIdFor('PreToolUse')))
    expect(res.status).toBe(409)
    expect(await res.json()).toEqual({ error: 'Session is shutting down' })
    // Refusing must not close the window — SessionEnd may still be coming.
    expect(sessions.has('relay-refuse')).toBe(true)
  })

  it('does not weaken auth for the exiting session', async () => {
    const { hookIdFor } = seedSession('relay-auth')
    destroySession('relay-auth')

    const res = await app.request(
      relayRequest('relay-auth', 'SessionEnd', hookIdFor('SessionEnd'), 'someone-elses-token'),
    )
    expect(res.status).toBe(401)
    expect(sessions.has('relay-auth')).toBe(true)
  })

  it('answers a duplicate SessionEnd with Unknown session rather than running it twice', async () => {
    const { hookIdFor } = seedSession('relay-dup')
    const hookId = hookIdFor('SessionEnd')
    destroySession('relay-dup')

    expect((await app.request(relayRequest('relay-dup', 'SessionEnd', hookId))).status).toBe(200)
    const second = await app.request(relayRequest('relay-dup', 'SessionEnd', hookId))
    expect(second.status).toBe(404)
  })

  it('keeps a running CLI and its in-flight tool calls on SessionEnd from /clear or /resume', async () => {
    // CLI 2.1.x fires SessionEnd(reason: clear|resume) and keeps running. The
    // owner's global SessionEnd hook made this route finalize the live session,
    // so its pending Read/Bash answered `Session destroyed` (INC-2026-0093).
    const { session, hookIdFor } = seedSession('relay-live')
    const inflight = sendMcpCall('relay-live', 'Read', { file_path: '/work/file.txt' })

    const res = await app.request(relayRequest('relay-live', 'SessionEnd', hookIdFor('SessionEnd')))
    expect(res.status).toBe(200)
    expect(sessions.get('relay-live')).toBe(session)

    // The same CLI keeps calling its MCP tools; the live call still settles normally.
    const requestId = [...pendingMcpCalls.entries()].find(([, call]) => call.sessionId === 'relay-live')?.[0]
    expect(requestId).toBeDefined()
    handleMcpResponse(requestId!, 'file body')
    await expect(inflight).resolves.toBe('file body')
    const res2 = await app.request(relayRequest('relay-live', 'PreToolUse', hookIdFor('PreToolUse')))
    expect(res2.status).toBe(200)
  })

  it('stops answering after the window times out', async () => {
    const { hookIdFor } = seedSession('relay-timeout')
    const hookId = hookIdFor('SessionEnd')
    destroySession('relay-timeout')
    vi.advanceTimersByTime(10_000)

    const res = await app.request(relayRequest('relay-timeout', 'SessionEnd', hookId))
    expect(res.status).toBe(404)
  })
})

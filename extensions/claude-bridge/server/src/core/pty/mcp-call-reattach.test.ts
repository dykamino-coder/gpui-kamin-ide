// INC-2026-0093: a tool call delivered to a client socket that then died must
// reach the client that reattached, instead of hanging until the tool timeout.
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

vi.mock('./mcp-tasks', () => ({ mcpTasks: { working: async () => [] } }))

import { acceptCommandReport } from './commands'
import { handleMcpResponse, pendingMcpCalls, rejectAllPending, sendMcpCall } from './session-mcp-call'
import type { PtySession } from './types'

function socket() {
  const frames: Array<{ type: string; requestId?: string }> = []
  return {
    frames,
    calls: () => frames.filter((f) => f.type === 'mcp:call').map((f) => f.requestId),
    ws: {
      readyState: 1,
      bufferedAmount: 0,
      send(payload: string) {
        frames.push(JSON.parse(payload))
      },
    },
  }
}

function session(ws: unknown): PtySession {
  return {
    id: 'reattach-session',
    ws,
    mcpEndpoint: { id: 'owner' },
    mcpCallCount: 0,
    inputCount: 0,
    mcpInitialized: true,
    mcpLastError: null,
    lastActivityAt: new Date(),
  } as unknown as PtySession
}

function onlyRequestId(): string {
  const ids = [...pendingMcpCalls.keys()]
  expect(ids).toHaveLength(1)
  return ids[0]!
}

describe('MCP call delivered to a socket that died before answering', () => {
  beforeEach(() => vi.useFakeTimers())
  afterEach(() => {
    rejectAllPending('test cleanup')
    vi.useRealTimers()
  })

  it('reaches the reattached client and resolves the live CLI call', async () => {
    const dead = socket()
    const target = session(dead.ws)
    const result = sendMcpCall(target, 'Read', { file_path: '/work/a.txt' })
    const requestId = onlyRequestId()
    // The half-open socket accepted the frame; its client never answers.
    expect(dead.calls()).toEqual([requestId])

    const fresh = socket()
    target.ws = fresh.ws as PtySession['ws']
    // The reattached client (restarted host) holds no calls at all.
    await acceptCommandReport(target, { calls: [], shells: [] })
    expect(fresh.calls()).toEqual([requestId])

    handleMcpResponse(requestId, { content: [{ type: 'text', text: 'file body' }] })
    await expect(result).resolves.toEqual({ content: [{ type: 'text', text: 'file body' }] })
  })

  it('does not resend a call the reattached client still executes', async () => {
    const old = socket()
    const target = session(old.ws)
    void sendMcpCall(target, 'Bash', { command: 'npm test' }).catch(() => {})
    const requestId = onlyRequestId()

    const fresh = socket()
    target.ws = fresh.ws as PtySession['ws']
    await acceptCommandReport(target, { calls: [{ id: requestId, output: '' }], shells: [] })
    expect(fresh.calls()).toEqual([])
  })

  it('never resends a call delivered on the current socket', async () => {
    const live = socket()
    const target = session(live.ws)
    void sendMcpCall(target, 'Write', { file_path: '/work/b.txt' }).catch(() => {})
    await acceptCommandReport(target, { calls: [], shells: [] })
    expect(live.calls()).toHaveLength(1)
  })

  it('resends at most once per socket', async () => {
    const target = session(socket().ws)
    void sendMcpCall(target, 'Read', { file_path: '/work/c.txt' }).catch(() => {})
    const fresh = socket()
    target.ws = fresh.ws as PtySession['ws']
    await acceptCommandReport(target, { calls: [], shells: [] })
    await acceptCommandReport(target, { calls: [], shells: [] })
    expect(fresh.calls()).toHaveLength(1)
  })

  it('treats a capped report as incomplete', async () => {
    const target = session(socket().ws)
    void sendMcpCall(target, 'Read', { file_path: '/work/d.txt' }).catch(() => {})
    const fresh = socket()
    target.ws = fresh.ws as PtySession['ws']
    const full = Array.from({ length: 128 }, (_, i) => ({ id: `other-${i}`, output: '' }))
    await acceptCommandReport(target, { calls: full, shells: [] })
    expect(fresh.calls()).toEqual([])
  })
})

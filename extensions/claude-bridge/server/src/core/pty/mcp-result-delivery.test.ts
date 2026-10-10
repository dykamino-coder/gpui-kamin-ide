import { afterEach, describe, expect, it, vi } from 'vitest'
import {
  pendingMcpCalls,
  sendMcpCall,
  settleMcpResult,
  rejectAllPending,
  rejectPendingForSession,
} from './session-mcp-call'
import type { PtySession } from './types'

function call(tool = 'AskUserQuestion') {
  const session = {
    id: 's',
    ws: { readyState: 1, bufferedAmount: 0, send: vi.fn() },
    mcpCallCount: 0,
    lastActivityAt: new Date(),
  } as unknown as PtySession
  const promise = sendMcpCall(session, tool, {})
  const id = [...pendingMcpCalls.keys()][0]!
  return { id, promise }
}
afterEach(() => {
  rejectAllPending('test cleanup')
  vi.useRealTimers()
})

describe('INC-2026-0057 server settlement acknowledgement', () => {
  it('settles once and acknowledges duplicates after a lost ack', async () => {
    const { id, promise } = call()
    const response = { type: 'mcp:response' as const, sessionId: 's', requestId: id, result: 'first' }
    expect(settleMcpResult('s', response)).toBe(true)
    expect(settleMcpResult('s', { ...response, result: 'second' })).toBe(true)
    await expect(promise).resolves.toBe('first')
    expect(pendingMcpCalls.size).toBe(0)
  })
  it('cannot settle another session or accept a mismatched session id', async () => {
    const { id, promise } = call()
    const response = { type: 'mcp:response' as const, sessionId: 's', requestId: id, result: 'first' }
    expect(settleMcpResult('other', response)).toBe(false)
    expect(settleMcpResult('s', { ...response, sessionId: 'other' })).toBe(false)
    expect(pendingMcpCalls.has(id)).toBe(true)
    expect(settleMcpResult('s', response)).toBe(true)
    await promise
    expect(settleMcpResult('other', { ...response, sessionId: 'other' })).toBe(false)
  })
  it('settles error and denial outcomes and acknowledges denial duplicates', async () => {
    const error = call()
    expect(settleMcpResult('s', { type: 'mcp:response', requestId: error.id, result: 'Error: synthetic' })).toBe(true)
    await expect(error.promise).resolves.toBe('Error: synthetic')
    const denied = call()
    const assertion = expect(denied.promise).rejects.toThrow('denied')
    const outcome = { type: 'mcp:denied' as const, requestId: denied.id, reason: 'denied' }
    expect(settleMcpResult('s', outcome)).toBe(true)
    expect(settleMcpResult('s', outcome)).toBe(true)
    await assertion
  })
  it('rejects late results after timeout or session destruction', async () => {
    vi.useFakeTimers()
    const expired = call('Read')
    const timeout = expect(expired.promise).rejects.toThrow('timeout')
    vi.advanceTimersByTime(1_800_000)
    await timeout
    expect(settleMcpResult('s', { type: 'mcp:response', requestId: expired.id, result: 1 })).toBe(false)
    const destroyed = call()
    const cancelled = expect(destroyed.promise).rejects.toThrow('destroyed')
    rejectPendingForSession('s', 'destroyed')
    await cancelled
    expect(settleMcpResult('s', { type: 'mcp:response', requestId: destroyed.id, result: 1 })).toBe(false)
  })
})

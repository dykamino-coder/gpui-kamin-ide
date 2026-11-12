import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import type { McpServerState, TransportContext } from './context'
import { handleServerMessage, type MessageHandlerCtx } from '../server-message-handlers'
import { callStdioTool, stdioJsonRpc } from './stdio'
import { wsJsonRpc } from './ws'
import {
  MCP_ELICITATION_TIMEOUT_MS,
  MCP_REQUEST_TIMEOUT_MS,
  MCP_TOOL_CALL_IDLE_TIMEOUT_MS,
  MCP_TOOL_CALL_MAX_TIMEOUT_MS,
  noteMcpActivity,
} from './request-deadline'

// ws.ts reads OAuth tokens through the VS Code host; requests here need none.
vi.mock('../oauth-store', () => ({ loadTokens: () => null }))

const MIN = 60_000

function fixture(signal?: AbortSignal) {
  const frames: any[] = []
  const state = {
    config: { name: 'picker' },
    process: { stdin: { writable: true, write: (line: string) => frames.push(JSON.parse(line)) } },
    ws: { readyState: 1, send: (raw: string) => frames.push(JSON.parse(raw)) },
    pendingRequests: new Map(),
    nextRequestId: 1,
  } as unknown as McpServerState
  const servers = new Map([['picker', state]])
  const handlerCtx = {
    servers,
    window: { webContents: { send: () => {} } },
    notifyChanged: () => {},
    onToolsChanged: null,
    rpcRequest: async () => ({}),
  } as unknown as MessageHandlerCtx
  const ctx = {
    servers,
    shutdownSignal: signal,
    handleServerMessage: (id: string, msg: any, reply: (r: Record<string, unknown>) => void) =>
      handleServerMessage(handlerCtx, id, msg, reply),
  } as unknown as TransportContext
  const fromServer = (msg: Record<string, unknown>) => ctx.handleServerMessage('picker', { jsonrpc: '2.0', ...msg }, () => {})
  const call = () => frames.find((frame) => frame.method === 'tools/call')
  return { frames, state, ctx, fromServer, call }
}

/** Collects the settled outcome without leaving a rejection unobserved. */
function track<T>(promise: Promise<T>) {
  const outcome: { value?: T; error?: Error } = {}
  promise.then((value) => { outcome.value = value }, (error: Error) => { outcome.error = error })
  return outcome
}

beforeEach(() => vi.useFakeTimers())
afterEach(() => vi.useRealTimers())

describe('external MCP tool-call deadline', () => {
  it('keeps the tool-call deadline longer than the person-facing elicitation window', () => {
    expect(MCP_TOOL_CALL_IDLE_TIMEOUT_MS).toBeGreaterThan(MCP_ELICITATION_TIMEOUT_MS)
    expect(MCP_TOOL_CALL_MAX_TIMEOUT_MS).toBeGreaterThan(MCP_TOOL_CALL_IDLE_TIMEOUT_MS)
  })

  it('waits on a stdio tool call well past the old 120 s limit and delivers the late answer', async () => {
    const { ctx, fromServer, call, state } = fixture()
    const outcome = track(callStdioTool(ctx, 'picker', 'choose_icon', {}))
    await vi.advanceTimersByTimeAsync(MCP_ELICITATION_TIMEOUT_MS)
    expect(outcome).toEqual({})
    fromServer({ id: call().id, result: { content: [{ type: 'text', text: 'star' }] } })
    await vi.advanceTimersByTimeAsync(0)
    expect(outcome.value).toEqual({ content: [{ type: 'text', text: 'star' }] })
    expect(state.pendingRequests.size).toBe(0)
  })

  it('asks for progress and restarts the idle clock on each progress notification', async () => {
    const { ctx, fromServer, call, frames } = fixture()
    const outcome = track(callStdioTool(ctx, 'picker', 'choose_icon', {}))
    const token = call().params._meta.progressToken
    expect(token).toBe(call().id)
    await vi.advanceTimersByTimeAsync(MCP_TOOL_CALL_IDLE_TIMEOUT_MS - MIN)
    fromServer({ method: 'notifications/progress', params: { progressToken: token, progress: 1 } })
    await vi.advanceTimersByTimeAsync(MCP_TOOL_CALL_IDLE_TIMEOUT_MS - MIN)
    expect(outcome).toEqual({})
    await vi.advanceTimersByTimeAsync(MIN)
    expect(outcome.error?.message).toMatch(/tools\/call timeout \(no reply or progress for 15 min\)/)
    expect(frames.at(-1)).toMatchObject({ method: 'notifications/cancelled', params: { requestId: token } })
  })

  it('stops at the total cap even when progress keeps arriving', async () => {
    const { ctx, fromServer, call } = fixture()
    const outcome = track(callStdioTool(ctx, 'picker', 'choose_icon', {}))
    const token = call().params._meta.progressToken
    let elapsed = 0
    while (elapsed + 5 * MIN < MCP_TOOL_CALL_MAX_TIMEOUT_MS) {
      await vi.advanceTimersByTimeAsync(5 * MIN)
      elapsed += 5 * MIN
      fromServer({ method: 'notifications/progress', params: { progressToken: token, progress: elapsed } })
    }
    await vi.advanceTimersByTimeAsync(MCP_TOOL_CALL_MAX_TIMEOUT_MS - elapsed - 1)
    expect(outcome).toEqual({})
    await vi.advanceTimersByTimeAsync(1)
    expect(outcome.error?.message).toMatch(/tools\/call timeout \(30 min total\)/)
  })

  it('restarts the idle clock while the server waits on an elicitation answer', async () => {
    const { ctx, state } = fixture()
    const outcome = track(callStdioTool(ctx, 'picker', 'choose_icon', {}))
    await vi.advanceTimersByTimeAsync(MCP_TOOL_CALL_IDLE_TIMEOUT_MS - MIN)
    noteMcpActivity(state)
    await vi.advanceTimersByTimeAsync(MCP_TOOL_CALL_IDLE_TIMEOUT_MS - MIN)
    expect(outcome).toEqual({})
  })

  it('keeps service methods short and sends them without a progress token', async () => {
    const { ctx, frames } = fixture()
    const outcome = track(stdioJsonRpc(ctx, 'picker', 'tools/list', {}))
    expect(frames[0].params).toEqual({})
    await vi.advanceTimersByTimeAsync(MCP_REQUEST_TIMEOUT_MS - 1)
    expect(outcome).toEqual({})
    await vi.advanceTimersByTimeAsync(1)
    expect(outcome.error?.message).toBe('tools/list timeout (2 min total)')
  })

  it('cancels a pending stdio tool call at once on interrupt and tells the server', async () => {
    const interrupt = new AbortController()
    const { ctx, call, frames, state } = fixture(interrupt.signal)
    const outcome = track(callStdioTool(ctx, 'picker', 'choose_icon', {}))
    interrupt.abort()
    await Promise.resolve()
    await Promise.resolve()
    expect(outcome.error?.message).toBe('MCP request cancelled')
    expect(state.pendingRequests.size).toBe(0)
    expect(frames.at(-1)).toEqual({
      jsonrpc: '2.0',
      method: 'notifications/cancelled',
      params: { requestId: call().id, reason: 'MCP request cancelled' },
    })
    expect(vi.getTimerCount()).toBe(0)
  })

  it('applies the same deadline and cancellation to WebSocket servers', async () => {
    const interrupt = new AbortController()
    const { ctx, call, frames, fromServer } = fixture(interrupt.signal)
    const longCall = track(wsJsonRpc(ctx, 'picker', { jsonrpc: '2.0', id: 7, method: 'tools/call', params: { name: 'x' } }))
    await vi.advanceTimersByTimeAsync(MCP_ELICITATION_TIMEOUT_MS)
    expect(longCall).toEqual({})
    fromServer({ id: call().id, result: { content: [] } })
    await vi.advanceTimersByTimeAsync(0)
    expect(longCall.value).toEqual({ content: [] })

    const cancelled = track(wsJsonRpc(ctx, 'picker', { jsonrpc: '2.0', id: 8, method: 'tools/call', params: { name: 'x' } }))
    interrupt.abort()
    await vi.advanceTimersByTimeAsync(0)
    expect(cancelled.error?.message).toBe('MCP request cancelled')
    expect(frames.at(-1)).toMatchObject({ method: 'notifications/cancelled', params: { requestId: 8 } })
  })
})

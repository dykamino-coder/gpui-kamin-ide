import { afterEach, describe, expect, it, vi } from 'vitest'
import { cancelMcpCall, handleMcpCall, type McpCallCtx } from './mcp-call-pipeline'
import { McpResultOutbox } from './mcp-result-outbox'

const execute = vi.hoisted(() => vi.fn())
vi.mock('../mcp/executor', () => ({ executeTool: execute }))

afterEach(() => vi.clearAllMocks())

describe('INC-2026-0057 result delivery', () => {
  it('retains a completed tool while its response has no server acknowledgement', async () => {
    execute.mockResolvedValue('synthetic result')
    const publish = vi.fn()
    const pending = new Map()
    const ctx = {
      window: { webContents: { send: publish } }, tabId: 'tab-a',
      send: vi.fn(), pendingMcpCalls: pending, pendingPermissions: new Map(),
      permissionMode: () => 'bypassPermissions', permissionManager: {}, denyMcp: vi.fn(),
      deliverResult: () => new Promise<void>(() => {}),
    } as unknown as McpCallCtx
    void handleMcpCall(ctx, { requestId: 'request-a', toolName: 'Write', input: {} })
    await vi.waitFor(() => expect(execute).toHaveBeenCalledOnce())
    await new Promise(resolve => setTimeout(resolve, 0))
    expect(pending.has('request-a')).toBe(true)
    expect(publish.mock.calls.some(call => call[2]?.status === 'completed')).toBe(false)
  })

  it.each(['success', 'error'])('executes %s once across duplicate calls and lost response acknowledgement', async kind => {
    const frames: string[] = []
    const outbox = new McpResultOutbox(frame => { frames.push(frame); return true })
    outbox.attach('s')
    if (kind === 'success') execute.mockResolvedValue('result')
    else execute.mockRejectedValue(new Error('synthetic execution failure'))
    const publish = vi.fn()
    const ctx = {
      window: { webContents: { send: publish } }, tabId: 'tab-a',
      send: vi.fn(), pendingMcpCalls: new Map(), pendingPermissions: new Map(),
      permissionMode: () => 'bypassPermissions', permissionManager: {}, denyMcp: vi.fn(), reportDeliveryFailure: vi.fn(),
      deliverResult: (outcome: any) => outbox.deliver('s', outcome),
    } as unknown as McpCallCtx
    const msg = { requestId: 'r', toolName: 'Write', input: {} }
    const first = handleMcpCall(ctx, msg)
    await vi.waitFor(() => expect(frames).toHaveLength(1))
    await handleMcpCall(ctx, msg)
    outbox.detach()
    outbox.attach('s')
    expect(frames).toHaveLength(2)
    expect(frames[0]).toBe(frames[1])
    expect(execute).toHaveBeenCalledOnce()
    expect(ctx.pendingMcpCalls.has('r')).toBe(true)
    outbox.acknowledge('s', 'r', true)
    await first
    expect(ctx.pendingMcpCalls.size).toBe(0)
    expect(publish.mock.calls.filter(call => call[2]?.status === 'completed')).toHaveLength(1)
    outbox.dispose()
  })
})
describe('task execution cancellation', () => {
  it('aborts only the addressed owner and returns the outcome through the existing delivery channel', async () => {
    execute.mockImplementation(
      (_tool, _input, context) =>
        new Promise((resolve) =>
          context.signal.addEventListener('abort', () => resolve('cancelled'), {
            once: true,
          }),
        ),
    )
    const delivered = vi.fn().mockResolvedValue(undefined)
    const ctx = {
      window: { webContents: { send: vi.fn() } },
      tabId: 'tab',
      send: vi.fn(),
      pendingMcpCalls: new Map(),
      pendingPermissions: new Map(),
      permissionMode: () => 'bypassPermissions',
      permissionManager: {},
      denyMcp: vi.fn(),
      reportDeliveryFailure: vi.fn(),
      deliverResult: delivered,
    } as unknown as McpCallCtx
    const running = handleMcpCall(ctx, {
      requestId: 'task',
      taskOwner: 'owner',
      toolName: 'Bash',
      input: {},
    })
    await vi.waitFor(() => expect(execute).toHaveBeenCalledOnce())
    cancelMcpCall(ctx, 'task', 'other')
    expect(ctx.pendingMcpCalls.get('task')!.controller!.signal.aborted).toBe(false)
    cancelMcpCall(ctx, 'task', 'owner')
    await running
    expect(delivered).toHaveBeenCalledWith({
      type: 'mcp:response',
      requestId: 'task',
      taskOwner: 'owner',
      result: 'cancelled',
    })
    expect(ctx.pendingMcpCalls.size).toBe(0)
  })
})

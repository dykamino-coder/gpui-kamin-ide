import { afterEach, describe, expect, it, vi } from 'vitest'
const execute = vi.hoisted(() => vi.fn())
vi.mock('vscode', () => ({ window: { showWarningMessage: vi.fn() } }))
vi.mock('../mcp/permission-manager', () => ({ PermissionManager: class {} }))
vi.mock('../mcp/executor', () => ({ executeTool: execute }))
import { ConnectionManager } from './connection-manager'
import type { ConnectionConfig } from '../../shared/types'
import type { BrowserWindow } from '@kaminide/host-compat'

const managers: ConnectionManager[] = []
afterEach(() => { for (const manager of managers.splice(0)) manager.disconnect({ endSession: true }); vi.clearAllMocks() })
function manager(tab: string) {
  const publish = vi.fn()
  const manager = new ConnectionManager({ serverUrl: 'ws://synthetic', token: 'synthetic' } as ConnectionConfig,
    { webContents: { send: publish } } as unknown as BrowserWindow, tab)
  managers.push(manager)
  const frames: string[] = []
  const socket = { readyState: 1, bufferedAmount: 0, send: (frame: string) => frames.push(frame), removeAllListeners: vi.fn(), on: vi.fn(), close: vi.fn() }
  const internal = manager as any
  internal.ws = socket
  internal.dispatchMessage({ type: 'session:created', sessionId: tab })
  return { manager, internal, frames, socket, publish }
}

describe('INC-2026-0057 manager lifecycle', () => {
  it('retains execution across reconnect and completes only after the owning ack', async () => {
    let finish!: (result: unknown) => void
    execute.mockImplementation(() => new Promise(resolve => { finish = resolve }))
    const m = manager('s')
    m.internal.dispatchMessage({ type: 'mcp:call', requestId: 'r', toolName: 'Write', input: {} })
    await vi.waitFor(() => expect(execute).toHaveBeenCalledOnce())
    m.manager.disconnect()
    finish('synthetic result')
    await new Promise(resolve => setTimeout(resolve, 0))
    expect(m.manager.hasPendingMcp('r')).toBe(true)
    expect(m.frames).toHaveLength(0)
    m.internal.ws = m.socket
    m.internal.dispatchMessage({ type: 'session:created', sessionId: 's' })
    expect(m.frames).toHaveLength(1)
    m.internal.dispatchMessage({ type: 'mcp:call', requestId: 'r', toolName: 'Write', input: {} })
    expect(execute).toHaveBeenCalledOnce()
    m.internal.dispatchMessage({ type: 'mcp:result-ack', sessionId: 'other', requestId: 'r', accepted: true })
    expect(m.manager.hasPendingMcp('r')).toBe(true)
    m.internal.dispatchMessage({ type: 'mcp:result-ack', sessionId: 's', requestId: 'r', accepted: true })
    await vi.waitFor(() => expect(m.manager.hasPendingMcp('r')).toBe(false))
    m.internal.dispatchMessage({ type: 'mcp:call', requestId: 'r', toolName: 'Write', input: {} })
    expect(execute).toHaveBeenCalledOnce()
    expect(m.publish.mock.calls.filter(call => call[2]?.status === 'completed')).toHaveLength(1)
  })

  it('routes denial to one owning tab and rejects server-session replacement', async () => {
    const a = manager('a')
    const b = manager('b')
    a.internal.pendingMcpCalls.set('r', { toolName: 'Write', input: {} })
    b.manager.denyMcp('r', 'synthetic denial')
    expect(b.frames).toHaveLength(0)
    a.manager.denyMcp('r', 'synthetic denial')
    expect(JSON.parse(a.frames[0]!)).toMatchObject({ type: 'mcp:denied', sessionId: 'a' })
    a.internal.dispatchMessage({ type: 'session:created', sessionId: 'replacement' })
    await new Promise(resolve => setTimeout(resolve, 0))
    expect(a.manager.hasPendingMcp('r')).toBe(false)
    expect(a.publish.mock.calls.some(call => call[2]?.status === 'completed')).toBe(false)
  })
})

import { afterEach, describe, expect, it, vi } from 'vitest'
import type { BrowserWindow } from '@kaminide/host-compat'
import type { ConnectionConfig, PermissionDecision } from '../../shared/types'

const execute = vi.hoisted(() => vi.fn())
vi.mock('vscode', () => ({ window: { showWarningMessage: vi.fn() } }))
vi.mock('../mcp/permission-manager', () => ({ PermissionManager: class {} }))
vi.mock('../mcp/executor', () => ({ executeTool: execute }))
import { ConnectionManager } from './connection-manager'
import * as vscode from 'vscode'

const managers: ConnectionManager[] = []
afterEach(() => {
  for (const manager of managers.splice(0)) manager.dispose()
  vi.clearAllMocks()
})

function fixture(tab = 'parent') {
  const publish = vi.fn()
  const manager = new ConnectionManager({ serverUrl: 'ws://synthetic', token: 'synthetic' } as ConnectionConfig,
    { webContents: { send: publish } } as unknown as BrowserWindow, tab)
  managers.push(manager)
  const frames: any[] = []
  const socket = { readyState: 1, bufferedAmount: 0, send: (frame: string) => frames.push(JSON.parse(frame)), removeAllListeners: vi.fn(), on: vi.fn(), close: vi.fn() }
  const internal = manager as any
  internal.ws = socket
  internal.dispatchMessage({ type: 'session:created', sessionId: tab })
  manager.setPermissionMode('ask')
  const permissions = {
    getToolCategory: vi.fn(() => 'shell'), checkPermission: vi.fn(() => 'ask'), setSessionPermission: vi.fn(),
  }
  internal.permissionManager = permissions
  const call = (id = 'request', taskOwner?: string) => internal.dispatchMessage({ type: 'mcp:call', requestId: id, toolName: 'Bash', input: {}, taskOwner })
  const ack = (id = 'request', accepted = true) => internal.dispatchMessage({ type: 'mcp:result-ack', sessionId: tab, requestId: id, accepted })
  const terminal = () => publish.mock.calls.filter(([channel, , payload]) => channel === 'mcp-activity' && payload.status !== 'pending')
  return { manager, internal, frames, socket, publish, permissions, call, ack, terminal }
}

describe('INC-2026-0077 MCP permission provenance and ownership', () => {
  it('reports policy denial without inventing a user response or publishing a prompt', async () => {
    const f = fixture()
    f.permissions.checkPermission.mockReturnValue('deny')
    f.call()
    await vi.waitFor(() => expect(f.frames).toHaveLength(1))
    expect(f.frames[0]).toMatchObject({ type: 'mcp:denied', sessionId: 'parent', requestId: 'request', reason: 'Permission denied by policy' })
    expect(f.publish.mock.calls.some(([channel]) => channel === 'permission-request')).toBe(false)
    expect(execute).not.toHaveBeenCalled()
    f.ack()
    await vi.waitFor(() => expect(f.manager.hasPendingMcp('request')).toBe(false))
    expect(f.terminal()).toHaveLength(1)
  })

  it.each<PermissionDecision>(['deny', 'allow_once', 'allow_session', 'always'])('answers an explicit %s exactly once', async decision => {
    execute.mockResolvedValue('executed once')
    const f = fixture()
    f.call()
    expect(f.internal.pendingPermissions.size).toBe(1)
    expect(execute).not.toHaveBeenCalled()
    const late = f.internal.pendingPermissions.get('request')
    f.manager.respondPermission('request', decision)
    late('allow_session')
    f.manager.respondPermission('request', 'allow_once')
    await vi.waitFor(() => expect(f.frames).toHaveLength(1))
    expect(execute).toHaveBeenCalledTimes(decision === 'deny' ? 0 : 1)
    expect(f.permissions.setSessionPermission).toHaveBeenCalledTimes(['allow_session', 'always'].includes(decision) ? 1 : 0)
    if (decision === 'deny') expect(f.frames[0].reason).toBe('User denied permission')
    else expect(f.frames[0]).toMatchObject({ type: 'mcp:response', result: 'executed once' })
    f.ack()
    f.ack()
    await vi.waitFor(() => expect(f.manager.hasPendingMcp('request')).toBe(false))
    f.call()
    expect(f.terminal()).toHaveLength(1)
    expect(f.internal.pendingPermissions.size).toBe(0)
    expect(f.publish.mock.calls.filter(([channel]) => channel === 'permission-resolved')).toHaveLength(1)
  })

  it('keeps unanswered requests pending and rejects another parent response', async () => {
    const a = fixture('a')
    const b = fixture('b')
    a.call()
    b.manager.respondPermission('request', 'allow_once')
    await new Promise(resolve => setTimeout(resolve, 0))
    expect(execute).not.toHaveBeenCalled()
    expect(a.frames).toHaveLength(0)
    expect(a.terminal()).toHaveLength(0)
    expect(a.manager.hasPendingMcp('request')).toBe(true)
    expect(a.internal.pendingPermissions.size).toBe(1)
  })

  it.each(['disconnect', 'socket-close'])('cancels pending approval on %s and delivers the distinct reason on reattach', async action => {
    const f = fixture()
    f.call()
    const late = f.internal.pendingPermissions.get('request')
    if (action === 'disconnect') f.manager.disconnect()
    else f.internal.setState({ status: 'disconnected' }) // websocket close handler
    late('always')
    await new Promise(resolve => setTimeout(resolve, 0))
    expect(f.internal.pendingPermissions.size).toBe(0)
    expect(f.publish.mock.calls.filter(([channel]) => channel === 'permission-resolved')).toHaveLength(1)
    expect(f.frames).toHaveLength(0)
    f.internal.ws = f.socket
    f.internal.dispatchMessage({ type: 'session:created', sessionId: 'parent' })
    expect(f.frames).toHaveLength(1)
    expect(f.frames[0].reason).toBe('Permission request cancelled: connection disconnected')
    f.ack()
    await vi.waitFor(() => expect(f.manager.hasPendingMcp('request')).toBe(false))
    expect(f.terminal()).toHaveLength(1)
    expect(execute).not.toHaveBeenCalled()
    expect(f.permissions.setSessionPermission).not.toHaveBeenCalled()
  })

  it.each(['dispose', 'replacement'])('reconciles a pending prompt on %s without a late execution or duplicate terminal outcome', async action => {
    const f = fixture()
    f.call()
    const late = f.internal.pendingPermissions.get('request')
    if (action === 'dispose') f.manager.dispose()
    else f.internal.dispatchMessage({ type: 'session:created', sessionId: 'replacement' })
    late('allow_session')
    await new Promise(resolve => setTimeout(resolve, 0))
    expect(f.internal.pendingPermissions.size).toBe(0)
    expect(f.manager.hasPendingMcp('request')).toBe(false)
    expect(f.terminal()).toHaveLength(1)
    expect(f.terminal()[0]![2]).toMatchObject({ requestId: 'request', status: 'denied', result: 'MCP call cancelled: session ended' })
    expect(f.frames).toHaveLength(0)
    expect(execute).not.toHaveBeenCalled()
    expect(f.permissions.setSessionPermission).not.toHaveBeenCalled()
  })

  it('only the owning task can cancel an approval, with a cancellation reason', async () => {
    const f = fixture()
    f.internal.dispatchMessage({ type: 'session:created', sessionId: 'parent', mcpTaskOwner: 'owner' })
    f.call('request', 'owner')
    f.internal.dispatchMessage({ type: 'mcp:cancel', requestId: 'request', taskOwner: 'other' })
    expect(f.internal.pendingPermissions.size).toBe(1)
    f.internal.dispatchMessage({ type: 'mcp:cancel', requestId: 'request', taskOwner: 'owner' })
    await vi.waitFor(() => expect(f.frames).toHaveLength(1))
    expect(f.frames[0]).toMatchObject({ reason: 'Permission request cancelled', taskOwner: 'owner' })
    f.ack()
    await vi.waitFor(() => expect(f.manager.hasPendingMcp('request')).toBe(false))
    expect(execute).not.toHaveBeenCalled()
  })

  it('ignores output and completion from an ended session when its request id is reused', async () => {
    let complete!: (result: string) => void
    let output!: (chunk: string) => void
    execute.mockImplementation((_tool, _input, options) => {
      output = options.onOutput
      return new Promise<string>(resolve => { complete = resolve })
    })
    const f = fixture()
    f.call()
    f.manager.respondPermission('request', 'allow_once')
    await vi.waitFor(() => expect(execute).toHaveBeenCalledOnce())
    f.internal.dispatchMessage({ type: 'session:created', sessionId: 'replacement' })
    f.call()
    output('late old output')
    expect(f.internal.pendingMcpCalls.get('request').output).toBeUndefined()
    complete('late old result')
    await new Promise(resolve => setTimeout(resolve, 0))
    expect(f.internal.pendingPermissions.size).toBe(1)
    expect(f.frames).toHaveLength(0)
    expect(f.terminal()).toHaveLength(1)
    f.manager.respondPermission('request', 'deny')
    await vi.waitFor(() => expect(f.frames).toHaveLength(1))
    expect(f.frames[0]).toMatchObject({ sessionId: 'replacement', reason: 'User denied permission' })
    f.internal.dispatchMessage({ type: 'mcp:result-ack', sessionId: 'replacement', requestId: 'request', accepted: true })
    await vi.waitFor(() => expect(f.manager.hasPendingMcp('request')).toBe(false))
    expect(f.terminal()).toHaveLength(2)
    expect(execute).toHaveBeenCalledOnce()
  })

  it('registers the pending decision before publishing, so an immediate response is retained', async () => {
    execute.mockResolvedValue('immediate')
    const f = fixture()
    f.publish.mockImplementation(channel => { if (channel === 'permission-request') f.manager.respondPermission('request', 'allow_once') })
    f.call()
    await vi.waitFor(() => expect(f.frames).toHaveLength(1))
    expect(execute).toHaveBeenCalledOnce()
    f.ack()
    await vi.waitFor(() => expect(f.manager.hasPendingMcp('request')).toBe(false))
  })

  it('fails closed when prompt publication fails, preserving the transport cause', async () => {
    const f = fixture()
    f.publish.mockImplementation(channel => { if (channel === 'permission-request') throw new Error('synthetic IPC unavailable') })
    f.call()
    await vi.waitFor(() => expect(f.frames).toHaveLength(1))
    expect(f.frames[0].reason).toBe('Permission request delivery failed: synthetic IPC unavailable')
    expect(f.internal.pendingPermissions.size).toBe(0)
    expect(execute).not.toHaveBeenCalled()
    f.ack()
    await vi.waitFor(() => expect(f.manager.hasPendingMcp('request')).toBe(false))
  })

  it('reports failed acknowledgement separately and clears the request without retrying execution', async () => {
    const f = fixture()
    f.permissions.checkPermission.mockReturnValue('deny')
    f.call()
    await vi.waitFor(() => expect(f.frames).toHaveLength(1))
    f.ack('request', false)
    await vi.waitFor(() => expect(f.manager.hasPendingMcp('request')).toBe(false))
    expect(f.terminal()).toHaveLength(1)
    expect(f.terminal()[0]![2].result).toContain('Delivery failed: Error: Server no longer accepts this MCP result')
    expect(vscode.window.showWarningMessage).toHaveBeenCalledOnce()
    expect(execute).not.toHaveBeenCalled()
  })
})

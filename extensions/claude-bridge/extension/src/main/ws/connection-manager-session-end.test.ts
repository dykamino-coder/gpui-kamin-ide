import { EventEmitter } from 'node:events'
import { afterEach, expect, it, vi } from 'vitest'
vi.mock('vscode', () => ({ window: { showWarningMessage: vi.fn() } }))
vi.mock('../mcp/permission-manager', () => ({ PermissionManager: class {} }))
vi.mock('../mcp/executor', () => ({ executeTool: vi.fn() }))
vi.mock('../plugin-monitors', () => ({ handleMonitorTriggerCommand: vi.fn(async () => false) }))
const executeHook = vi.fn(async () => ({ stdout: 'ok', stderr: '', exitCode: 0, outcome: 'success', durationMs: 3 }))
vi.mock('../hooks/executor', () => ({ executeHook }))
import { ConnectionManager } from './connection-manager'
import { endSessionEndLingers, SESSION_END_HOOK_WINDOW_MS } from './session-end-linger'

// BR-18A: on a tab close the client sent `session:end` and closed its socket in
// the same tick, so the server's relay of the session's own local SessionEnd
// hook found no client and the hook never ran.

const managers: ConnectionManager[] = []
afterEach(() => { managers.splice(0).forEach(m => m.dispose()); vi.useRealTimers(); executeHook.mockClear() })

class FakeSocket extends EventEmitter {
  readyState = 1
  sent: any[] = []
  send = vi.fn((frame: string) => { this.sent.push(JSON.parse(frame)) })
  close = vi.fn(() => { this.readyState = 3; this.emit('close', 1000, Buffer.from('')) })
}

function fixture() {
  vi.useFakeTimers()
  const manager = new ConnectionManager({ serverUrl: 'ws://synthetic', token: 'synthetic', cwd: '' } as any,
    { webContents: { send: vi.fn() } } as any, 'tab')
  managers.push(manager)
  const socket = new FakeSocket()
  const internal = manager as any
  internal.ws = socket
  internal.dispatchMessage({ type: 'session:created', sessionId: 'pty-1' })
  return { manager, socket }
}

const relayed = { type: 'hook:execute', requestId: 'r1', sessionId: 'pty-1', event: 'SessionEnd', command: 'marker', payload: {}, timeoutMs: 1000 }

it('runs the SessionEnd hook relayed after session:end and answers on the same socket', async () => {
  const { manager, socket } = fixture()
  manager.disconnect({ endSession: true })
  expect(socket.sent.at(-1)).toEqual({ type: 'session:end' })
  expect(socket.close).not.toHaveBeenCalled()

  socket.emit('message', Buffer.from(JSON.stringify(relayed)))
  await vi.advanceTimersByTimeAsync(0)
  expect(executeHook).toHaveBeenCalledOnce()
  expect(socket.sent.at(-1)).toMatchObject({ type: 'hook:response', requestId: 'r1', result: { outcome: 'success' } })
})

it('closes the socket when the window ends, and never later than a running hook', async () => {
  const { manager, socket } = fixture()
  let finish!: () => void
  executeHook.mockImplementationOnce(() => new Promise(resolve => {
    finish = () => resolve({ stdout: '', stderr: '', exitCode: 0, outcome: 'success', durationMs: 1 })
  }))
  manager.disconnect({ endSession: true })
  socket.emit('message', Buffer.from(JSON.stringify(relayed)))
  await vi.advanceTimersByTimeAsync(SESSION_END_HOOK_WINDOW_MS)
  expect(socket.close).not.toHaveBeenCalled()
  finish()
  await vi.advanceTimersByTimeAsync(0)
  expect(socket.sent.at(-1)).toMatchObject({ type: 'hook:response', requestId: 'r1' })
  expect(socket.close).toHaveBeenCalledOnce()
})

it('closes an idle ended socket at the end of the window', async () => {
  const { manager, socket } = fixture()
  manager.disconnect({ endSession: true })
  await vi.advanceTimersByTimeAsync(SESSION_END_HOOK_WINDOW_MS - 1)
  expect(socket.close).not.toHaveBeenCalled()
  await vi.advanceTimersByTimeAsync(1)
  expect(socket.close).toHaveBeenCalledOnce()
})

it('closes at once on a disconnect that does not end the session', () => {
  const { manager, socket } = fixture()
  manager.disconnect()
  expect(socket.close).toHaveBeenCalledOnce()
  expect(socket.sent.some(m => m.type === 'session:end')).toBe(false)
})

it('closes ended sockets at once when the host shuts down, and only once', async () => {
  const { manager, socket } = fixture()
  manager.disconnect({ endSession: true })
  expect(socket.close).not.toHaveBeenCalled()
  endSessionEndLingers()
  expect(socket.close).toHaveBeenCalledOnce()
  endSessionEndLingers()
  await vi.advanceTimersByTimeAsync(SESSION_END_HOOK_WINDOW_MS)
  expect(socket.close).toHaveBeenCalledOnce()
  expect(vi.getTimerCount()).toBe(0)
})

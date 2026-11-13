import { afterEach, expect, it, vi } from 'vitest'
vi.mock('vscode', () => ({ window: { showWarningMessage: vi.fn() } }))
vi.mock('../mcp/permission-manager', () => ({ PermissionManager: class {} }))
vi.mock('../mcp/executor', () => ({ executeTool: vi.fn() }))
import { ConnectionManager } from './connection-manager'

const managers: ConnectionManager[] = []
afterEach(() => { managers.splice(0).forEach(m => m.dispose()); vi.useRealTimers() })
function fixture() {
  vi.useFakeTimers()
  const publish = vi.fn(), frames: any[] = []
  const manager = new ConnectionManager({ serverUrl: 'ws://synthetic', token: 'synthetic' } as any,
    { webContents: { send: publish } } as any, 'tab')
  managers.push(manager)
  const internal = manager as any
  const socket = { readyState: 1, send: (frame: string) => frames.push(JSON.parse(frame)), removeAllListeners: vi.fn(), on: vi.fn(), close: vi.fn() }
  internal.ws = socket
  internal.dispatchMessage({ type: 'session:created', sessionId: 'old' })
  const receipt = (id: string, state: string) => internal.dispatchMessage({ type: 'session:submission', sessionId: 'old', id, state })
  const rows = () => publish.mock.calls.filter(c => c[0] === 'session:submission').map(c => c[2])
  return { manager, internal, socket, publish, frames, receipt, rows }
}

it('moves Sending/Queued to the replacement session once with their original IDs', () => {
  const f = fixture()
  f.manager.submitText('first', 'one'); f.receipt('one', 'sending')
  f.manager.submitText('second', 'two'); f.receipt('two', 'queued')
  f.manager.disconnectForRebind()
  expect(f.frames.at(-1)).toEqual({ type: 'session:end', handoff: true })
  expect(f.socket.close).not.toHaveBeenCalled()
  f.internal.dispatchMessage({ type: 'session:handoff', sessionId: 'unrelated', transferable: ['one'] })
  expect(f.socket.close).not.toHaveBeenCalled()
  f.internal.dispatchMessage({ type: 'session:handoff', sessionId: 'old', transferable: ['one', 'two'] })
  expect(f.socket.close).toHaveBeenCalledOnce()
  f.internal.ws = f.socket
  f.frames.length = 0
  f.internal.dispatchMessage({ type: 'session:created', sessionId: 'new' })
  f.internal.dispatchMessage({ type: 'session:created', sessionId: 'new' })
  expect(f.frames).toEqual([
    { type: 'session:submitText', data: 'first', submissionId: 'one' },
    { type: 'session:submitText', data: 'second', submissionId: 'two' },
  ])
  f.internal.dispatchMessage({ type: 'session:submission', sessionId: 'new', id: '', known: [] })
  expect(f.rows().at(-1).known).toEqual(['one', 'two'])
  f.receipt('one', 'read')
  f.receipt('two', 'native')
  expect(f.rows().at(-1)).toMatchObject({ id: 'two', state: 'native' })
})

it('retains unconfirmed native delivery with a visible reason and working retry/cancel', () => {
  const f = fixture()
  for (const id of ['retry', 'cancel']) { f.manager.submitText(id, id); f.receipt(id, 'native') }
  f.manager.disconnectForRebind()
  f.internal.dispatchMessage({ type: 'session:handoff', sessionId: 'old', transferable: [] })
  expect(f.rows().at(-1)).toMatchObject({ state: 'uncertain', retryable: true, cancelable: true, warning: expect.stringContaining('previous CLI') })
  f.manager.submissionAction('retry', 'push')
  expect(f.rows().at(-1).warning).toContain('Reconnect')
  f.internal.ws = f.socket
  f.internal.dispatchMessage({ type: 'session:created', sessionId: 'new' })
  f.frames.length = 0
  f.manager.submissionAction('retry', 'push')
  f.manager.submissionAction('retry', 'push')
  f.internal.dispatchMessage({ type: 'session:submission', sessionId: 'new', id: '', known: [] })
  expect(f.rows().at(-1).known).toContain('retry')
  f.manager.submissionAction('cancel', 'cancel')
  expect(f.frames.filter(m => m.type === 'session:submitText')).toEqual([{ type: 'session:submitText', data: 'retry', submissionId: 'retry' }])
  expect(f.rows().at(-1)).toMatchObject({ id: 'cancel', state: 'cancelled' })
})

it('bounds a missing handoff reply, keeps its outcome visible, and queues an immediate reconnect', () => {
  const f = fixture()
  f.manager.submitText('unknown', 'lost')
  f.manager.disconnectForRebind()
  f.manager.connect()
  const connect = vi.spyOn(f.manager, 'connect').mockImplementation(() => {})
  vi.advanceTimersByTime(2000)
  expect(connect).toHaveBeenCalledOnce()
  expect(f.rows().at(-1)).toMatchObject({ id: 'lost', state: 'uncertain', retryable: true, warning: expect.stringContaining('did not confirm') })
  f.internal.dispatchMessage({ type: 'session:handoff', sessionId: 'old', transferable: ['lost'] })
  expect(connect).toHaveBeenCalledOnce()
})

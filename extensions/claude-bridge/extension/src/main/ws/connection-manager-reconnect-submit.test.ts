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
  const rows = () => publish.mock.calls.filter(c => c[0] === 'session:submission').map(c => c[2])
  return { manager, internal, socket, frames, rows }
}

it.each([0, 2, 3, undefined])('explicitly rejects text while socket is %s, without replay after resume', readyState => {
  const f = fixture()
  f.internal.ws = readyState === undefined ? undefined : { ...f.socket, readyState }
  const text = 'first\nsecond\nthird\nfourth'
  f.manager.submitText(text, 'one')
  expect(f.rows().at(-1)).toMatchObject({ id: 'one', text, state: 'cancelled', rejected: true })
  f.internal.ws = f.socket
  f.internal.dispatchMessage({ type: 'session:created', sessionId: 'old' })
  f.internal.dispatchMessage({ type: 'session:created', sessionId: 'old' })
  expect(f.frames.filter(m => m.type === 'session:submitText')).toEqual([])
})

it('rejects during identity hold and sends only a new explicit attempt after confirmation', () => {
  const f = fixture()
  f.internal.identityHold = true
  f.manager.submitText('not for a replacement session', 'held')
  expect(f.rows().at(-1)).toMatchObject({ id: 'held', rejected: true })
  f.manager.sendInput('raw input')
  f.internal.dispatchMessage({ type: 'session:created', sessionId: 'replacement' })
  f.manager.submitText('first accepted', 'one')
  f.manager.submitText('second accepted', 'two')
  expect(f.frames.filter(m => ['session:submitText', 'session:input'].includes(m.type))).toEqual([
    { type: 'session:submitText', data: 'first accepted', submissionId: 'one' },
    { type: 'session:submitText', data: 'second accepted', submissionId: 'two' },
  ])
})

it('requires session confirmation even with an open socket and never sends after disposal', () => {
  const f = fixture()
  f.internal.state.status = 'connected'
  f.manager.submitText('before resume', 'early')
  expect(f.rows().at(-1)).toMatchObject({ id: 'early', rejected: true })
  f.manager.dispose()
  f.internal.ws = f.socket
  f.manager.submitText('closed tab', 'late')
  expect(f.frames.filter(m => m.type === 'session:submitText')).toEqual([])
})

it('rejects a synchronous transport failure and excludes it from handoff/retry', () => {
  const f = fixture()
  f.internal.ws = { ...f.socket, send: () => { throw new Error('socket closed') } }
  f.manager.submitText('not accepted', 'failed')
  expect(f.rows().at(-1)).toMatchObject({ id: 'failed', rejected: true })
  f.internal.ws = f.socket
  f.manager.disconnectForRebind()
  f.internal.dispatchMessage({ type: 'session:handoff', sessionId: 'old', transferable: ['failed'] })
  f.internal.ws = f.socket
  f.internal.dispatchMessage({ type: 'session:created', sessionId: 'new' })
  f.manager.submissionAction('failed', 'cancel')
  expect(f.frames.filter(m => m.type === 'session:submitText')).toEqual([])
})

it('rejects new prompts during tab rebind without buffering raw PTY input', () => {
  const f = fixture()
  f.manager.disconnectForRebind()
  f.manager.submitText('during handoff', 'new')
  expect(f.rows().at(-1)).toMatchObject({ id: 'new', rejected: true })
  f.internal.ws = undefined
  f.manager.sendInput('raw')
  f.internal.dispatchMessage({ type: 'session:handoff', sessionId: 'old', transferable: [] })
  f.internal.ws = f.socket
  f.internal.dispatchMessage({ type: 'session:created', sessionId: 'new' })
  expect(f.frames.filter(m => ['session:submitText', 'session:input'].includes(m.type))).toEqual([])
})

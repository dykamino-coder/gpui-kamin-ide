import { afterEach, expect, it, vi } from 'vitest'
vi.mock('vscode', () => ({ window: { showWarningMessage: vi.fn() } }))
vi.mock('../mcp/permission-manager', () => ({ PermissionManager: class {} }))
vi.mock('../mcp/executor', () => ({ executeTool: vi.fn() }))
import { ConnectionManager } from './connection-manager'
import { CHANGED_WARNING, EXPIRED_WARNING, HOLD_WARNING, RECONNECT_HOLD_MS } from './reconnect-outbox'

// A message written while the chat's own conversation reconnects (Bridge server
// restart): it waits visibly and goes out once that conversation is back,
// instead of being refused and having to be sent again by hand.

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
  internal.conversationId = 'conv-1'
  const rows = () => publish.mock.calls.filter(c => c[0] === 'session:submission').map(c => c[2])
  const sent = () => frames.filter(m => m.type === 'session:submitText')
  // Server gone: the socket closed and the tab is reconnecting.
  const drop = () => { internal.ws = { ...socket, readyState: 3 }; internal.state.status = 'connecting' }
  const back = () => { internal.ws = socket; internal.dispatchMessage({ type: 'session:created', sessionId: 'new' }) }
  return { manager, internal, rows, sent, drop, back }
}

it('holds messages written during the reconnect, shows it, and sends them in order once the chat is back', () => {
  const f = fixture()
  f.drop()
  f.manager.submitText('first', 'one')
  f.manager.submitText('second', 'two')
  expect(f.rows()).toEqual([
    expect.objectContaining({ id: 'one', state: 'queued', cancelable: true, warning: HOLD_WARNING }),
    expect.objectContaining({ id: 'two', state: 'queued', warning: HOLD_WARNING }),
  ])
  expect(f.rows().some(r => r.rejected)).toBe(false)
  expect(f.sent()).toEqual([])
  vi.advanceTimersByTime(5_000)
  f.back()
  expect(f.sent()).toEqual([
    { type: 'session:submitText', data: 'first', submissionId: 'one' },
    { type: 'session:submitText', data: 'second', submissionId: 'two' },
  ])
  // Once only: a later reconnect does not send them again.
  f.back()
  expect(f.sent()).toHaveLength(2)
})

it('returns the text to the composer when the chat is not back within the hold time', () => {
  const f = fixture()
  f.drop()
  f.manager.submitText('late', 'late')
  vi.advanceTimersByTime(RECONNECT_HOLD_MS)
  expect(f.rows().at(-1)).toMatchObject({ id: 'late', text: 'late', rejected: true, warning: EXPIRED_WARNING })
  f.back()
  expect(f.sent()).toEqual([])
})

it('never delivers into another conversation: the text goes back to the composer', () => {
  const f = fixture()
  f.drop()
  f.manager.submitText('for conv-1', 'c1')
  f.internal.conversationId = 'conv-2'
  f.back()
  expect(f.sent()).toEqual([])
  expect(f.rows().at(-1)).toMatchObject({ id: 'c1', rejected: true, warning: CHANGED_WARNING })
})

it('a held message can be cancelled; Send now waits for the chat like the hold itself', () => {
  const f = fixture()
  f.drop()
  f.manager.submitText('drop me', 'x')
  f.manager.submitText('keep me', 'y')
  f.manager.submissionAction('y', 'push')
  f.manager.submissionAction('x', 'cancel')
  expect(f.rows().at(-1)).toMatchObject({ id: 'x', state: 'cancelled' })
  f.back()
  expect(f.sent()).toEqual([{ type: 'session:submitText', data: 'keep me', submissionId: 'y' }])
})

it('still refuses during identity recovery, a chosen new conversation or a parked resume', () => {
  for (const setup of [
    (i: any) => { i.identityHold = { pendingId: null } },
    (i: any) => { i.pendingNewConversation = true },
    (i: any) => { i.resumeParked = true },
    (i: any) => { i.conversationId = null },
  ]) {
    const f = fixture()
    f.drop()
    setup(f.internal)
    f.manager.submitText('not held', 'n')
    expect(f.rows().at(-1)).toMatchObject({ id: 'n', rejected: true })
    f.internal.identityHold = null
    f.internal.pendingNewConversation = false
    f.internal.resumeParked = false
    f.back()
    expect(f.sent()).toEqual([])
  }
})

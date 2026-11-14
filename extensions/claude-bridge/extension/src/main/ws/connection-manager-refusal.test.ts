import { afterEach, expect, it, vi } from 'vitest'
vi.mock('vscode', () => ({ window: { showWarningMessage: vi.fn() } }))
vi.mock('../mcp/permission-manager', () => ({ PermissionManager: class {} }))
vi.mock('../mcp/executor', () => ({ executeTool: vi.fn() }))
import { ConnectionManager, SESSION_LIMIT_CLOSE_CODE, stickyRefusal } from './connection-manager'

// BR-05B: a tab refused at the per-token session limit kept retrying with only
// "Reconnecting... (attempt N)": the 4002 reason was overwritten by the next state.

const managers: ConnectionManager[] = []
afterEach(() => { managers.splice(0).forEach(m => m.dispose()); vi.useRealTimers() })

function fixture() {
  vi.useFakeTimers()
  const publish = vi.fn()
  const manager = new ConnectionManager({ serverUrl: 'ws://synthetic', token: 'synthetic' } as any,
    { webContents: { send: publish } } as any, 'tab')
  managers.push(manager)
  vi.spyOn(manager, 'connect').mockImplementation(() => {})
  return { manager, internal: manager as any }
}

/** The socket `close` handler's two steps for a refused tab. */
function refusedClose(internal: any): void {
  internal.setState({ status: 'disconnected', error: 'Connection closed: 4002 Max sessions reached', closeCode: SESSION_LIMIT_CLOSE_CODE })
  internal.scheduleReconnect()
}

it('keeps the session-limit refusal on the tab through the reconnect states', () => {
  const { manager, internal } = fixture()
  refusedClose(internal)
  expect(manager.getState()).toMatchObject({ status: 'connecting', retryAttempt: 1, refusal: 'session-limit' })
  vi.advanceTimersByTime(1000)
  internal.setState({ status: 'connected' })
  expect(manager.getState().refusal).toBe('session-limit')
  refusedClose(internal)
  expect(manager.getState()).toMatchObject({ retryAttempt: 2, refusal: 'session-limit' })
})

it('clears the refusal once the tab connects', () => {
  const { manager, internal } = fixture()
  refusedClose(internal)
  internal.dispatchMessage({ type: 'session:created', sessionId: 'pty-1' })
  expect(manager.getState()).toMatchObject({ status: 'connected', refusal: undefined })
  internal.setState({ status: 'disconnected', closeCode: 1006 })
  expect(manager.getState().refusal).toBeUndefined()
})

it('names no refusal for other closes', () => {
  const { manager, internal } = fixture()
  internal.setState({ status: 'disconnected', closeCode: 1006 })
  internal.scheduleReconnect()
  expect(manager.getState().refusal).toBeUndefined()
})

it('drops the refusal on an intentional disconnect', () => {
  expect(stickyRefusal('session-limit', { status: 'disconnected' }, true)).toBeUndefined()
  expect(stickyRefusal('session-limit', { status: 'connecting' }, false)).toBe('session-limit')
  expect(stickyRefusal(undefined, { status: 'disconnected', closeCode: 4002 }, false)).toBe('session-limit')
})

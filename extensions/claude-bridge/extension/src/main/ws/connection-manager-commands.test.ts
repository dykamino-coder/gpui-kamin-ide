import { afterEach, expect, it, vi } from 'vitest'
import type { BrowserWindow } from '@kaminide/host-compat'
import type { ConnectionConfig } from '../../shared/types'
const fixture = vi.hoisted(() => ({ local: vi.fn(), report: vi.fn(), stop: vi.fn(), remoteStop: vi.fn() }))
vi.mock('vscode', () => ({ window: { showWarningMessage: vi.fn() } }))
vi.mock('../mcp/permission-manager', () => ({ PermissionManager: class {} }))
vi.mock('../mcp/tools/shell', () => ({
  localBackgroundShellSnapshot: fixture.local,
  backgroundShellSnapshot: fixture.report,
  stopLocalBackgroundShell: fixture.stop,
  stopBackgroundShellForOwner: fixture.remoteStop,
}))
import { ConnectionManager } from './connection-manager'
const managers: ConnectionManager[] = []
afterEach(() => {
  for (const manager of managers.splice(0)) manager.disconnect({ endSession: true })
  vi.clearAllMocks()
})
it.each([undefined, 'new-owner'])('routes legacy controls locally with server owner %s', (mcpTaskOwner) => {
  const publish = vi.fn()
  const frames: any[] = []
  const manager = new ConnectionManager(
    { serverUrl: 'ws://synthetic', token: 'synthetic' } as ConnectionConfig,
    { webContents: { send: publish } } as unknown as BrowserWindow,
    'tab',
  )
  managers.push(manager)
  const internal = manager as any
  internal.ws = {
    readyState: 1,
    bufferedAmount: 0,
    send: (s: string) => frames.push(JSON.parse(s)),
    removeAllListeners: vi.fn(),
    on: vi.fn(),
    close: vi.fn(),
  }
  internal.dispatchMessage({ type: 'session:created', sessionId: 's', mcpTaskOwner })
  fixture.local.mockReturnValue([
    { id: 'shell:1', tool: 'PowerShell', command: 'legacy', startedAt: 1, running: true, output: 'Привет' },
  ])
  fixture.report.mockReturnValue([])
  manager.commandAction('list')
  expect(publish).toHaveBeenLastCalledWith(
    'commands:snapshot',
    'tab',
    expect.objectContaining({ serverAvailable: false, commands: [expect.objectContaining({ id: 'shell:1' })] }),
  )
  manager.commandAction('output', 'shell:1')
  expect(publish).toHaveBeenLastCalledWith('commands:output', 'tab', { id: 'shell:1', output: 'Привет' })
  manager.commandAction('stop', 'shell:1')
  expect(fixture.stop).toHaveBeenCalledWith('tab', 'shell:1')
  expect(frames).toEqual([{ type: 'commands:list' }])
  internal.dispatchMessage({ type: 'commands:request' })
  expect(fixture.report).toHaveBeenCalledWith('tab', mcpTaskOwner)
  expect(frames.at(-1)).toMatchObject({ type: 'commands:report', report: { shells: [] } })
  internal.dispatchMessage({ type: 'commands:snapshot', snapshot: { sessionId: 's', commands: [] } })
  expect(publish.mock.lastCall?.[2].serverAvailable).toBe(true)
  const count = publish.mock.calls.length
  internal.dispatchMessage({ type: 'commands:snapshot', snapshot: { sessionId: 'other', commands: [] } })
  expect(publish).toHaveBeenCalledTimes(count)
  manager.commandAction('stop', 'task')
  expect(frames.at(-1)).toEqual({ type: 'commands:stop', id: 'task' })
})

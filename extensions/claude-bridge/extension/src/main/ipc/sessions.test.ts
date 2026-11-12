import { describe, expect, it, vi } from 'vitest'

// tab:create writes the new tab's server URL and token into the global config.
// A partial config must be refused before anything changes: `createTab({})`
// used to erase the stored token, and every later new chat failed silently.

const ipc = vi.hoisted(() => ({ handlers: new Map<string, (...args: unknown[]) => unknown>() }))
vi.mock('@kaminide/host-compat', () => ({
  ipcMain: {
    handle: (channel: string, fn: (...args: unknown[]) => unknown) => ipc.handlers.set(channel, fn),
    on: vi.fn(),
  },
}))
vi.mock('../ws/connection-manager', () => ({ ConnectionManager: class {} }))
vi.mock('../mcp/tools/ui-tools', () => ({ updateUiToolsActiveTab: vi.fn() }))
vi.mock('../../session-activation', () => ({ activateSessionForTab: vi.fn() }))
vi.mock('../../displayed-tab', () => ({ reportDisplayedTab: vi.fn(), forgetDisplayedTab: vi.fn() }))

import { registerSessionsIPC, tabConfigProblem } from './sessions'

function setup() {
  let stored = { serverUrl: 'ws://127.0.0.1:3462', token: 'stored-token' }
  const configStore = {
    get: () => stored,
    set: vi.fn((next: typeof stored) => { stored = next }),
    getDefaultEngine: () => 'claude',
  }
  const tabs = { createTab: vi.fn(() => 'tab-1') }
  registerSessionsIPC({
    configStore: configStore as never,
    getTabManager: () => tabs as never,
    getMainWindow: () => null,
    getUserCwd: () => null,
  })
  const create = (config: unknown) => ipc.handlers.get('tab:create')!({}, config)
  return { create, configStore, tabs, stored: () => stored }
}

describe('tab:create', () => {
  it.each([
    [{}, 'serverUrl is required'],
    [{ serverUrl: 'ws://127.0.0.1:3462' }, 'token is required'],
    [{ token: 'stored-token', serverUrl: '  ' }, 'serverUrl is required'],
    [undefined, 'a connection config is required'],
  ])('refuses %j without touching the stored config', (config, problem) => {
    const { create, configStore, tabs, stored } = setup()
    expect(() => create(config)).toThrow(`tab:create: ${problem}`)
    expect(configStore.set).not.toHaveBeenCalled()
    expect(tabs.createTab).not.toHaveBeenCalled()
    expect(stored()).toEqual({ serverUrl: 'ws://127.0.0.1:3462', token: 'stored-token' })
  })

  it('opens a tab and stores a complete config', () => {
    const { create, tabs, stored } = setup()
    expect(create({ serverUrl: 'ws://bridge:3456', token: 'new-token', cwd: '' })).toBe('tab-1')
    expect(tabs.createTab).toHaveBeenCalledTimes(1)
    expect(stored()).toMatchObject({ serverUrl: 'ws://bridge:3456', token: 'new-token' })
  })

  it('names the missing field', () => {
    expect(tabConfigProblem({ serverUrl: 'ws://x', token: 't' })).toBeNull()
    expect(tabConfigProblem({ serverUrl: 'ws://x', token: '' })).toBe('token is required')
  })
})

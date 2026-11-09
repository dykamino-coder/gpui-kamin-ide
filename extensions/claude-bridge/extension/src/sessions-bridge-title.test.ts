import { beforeEach, describe, expect, it, vi } from 'vitest'
import type { BridgeHost } from './bridge-host'

const updateSession = vi.hoisted(() => vi.fn())
vi.mock('kaminide', () => ({ sessions: { updateSession } }))
vi.mock('@kaminide/host-compat', () => ({ ipcMain: {} }))
vi.mock('./main/ws/connection-manager', () => ({ ConnectionManager: {} }))
import { SessionsBridge } from './sessions-bridge'

describe('native title regeneration integration', () => {
  beforeEach(() => updateSession.mockReset())

  function fixture(live: boolean) {
    const submit = vi.fn()
    const host = { getTabManager: () => ({ getTab: () => live ? {} : undefined }), cmdRegenerateTitle: submit }
    const sessions = new SessionsBridge(host as unknown as BridgeHost, vi.fn())
    // Native session mappings are established by install/onActive in production.
    Object.assign(sessions, { tabToKamin: new Map([['tab', 'native-session']]), kaminToTab: new Map([['native-session', 'tab']]) })
    return { sessions, submit }
  }

  it('clears the native sticky name before delegating header and context-menu requests to the shared host', () => {
    const f = fixture(true)
    f.sessions.regenerateTitleByTab('tab')
    f.sessions.regenerateTitleByKamin('native-session')
    expect(updateSession).toHaveBeenCalledTimes(2)
    expect(updateSession).toHaveBeenCalledWith('native-session', { metadata: { nameSetByUser: false } })
    expect(f.submit.mock.calls).toEqual([['tab'], ['tab']])
    expect(updateSession.mock.invocationCallOrder[0]).toBeLessThan(f.submit.mock.invocationCallOrder[0])
  })

  it('does not clear a name or submit for a session with no live tab', () => {
    const f = fixture(false)
    f.sessions.regenerateTitleByTab('tab')
    f.sessions.regenerateTitleByKamin('native-session')
    expect(updateSession).not.toHaveBeenCalled()
    expect(f.submit).not.toHaveBeenCalled()
  })
})

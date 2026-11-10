import { afterEach, describe, expect, it, vi } from 'vitest'

vi.mock('./ws/connection-manager', () => ({ ConnectionManager: class {
  status = 'disconnected'
  connect = vi.fn(() => { this.status = 'connecting' })
  disconnect = vi.fn()
  dispose = vi.fn()
  getState = () => ({ status: 'authenticated', authority: 'session', revision: 1 })
  setRendererMuted = vi.fn()
  getExtendedState = () => ({ status: this.status })
} }))
vi.mock('./sync/sync-client', () => ({ syncUserData: vi.fn(), syncProjectData: vi.fn(), resetSyncTimers: vi.fn() }))
vi.mock('fs', () => ({ default: { appendFileSync: vi.fn() } }))
import { TabManager } from './tab-manager'
import { syncUserData, type SyncResult } from './sync/sync-client'
import type { ConnectionConfig } from '../shared/types'

const config = { cwd: '/synthetic-project' } as ConnectionConfig
afterEach(() => vi.mocked(syncUserData).mockReset())
function manager() {
  return new TabManager({ isDestroyed: () => false, webContents: { send: vi.fn() } } as any)
}

describe('tab-owned close cleanup', () => {
  for (const conversationId of [undefined, 'known-conversation']) {
    it(`cleans ${conversationId ? 'identified' : 'unidentified'} tab before disconnect/removal exactly once`, () => {
      const tabs = manager(), id = tabs.createTab(config), other = tabs.createTab(config)
      const tab = tabs.getTab(id)!
      tab.conversationId = conversationId
      const onClose = vi.fn(closing => {
        expect(closing).toBe(tab)
        expect(tabs.getTab(id)).toBe(tab)
        expect(tab.connection.disconnect).not.toHaveBeenCalled()
      })
      tabs.onTabClosing = onClose
      tabs.closeTab(id)
      tabs.closeTab(id)
      tabs.closeTab('missing')
      expect(onClose).toHaveBeenCalledTimes(1)
      expect(tab.connection.disconnect).toHaveBeenCalledExactlyOnceWith({ endSession: true })
      expect(tabs.getTab(id)).toBeUndefined()
      expect(tabs.getConnection(other)!.disconnect).not.toHaveBeenCalled()
      expect(tabs.getActiveTabId()).toBe(other)
    })
  }

  it('closes every identified and unidentified tab during close-all', () => {
    const tabs = manager(), first = tabs.createTab(config), second = tabs.createTab(config)
    tabs.getTab(second)!.conversationId = 'known-conversation'
    const onClose = vi.fn()
    tabs.onTabClosing = onClose
    tabs.closeAllTabs()
    tabs.closeAllTabs()
    expect(onClose.mock.calls.map(([tab]) => tab.id)).toEqual([first, second])
    expect(tabs.size).toBe(0)
  })

  it('does not connect dormant restored tabs during bulk close', () => {
    const tabs = manager()
    const first = tabs.resumeSession({ ...config, conversationId: 'first' }, { background: true })
    const second = tabs.resumeSession({ ...config, conversationId: 'second' }, { background: true })
    tabs.switchTab(first)
    const connections = [tabs.getConnection(first)!, tabs.getConnection(second)!]
    for (const connection of connections) vi.mocked(connection.connect).mockClear()
    tabs.closeAllTabs()
    for (const connection of connections) {
      expect(connection.connect).not.toHaveBeenCalled()
      expect(connection.disconnect).toHaveBeenCalledExactlyOnceWith({ endSession: true })
    }
    expect(tabs.getActiveTabId()).toBeNull()
  })
})

describe('initial connection ownership', () => {
  for (const resume of [false, true]) {
    for (const failSync of [false, true]) {
      it(`${resume ? 'resume' : 'create'} activates during ${failSync ? 'failed' : 'successful'} sync without double connecting`, async () => {
        let finish!: () => void
        vi.mocked(syncUserData).mockImplementationOnce(() => new Promise<SyncResult>((resolve, reject) => {
          finish = () => failSync ? reject(new Error('Synthetic sync failure')) : resolve({ ok: true })
        }))
        const tabs = manager(), cfg = { ...config, serverUrl: 'http://synthetic.invalid', token: 'synthetic-token' }
        const id = resume ? tabs.resumeSession({ ...cfg, conversationId: 'known-conversation' }) : tabs.createTab(cfg)
        const connection = tabs.getConnection(id)!
        tabs.switchTab(id)
        tabs.switchTab(id)
        expect(tabs.getActiveTabId()).toBe(id)
        expect(connection.connect).not.toHaveBeenCalled()
        finish()
        await vi.waitFor(() => expect(connection.connect).toHaveBeenCalledTimes(1))
        tabs.switchTab(id)
        expect(connection.connect).toHaveBeenCalledTimes(1)
      })
    }
  }

  for (const resume of [false, true]) {
    for (const action of ['close', 'disconnect', 'reconnect', 'dispose'] as const) {
      it(`does not resurrect pending ${resume ? 'resume' : 'create'} after ${action}`, async () => {
        let finish!: () => void
        vi.mocked(syncUserData).mockImplementationOnce(() => new Promise<SyncResult>(resolve => { finish = () => resolve({ ok: true }) }))
        const tabs = manager(), cfg = { ...config, serverUrl: 'http://synthetic.invalid', token: 'synthetic-token' }
        const id = resume ? tabs.resumeSession({ ...cfg, conversationId: 'known-conversation' }) : tabs.createTab(cfg)
        const connection = tabs.getConnection(id)!
        if (action === 'close') tabs.closeTab(id)
        if (action === 'disconnect') tabs.disconnectTab(id)
        if (action === 'reconnect') tabs.reconnectTab(id)
        if (action === 'dispose') tabs.dispose()
        finish()
        // Flush both sync stages and their completion callback.
        await new Promise(resolve => setTimeout(resolve, 0))
        expect(connection.connect).toHaveBeenCalledTimes(action === 'reconnect' ? 1 : 0)
      })
    }
  }
  for (const resume of [false, true]) {
    it(`lazily connects background ${resume ? 'resume' : 'create'} exactly once`, () => {
      const tabs = manager()
      const id = resume ? tabs.resumeSession({ ...config, conversationId: 'known-conversation' }, { background: true })
        : tabs.createTab(config, { background: true })
      const connection = tabs.getConnection(id)!
      expect(connection.connect).not.toHaveBeenCalled()
      tabs.switchTab(id)
      tabs.switchTab(id)
      expect(connection.connect).toHaveBeenCalledTimes(1)
    })
  }
})

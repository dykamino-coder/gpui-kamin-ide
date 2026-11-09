import { describe, expect, it, vi } from 'vitest'

vi.mock('./ws/connection-manager', () => ({ ConnectionManager: class {
  connect = vi.fn()
  disconnect = vi.fn()
  getState = () => ({ status: 'authenticated', authority: 'session', revision: 1 })
} }))
vi.mock('./sync/sync-client', () => ({ syncUserData: vi.fn(), syncProjectData: vi.fn(), resetSyncTimers: vi.fn() }))
vi.mock('fs', () => ({ default: { appendFileSync: vi.fn() } }))
import { TabManager } from './tab-manager'
import type { ConnectionConfig } from '../shared/types'

const config = { cwd: '/synthetic-project' } as ConnectionConfig
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
})

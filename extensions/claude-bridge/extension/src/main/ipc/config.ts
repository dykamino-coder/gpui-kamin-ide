import { ipcMain, type IpcMainEvent } from '@kaminide/host-compat'
import os from 'os'
import type { ConfigStore } from '../config/store'
import type { TabManager } from '../tab-manager'
import { spawnToast } from '../notifications/toast-window'
import type { ConnectionConfig } from '../../shared/types'

export interface ConfigIpcContext {
  configStore: ConfigStore
  getTabManager: () => TabManager | null
  getUserCwd: () => string | null
}

export function registerConfigIPC(ctx: ConfigIpcContext): void {
  ipcMain.handle('get-config', () => ctx.configStore.get())
  // Standing instructions, per token — appended to (never replacing) the
  // technical system prompt. Kept out of `config` so a token switch can't carry
  // one workspace's prompt into another.
  ipcMain.handle('base-prompt:get', () => ctx.configStore.getBasePrompt())
  ipcMain.handle('base-prompt:set', (_e: unknown, text: string) => { ctx.configStore.setBasePrompt(text); return true })
  ipcMain.on('set-config', (_event: IpcMainEvent, config: ConnectionConfig) => {
    const prev = ctx.configStore.get()
    ctx.configStore.set(config)
    // Token change — the in-memory tabs belong to the previous token's
    // server-side sessions. Close them so the sidebar reflects the new
    // scope (its own per-token `savedSessions` + whatever the server can
    // restore).
    const tm = ctx.getTabManager()
    if (prev?.token && config?.token && prev.token !== config.token && tm) {
      tm.closeAllTabs()
    }
  })

  // Layout persistence (sidebar width, etc.) — written through ConfigStore.
  ipcMain.handle('layout:get', () => ctx.configStore.getLayout())
  ipcMain.on('layout:set', (_event: IpcMainEvent, patch: Record<string, unknown>) => {
    ctx.configStore.setLayout(patch as Partial<import('../config/store').LayoutState>)
  })

  // Notification preferences — which background toasts we spawn.
  ipcMain.handle('notifications:get', () => ctx.configStore.getNotifications())
  ipcMain.on('notifications:set', (_event: IpcMainEvent, patch: Record<string, unknown>) => {
    ctx.configStore.setNotifications(patch as Partial<import('../config/store').NotificationsConfig>)
  })
  // Test button: spawn a demo finished-toast immediately, bypassing both the
  // "main window is focused" and the "enabled" gates.
  ipcMain.handle('notifications:test', (_event, variant?: 'single' | 'burst' | 'sticky') => {
    const mode = variant ?? 'single'
    const force = { bypassFocusGate: true, bypassEnabledGate: true }
    if (mode === 'single') {
      spawnToast(
        {
          kind: 'finished',
          tabId: 'test',
          title: 'Test notification',
          message: 'If you can see this, background toasts work.',
        },
        force,
      )
    } else if (mode === 'sticky') {
      spawnToast(
        {
          kind: 'question',
          tabId: 'test',
          requestId: `test-${Date.now()}`,
          title: 'Sticky test',
          message: 'This toast only goes away when you close it manually.',
          sticky: true,
        },
        force,
      )
    } else {
      for (let i = 1; i <= 10; i++) {
        const sticky = i % 3 === 0
        spawnToast(
          {
            kind: sticky ? 'question' : 'finished',
            tabId: `test-${i}`,
            requestId: sticky ? `test-burst-${Date.now()}-${i}` : undefined,
            title: `${sticky ? 'Sticky' : 'Auto'} #${i}`,
            message: `Burst message ${i}/10. ${sticky ? 'Close me manually.' : 'Auto-dismiss in 8s.'}`,
            sticky,
          },
          force,
        )
      }
    }
    return { ok: true }
  })

  ipcMain.handle('get-cwd', () => ctx.getUserCwd())

  // Suggested token name: "<domain>/<user>" — used by the first-run gate modal
  // so the user doesn't have to invent a name.
  ipcMain.handle('get-machine-id', () => {
    const env = process.env
    const user = env.USER || env.USERNAME || os.userInfo().username || 'user'
    const domain = env.USERDOMAIN || env.COMPUTERNAME || os.hostname() || 'local'
    return `${domain}\\${user}`.replace(/[^A-Za-z0-9._\-\\]/g, '-')
  })
}

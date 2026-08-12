import { ipcMain, type IpcMainInvokeEvent } from '@kaminide/host-compat'
import type { ConfigStore } from '../config/store'
import { saveTabsState, loadTabsState, type PersistedTab } from '../tabs-state-store'

export interface TabsIpcContext {
  configStore: ConfigStore
}

export function registerTabsIPC(ctx: TabsIpcContext): void {
  ipcMain.handle('tabs:save-state', (_event: IpcMainInvokeEvent, tabs: PersistedTab[]) => {
    const token = ctx.configStore.get().token
    if (!token) return { ok: false }
    saveTabsState(Array.isArray(tabs) ? tabs : [], token)
    return { ok: true }
  })

  ipcMain.handle('tabs:restore-state', () => {
    const token = ctx.configStore.get().token
    if (!token) return []
    return loadTabsState(token) ?? []
  })
}

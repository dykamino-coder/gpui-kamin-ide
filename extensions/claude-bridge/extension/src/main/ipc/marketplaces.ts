import { ipcMain, type BrowserWindow, type IpcMainInvokeEvent } from 'electron'
import {
  listMarketplaces,
  cloneMarketplace,
  refreshMarketplaceOnce,
  refreshAllMarketplaces,
  setMarketplaceAuth,
  removeMarketplace,
  setMarketplaceAutoUpdate,
} from '../marketplace'

/** Get main window reference lazily — caller passes a getter because
 *  mainWindow may be recreated (e.g. after macOS dock re-open). */
export function registerMarketplaceIPC(getMainWindow: () => BrowserWindow | null): void {
  ipcMain.handle('plugins:list-marketplaces', () => {
    try { return listMarketplaces() } catch { return [] }
  })

  ipcMain.handle('plugins:add-marketplace', (
    _event: IpcMainInvokeEvent,
    name: string,
    rawInput: string,
    auth?: { username?: string; token?: string } | null,
  ) => {
    return cloneMarketplace({ name, rawInput, auth })
  })

  ipcMain.handle('plugins:set-marketplace-auth', async (
    _event: IpcMainInvokeEvent,
    name: string,
    auth: { username?: string; token?: string } | null,
  ) => {
    return setMarketplaceAuth(name, auth)
  })

  ipcMain.handle('plugins:remove-marketplace', (_event: IpcMainInvokeEvent, name: string) => {
    removeMarketplace(name)
  })

  ipcMain.handle('plugins:update-marketplace', async (_event: IpcMainInvokeEvent, name: string) => {
    return await refreshMarketplaceOnce(name)
  })

  ipcMain.handle('plugins:refresh-all-marketplaces', async () => {
    await refreshAllMarketplaces(getMainWindow() ?? undefined)
    return { ok: true }
  })

  ipcMain.handle('plugins:set-marketplace-autoupdate', (
    _event: IpcMainInvokeEvent,
    name: string,
    autoUpdate: boolean,
  ) => {
    return setMarketplaceAutoUpdate(name, autoUpdate)
  })

  // Kick off auto-refresh once per app launch, in the background. Delay a
  // bit so the window is up and the user isn't competing with a burst of
  // git requests at cold start. Matches CLI's at-session-start behaviour.
  setTimeout(() => {
    refreshAllMarketplaces(getMainWindow() ?? undefined).catch(err => {
      console.warn('[marketplaces] auto-refresh failed:', err instanceof Error ? err.message : err)
    })
  }, 8_000)
}

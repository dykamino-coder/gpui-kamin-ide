import { ipcMain } from '@kaminide/host-compat'
import { forceSync, type ForceSyncContext } from '../sync/force-sync'

export function registerSyncIPC(ctx: ForceSyncContext): void {
  ipcMain.handle('sync:force', () => forceSync(ctx))
}

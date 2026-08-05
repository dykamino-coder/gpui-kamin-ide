// IPC for the Customize → Logs panel (vendored from electron/main/ipc/logs.ts).
// The Electron original re-synced the broadcast window every 3s because macOS
// dock re-open could recreate it; the kamin sink is a stable fan-out object,
// so a single setLogsWindow call suffices.

import { ipcMain, type BrowserWindow } from '@kaminide/host-compat'
import { getLogs, clearLogs, setLogsWindow } from '../error-log'

export function registerLogsIPC(getMainWindow: () => BrowserWindow | null): void {
  ipcMain.handle('logs:list', () => getLogs())
  ipcMain.handle('logs:clear', () => { clearLogs(); return { ok: true } })
  setLogsWindow(getMainWindow())
}

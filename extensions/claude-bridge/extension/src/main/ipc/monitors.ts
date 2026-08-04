// IPC for the Customize → Monitors panel. The Electron original lived in
// ipc/monitors-lsp.ts and also registered lsp:* handlers; in KaminIDE the LSP
// stack belongs to the host, not the Bridge extension, so this ports only the
// monitors half.

import { ipcMain, type IpcMainInvokeEvent } from 'electron'
import { listMonitors, getMonitorLog } from '../plugin-monitors'

export function registerMonitorsIPC(): void {
  ipcMain.handle('monitors:list', () => listMonitors())
  ipcMain.handle('monitors:get-log', (_event: IpcMainInvokeEvent, id: string) => getMonitorLog(id))
}

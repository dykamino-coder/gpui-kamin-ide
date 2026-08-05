// IPC for the Customize → Monitors panel and plugin-provided LSP harness.

import { ipcMain, type IpcMainInvokeEvent } from 'electron'
import { listMonitors, getMonitorLog } from '../plugin-monitors'
import { pluginLspManager } from '../plugin-lsp'

export function registerMonitorsIPC(): void {
  ipcMain.handle('monitors:list', () => listMonitors())
  ipcMain.handle('monitors:get-log', (_event: IpcMainInvokeEvent, id: string) => getMonitorLog(id))
  ipcMain.handle('lsp:list', () => pluginLspManager.list())
  ipcMain.handle('lsp:restart', async () => {
    await pluginLspManager.restart()
    return { ok: true }
  })
}

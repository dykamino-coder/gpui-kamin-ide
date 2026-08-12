// KaminIDE host-compatibility facade for the VSIX bridge.
//
// The legacy standalone client used Electron-shaped IPC. Current source imports
// this module as `@kaminide/host-compat`; it implements only the small facade
// required by the ported handlers over the VSIX webview postMessage bridge.
import { EventEmitter } from "node:events"
import * as os from "node:os"
import * as path from "node:path"
import * as vscode from "vscode"

export interface WebContents {
  send(channel: string, ...args: unknown[]): void
}
export interface BrowserWindow {
  webContents: WebContents
  isDestroyed(): boolean
}
export type IpcMainEvent = { sender: WebContents }
export type IpcMainInvokeEvent = { sender: WebContents }

// Runtime side of `BrowserWindow` (value + type share the name). The vendored
// plugins handler calls `BrowserWindow.getAllWindows()` to broadcast a
// plugin-hook-approval event; BridgeHost registers its sink via
// registerShimWindow so that call resolves to the live webview fan-out.
const _windows: BrowserWindow[] = []
export function registerShimWindow(win: BrowserWindow): void {
  if (!_windows.includes(win)) _windows.push(win)
}
export const BrowserWindow = {
  getAllWindows(): BrowserWindow[] { return _windows },
}

type Handler = (event: IpcMainInvokeEvent, ...args: any[]) => unknown

class IpcMainShim extends EventEmitter {
  private handlers = new Map<string, Handler>()

  handle(channel: string, fn: Handler): void {
    this.handlers.set(channel, fn)
  }
  handleOnce(channel: string, fn: Handler): void {
    this.handlers.set(channel, (e, ...a) => {
      this.handlers.delete(channel)
      return fn(e, ...a)
    })
  }
  removeHandler(channel: string): void {
    this.handlers.delete(channel)
  }
  hasHandler(channel: string): boolean {
    return this.handlers.has(channel)
  }
  /** Invoke a registered `handle()` channel — used by the bridge router for
   *  webview `invoke()` calls. */
  async invokeHandler(channel: string, event: IpcMainInvokeEvent, ...args: any[]): Promise<unknown> {
    const fn = this.handlers.get(channel)
    if (!fn) throw new Error(`No ipc handler registered for "${channel}"`)
    return await fn(event, ...args)
  }
}

export const ipcMain = new IpcMainShim()
// Many handlers register on the singleton; lift the default 10-listener cap.
ipcMain.setMaxListeners(0)

// Shell helpers used by the vendored skills/plugins handlers. In an IDE the
// right "open" for a text file is the editor itself; reveal maps to the same
// host command core-ipc uses for `open-folder`.
export const shell = {
  async openPath(p: string): Promise<string> {
    try {
      await vscode.window.showTextDocument(vscode.Uri.file(p))
      return ''
    } catch {
      try { await vscode.env.openExternal(vscode.Uri.file(p)); return '' } catch (e) { return String(e) }
    }
  },
  showItemInFolder(p: string): void {
    void vscode.commands.executeCommand('revealFileInOS', vscode.Uri.file(p)).then(undefined, () => { /* ignore async reject */ })
  },
}

// Minimal `app` — the vendored MCP oauth-store only uses getPath('userData')
// in its LEGACY-migration path, so a home-relative dir is sufficient.
export const app = {
  getPath(name: string): string {
    if (name === "userData") return path.join(os.homedir(), ".claude", "open-claude-bridge")
    if (name === "home") return os.homedir()
    if (name === "temp") return os.tmpdir()
    return os.homedir()
  },
}

// Minimal `safeStorage` — only reached in the oauth-store legacy migration
// path, gated on isEncryptionAvailable(). Reporting false skips the encrypted
// legacy blob (current tokens live plaintext in ~/.claude/.credentials.json,
// same as the CLI), so encrypt/decrypt are never actually called.
export const safeStorage = {
  isEncryptionAvailable(): boolean { return false },
  encryptString(s: string): Buffer { return Buffer.from(s, "utf8") },
  decryptString(b: Buffer): string { return b.toString("utf8") },
}

// Save-file dialog used by the ported `jsonl:download` handler. Mapped to the
// host's native save dialog.
export const dialog = {
  async showSaveDialog(
    _win: BrowserWindow | null,
    opts: { defaultPath?: string; filters?: Array<{ name: string; extensions: string[] }> },
  ): Promise<{ canceled: boolean; filePath?: string }> {
    const uri = await vscode.window.showSaveDialog({
      defaultUri: opts.defaultPath ? vscode.Uri.file(opts.defaultPath) : undefined,
      filters: opts.filters
        ? Object.fromEntries(opts.filters.map((f) => [f.name, f.extensions]))
        : undefined,
    })
    return { canceled: !uri, filePath: uri?.fsPath }
  },
  // Open dialog used by `plugins:install-local` (folder picker). Maps to the
  // host's native open dialog; canFolders/canFiles derived from `properties`.
  async showOpenDialog(
    opts: { properties?: string[]; title?: string; defaultPath?: string },
  ): Promise<{ canceled: boolean; filePaths: string[] }> {
    const props = opts.properties ?? []
    const uris = await vscode.window.showOpenDialog({
      canSelectFolders: props.includes("openDirectory"),
      canSelectFiles: props.includes("openFile") || !props.includes("openDirectory"),
      canSelectMany: props.includes("multiSelections"),
      title: opts.title,
      defaultUri: opts.defaultPath ? vscode.Uri.file(opts.defaultPath) : undefined,
    })
    return { canceled: !uris || uris.length === 0, filePaths: (uris ?? []).map((u) => u.fsPath) }
  },
}

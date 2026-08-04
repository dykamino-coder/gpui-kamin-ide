// Extra core IPC handlers + ConnectionManager lifecycle wiring for the VSIX
// bridge. The bulk of the session/pty/mcp surface comes from the vendored
// ipc/sessions.ts + ipc/config.ts + ipc/tabs.ts; this file fills the host-
// native handlers (dialogs, fs probes, open-external) the Electron app put in
// ipc/system.ts, and reproduces the slimmed setup-connection-callbacks (minus
// the external-MCP-server manager, a later phase).
import * as vscode from "vscode"
import fs from "fs"
import path from "path"
import os from "os"
import type { IpcMainInvokeEvent } from "./shim/electron"
import { ipcMain } from "./shim/electron"
import type { TabManager } from "./main/tab-manager"
import type { ConfigStore } from "./main/config/store"
import { ConnectionManager } from "./main/ws/connection-manager"
import { enrichTreeWithTabs } from "./main/tree-enrichment"
import { spawnToast, closeToastsMatching } from "./main/notifications/toast-window"
import { getDisplayedTab } from "./displayed-tab"

const MIME: Record<string, string> = {
  ".png": "image/png", ".jpg": "image/jpeg", ".jpeg": "image/jpeg",
  ".gif": "image/gif", ".webp": "image/webp", ".svg": "image/svg+xml",
}

export function registerCoreIpc(context: vscode.ExtensionContext): void {
  ipcMain.handle("get-version", () => String(context.extension.packageJSON.version ?? ""))
  ipcMain.handle("vscode:is-available", () => false)
  ipcMain.on("vscode:open", () => {})
  ipcMain.on("theme:set", () => {})

  // Clipboard via the host — the sandboxed webview (opaque origin) can't use
  // navigator.clipboard, so copy routes through vscode.env.clipboard here.
  ipcMain.handle("write-clipboard", async (_e: IpcMainInvokeEvent, text: string) => {
    await vscode.env.clipboard.writeText(String(text ?? ""))
    return true
  })
  // Paste: the reverse — read the OS clipboard through the host (native arboard,
  // no focus requirement) so terminal/chat paste works from inside the iframe.
  ipcMain.handle("read-clipboard", async (): Promise<string> => {
    return await vscode.env.clipboard.readText()
  })

  // Notifications go to KaminIDE's SHARED notification UI (not a Bridge-owned
  // toast overlay) — the extension shows them via vscode.window.show*Message.
  ipcMain.on("show-notification", (_e: unknown, severity: string, message: string) => {
    const m = String(message ?? "")
    if (!m) return
    if (severity === "error") void vscode.window.showErrorMessage(m)
    else if (severity === "warning") void vscode.window.showWarningMessage(m)
    else void vscode.window.showInformationMessage(m)
  })

  ipcMain.handle("open-external", async (_e: IpcMainInvokeEvent, url: string) => {
    try { await vscode.env.openExternal(vscode.Uri.parse(url)); return true } catch { return false }
  })

  ipcMain.handle("select-folder", async () => {
    const picked = await vscode.window.showOpenDialog({ canSelectFolders: true, canSelectFiles: false, canSelectMany: false })
    return picked?.[0]?.fsPath ?? null
  })

  // Attach-file picker (paperclip). The webview can't open a native dialog, so it
  // runs here (vscode.window.showOpenDialog); images are read to base64 for the
  // inline thumbnail. Returns the shape AttachButton expects. `open-image-dialog`
  // is the same picker limited to image types.
  const IMG_EXT = ["png", "jpg", "jpeg", "gif", "webp", "svg"]
  const pickAttachments = async (imagesOnly: boolean): Promise<Array<{ path: string; name: string; ext: string; base64: string; mimeType: string; isImage: boolean }>> => {
    const picked = await vscode.window.showOpenDialog({
      canSelectFiles: true, canSelectFolders: false, canSelectMany: true,
      ...(imagesOnly ? { filters: { Images: IMG_EXT } } : {}),
    })
    if (!picked) return []
    const out = []
    for (const uri of picked) {
      const p = uri.fsPath
      const ext = path.extname(p).replace(/^\./, "").toLowerCase()
      const isImage = IMG_EXT.includes(ext)
      let base64 = "", mimeType = ""
      if (isImage) {
        try { base64 = (await fs.promises.readFile(p)).toString("base64"); mimeType = MIME[`.${ext}`] ?? "application/octet-stream" } catch { /* unreadable — still attach by path */ }
      }
      out.push({ path: p, name: path.basename(p), ext, base64, mimeType, isImage })
    }
    return out
  }
  ipcMain.handle("open-file-dialog", () => pickAttachments(false))
  ipcMain.handle("open-image-dialog", () => pickAttachments(true))

  // Pasted / dropped clipboard image → temp file, returns its path (the send
  // path reads it back to base64 like any picked attachment). Ported from the
  // Electron app's ipc/system.ts; without it `bridge.saveClipboardImage` hit an
  // unimplemented channel (resolved null) and Ctrl+V of a screenshot silently
  // did nothing (the paste handler then threw on `null.split`).
  ipcMain.handle("save-clipboard-image", async (_e: IpcMainInvokeEvent, base64Data: string, mimeType: string) => {
    const ext = mimeType === "image/jpeg" ? ".jpg" : ".png"
    const filePath = path.join(os.tmpdir(), `claude-paste-${String(Date.now())}${ext}`)
    await fs.promises.writeFile(filePath, Buffer.from(base64Data, "base64"))
    return filePath
  })

  // Non-image drop/paste attachment: the sandboxed webview can't resolve a
  // dropped File's OS path (Electron's webUtils is gone), so it ships the
  // BYTES instead; we land them in temp under the original file name so the
  // CLI can reference a real path (audit #70 A3 — the old getDroppedFilePath
  // stub made these flows a silent no-op).
  ipcMain.handle("save-dropped-file", async (_e: IpcMainInvokeEvent, base64Data: string, name: string) => {
    // Strip any path separators from the client-supplied name — it must stay
    // a bare file name inside the temp drop dir.
    const safe = (name || "attachment").replace(/[/\\]/g, "_").slice(-120)
    const dir = path.join(os.tmpdir(), `claude-drop-${String(Date.now())}`)
    await fs.promises.mkdir(dir, { recursive: true })
    const filePath = path.join(dir, safe)
    await fs.promises.writeFile(filePath, Buffer.from(base64Data, "base64"))
    return filePath
  })

  const revealInOs = (_e: unknown, p: string): void => {
    // `.catch` (not just try/catch) — executeCommand rejects ASYNC, and an
    // unhandled rejection surfaces as a host "Extension crashed" toast.
    void vscode.commands.executeCommand("revealFileInOS", vscode.Uri.file(p)).then(undefined, () => { /* ignore */ })
  }
  ipcMain.on("open-folder", revealInOs)
  ipcMain.on("reveal-in-explorer", revealInOs)

  ipcMain.handle("fs:path-exists", (_e: IpcMainInvokeEvent, p: string) => {
    try { return fs.existsSync(p) } catch { return false }
  })

  ipcMain.handle("fs:read-base64", async (_e: IpcMainInvokeEvent, filePath: string) => {
    try {
      const buf = await fs.promises.readFile(filePath)
      const ext = path.extname(filePath).toLowerCase()
      return { ok: true, base64: buf.toString("base64"), mimeType: MIME[ext] ?? "application/octet-stream" }
    } catch (err) {
      return { ok: false, error: err instanceof Error ? err.message : String(err) }
    }
  })

  // The sandboxed webview can't fetch the Bridge server over http (CSP
  // `connect-src 'self' https:`). The gate's token creation + the version
  // probe run here in the ext-host (Node, no CSP) instead.
  ipcMain.handle("bridge:create-token", async (_e: IpcMainInvokeEvent, httpBase: string, name: string) => {
    const resp = await fetch(`${httpBase}/api/dashboard/tokens`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ name }),
    })
    if (!resp.ok) throw new Error(`HTTP ${resp.status}`)
    return await resp.json()
  })

  ipcMain.handle("bridge:server-version", async (_e: IpcMainInvokeEvent, httpBase: string) => {
    for (const path of ["/api/bridge-version", "/health"]) {
      try {
        const resp = await fetch(`${httpBase}${path}`, { signal: AbortSignal.timeout(8000) })
        if (resp.ok) return await resp.json()
      } catch { /* try next */ }
    }
    return null
  })

  // Generic Bridge-server fetch proxy (same CSP reason as above): the Settings
  // panel + ConnectButton manage dashboard tokens / streaming-settings over HTTP.
  // `path` is appended to the server's http base; method/body/headers pass through.
  // Returns a plain {ok,status,data} so the webview never holds a Response.
  ipcMain.handle("bridge:server-fetch", async (
    _e: IpcMainInvokeEvent,
    httpBase: string,
    path: string,
    init?: { method?: string; body?: string; headers?: Record<string, string> },
  ) => {
    try {
      const resp = await fetch(`${httpBase}${path}`, {
        method: init?.method ?? "GET",
        headers: init?.headers ?? (init?.body ? { "Content-Type": "application/json" } : undefined),
        body: init?.body,
        signal: AbortSignal.timeout(8000),
      })
      const text = await resp.text()
      let data: unknown = null
      try { data = text ? JSON.parse(text) : null } catch { data = text }
      return { ok: resp.ok, status: resp.status, data }
    } catch (err) {
      return { ok: false, status: 0, data: null, error: err instanceof Error ? err.message : String(err) }
    }
  })

  ipcMain.handle("save-text-as", async (_e: IpcMainInvokeEvent, opts: { text: string; defaultName?: string }) => {
    try {
      const uri = await vscode.window.showSaveDialog({
        defaultUri: opts.defaultName ? vscode.Uri.file(opts.defaultName) : undefined,
      })
      if (!uri) return { ok: false, error: "Cancelled" }
      await fs.promises.writeFile(uri.fsPath, opts.text ?? "", "utf8")
      return { ok: true, filePath: uri.fsPath }
    } catch (err) {
      return { ok: false, error: err instanceof Error ? err.message : String(err) }
    }
  })
}

/** Slimmed setup-connection-callbacks: tree enrichment, saved-session
 *  persistence, and elicitation/idle toasts. External-MCP-tool registration is
 *  deferred to a later phase. */
export function wireConnectionCallbacks(tabManager: TabManager, configStore: ConfigStore): void {
  ConnectionManager.enrichTree = (tree: any[]) => {
    const enriched = enrichTreeWithTabs(tree, tabManager)
    tree.length = 0
    tree.push(...enriched)
  }

  ConnectionManager.onSessionInfo = (tabId, effort, model) => {
    tabManager.updateTabSessionInfo(tabId, effort, model)
  }

  ConnectionManager.onSessionTitle = (tabId, title) => {
    tabManager.updateTabSessionTitle(tabId, title)
    const tab = tabManager.getTab(tabId)
    if (tab?.conversationId) {
      configStore.addSavedSession({
        conversationId: tab.conversationId, cwd: tab.config.cwd || "", label: title || tab.label,
        folderName: tab.folderName, model: tab.model || "", lastActivity: new Date().toISOString(), messageCount: 0,
      })
    }
  }

  ConnectionManager.onConversationId = (tabId, conversationId) => {
    const tab = tabManager.getTab(tabId)
    if (!tab) return
    tab.conversationId = conversationId
    configStore.addSavedSession({
      conversationId, cwd: tab.config.cwd || "", label: tab.sessionTitle || tab.label,
      folderName: tab.folderName, model: tab.model || "", lastActivity: new Date().toISOString(), messageCount: 0,
    })
    tabManager.notifyTabListChangedPublic()
  }

  ConnectionManager.onElicitationRequest = (tabId, { requestId, toolName, message }) => {
    spawnToast({ kind: "question", tabId, requestId, title: `${toolName} needs your answer`, message: (message || "").slice(0, 240), sticky: true })
  }

  ConnectionManager.onElicitationResolved = (tabId, requestId) => {
    closeToastsMatching((o) => o.kind === "question" && o.requestId === requestId && o.tabId === tabId)
  }

  ConnectionManager.onSessionIdle = (tabId, rawTitle) => {
    // The user is LOOKING at this very chat (it is the active tab AND the
    // viewer confirmed it is what's on screen) — a "finished" toast tells them
    // nothing they aren't watching happen. Only notify about a session the
    // user is away from: another tab, or a chat that isn't showing.
    if (tabManager.getActiveTabId() === tabId && getDisplayedTab() === tabId) return
    const tab = tabManager.getTab(tabId)
    const label = tab?.sessionTitle || tab?.label || rawTitle || "Session"
    spawnToast({ kind: "finished", tabId, title: "Session finished", message: `Tab «${label}» is ready.` })
  }
}

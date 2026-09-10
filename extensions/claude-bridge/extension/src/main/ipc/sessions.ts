import { ipcMain, type BrowserWindow, type IpcMainEvent, type IpcMainInvokeEvent } from '@kaminide/host-compat'
import fs from 'fs'
import path from 'path'
import type { TabManager } from '../tab-manager'
import type { ConfigStore } from '../config/store'
import { ConnectionManager } from '../ws/connection-manager'
import { updateUiToolsActiveTab } from '../mcp/tools/ui-tools'
import { activateSessionForTab } from '../../session-activation'
import { reportDisplayedTab, forgetDisplayedTab } from '../../displayed-tab'
import type { ConnectionConfig, PermissionDecision } from '../../shared/types'

export interface SessionsIpcContext {
  configStore: ConfigStore
  getTabManager: () => TabManager | null
  getMainWindow: () => BrowserWindow | null
  getUserCwd: () => string | null
}

export function registerSessionsIPC(ctx: SessionsIpcContext): void {
  const tm = () => ctx.getTabManager()

  // ─── Tabs ──────────────────────────────────────────
  ipcMain.handle('tab:create', (_event: IpcMainInvokeEvent, config: ConnectionConfig) => {
    const userCwd = ctx.getUserCwd()
    if (!('cwd' in config) && userCwd) {
      config.cwd = userCwd
    }
    ctx.configStore.set({ ...ctx.configStore.get(), serverUrl: config.serverUrl, token: config.token })
    const tabId = tm()?.createTab(config) ?? null
    if (tabId) updateUiToolsActiveTab(tabId)
    return tabId
  })

  ipcMain.on('tab:close', (_event: IpcMainEvent, tabId: string) => {
    forgetDisplayedTab(tabId) // else the gap check compares against a dead tab
    tm()?.closeTab(tabId)
    updateUiToolsActiveTab(tm()?.getActiveTabId() ?? null)
  })

  // A switch made in the webview's own tab strip has to move the HOST's active
  // session too. It used to only move the Bridge tab, so the chat showed one
  // session while the session list, titlebar and every other host surface stayed
  // highlighting the one the user had just left — with nothing anywhere that
  // could detect or repair the split, since `setActiveSession` early-returns on
  // a repeat click and so re-selecting the session could not resync it either.
  //
  // No loop: the host's onActive calls `TabManager.switchTab` directly rather
  // than coming back through this channel, and `setActiveSession` is a no-op
  // when the session is already active.
  ipcMain.on('tab:switch', (_event: IpcMainEvent, tabId: string) => {
    tm()?.switchTab(tabId)
    updateUiToolsActiveTab(tabId)
    activateSessionForTab(tabId)
  })

  // The chat reporting what it has on screen. See displayed-tab.ts — a fact,
  // not a handshake: nothing waits on it.
  ipcMain.on('chat:bound', (_event: IpcMainEvent, tabId: string) => {
    if (typeof tabId === 'string' && tabId) reportDisplayedTab(tabId)
  })

  ipcMain.handle('tab:list', () => {
    return tm()?.listTabs() ?? []
  })

  ipcMain.handle('tab:get-active', () => {
    return tm()?.getActiveTabId() ?? null
  })

  // ─── Connection (legacy) ──────
  ipcMain.on('connect', (_event: IpcMainEvent, config: ConnectionConfig) => {
    const userCwd = ctx.getUserCwd()
    if (!('cwd' in config) && userCwd) {
      config.cwd = userCwd
    }
    ctx.configStore.set({ ...ctx.configStore.get(), serverUrl: config.serverUrl, token: config.token })
    tm()?.createTab(config)
  })

  ipcMain.on('disconnect', (_event: IpcMainEvent) => {
    tm()?.disconnectAll()
  })

  ipcMain.on('tab:disconnect', (_event: IpcMainEvent, tabId: string) => {
    tm()?.disconnectTab(tabId)
  })

  ipcMain.on('tab:reconnect', (_event: IpcMainEvent, tabId: string) => {
    tm()?.reconnectTab(tabId)
  })

  ipcMain.handle('get-connection-state', () => {
    const mgr = tm()
    const activeId = mgr?.getActiveTabId()
    if (!activeId) return { status: 'disconnected', authority: 'missing', authorityGeneration: 0, authoritySequence: 0, revision: 0 }
    return mgr?.getConnectionState(activeId) ?? { status: 'disconnected', authority: 'missing', authorityGeneration: 0, authoritySequence: 0, revision: 0 }
  })

  // ─── PTY / Terminal (per-tab) ──────────────────────
  ipcMain.on('session:interrupt', (_event: IpcMainEvent, tabId: string) => {
    tm()?.interruptSession(tabId)
  })

  ipcMain.on('pty-input', (_event: IpcMainEvent, tabId: string, data: string) => {
    tm()?.sendInput(tabId, data)
  })

  ipcMain.on('pty-submit-text', (_event: IpcMainEvent, tabId: string, text: string) => {
    tm()?.submitText(tabId, text)
  })

  ipcMain.on('pty-resize', (_event: IpcMainEvent, tabId: string, cols: number, rows: number) => {
    tm()?.resize(tabId, cols, rows)
  })

  // ─── MCP permissions / responses ────────────────────
  ipcMain.on('permission-response', (_event: IpcMainEvent, requestId: string, decision: PermissionDecision) => {
    tm()?.respondPermission(requestId, decision)
  })

  ipcMain.on('mcp-respond', (_event: IpcMainEvent, requestId: string, result: unknown) => {
    tm()?.respondMcp(requestId, result)
  })

  ipcMain.on('mcp-deny', (_event: IpcMainEvent, requestId: string, reason: string) => {
    tm()?.denyMcp(requestId, reason)
  })

  // ─── Elicitation ──────────────────────────────────
  ipcMain.on('elicitation-response', (_event: IpcMainEvent, requestId: string, action: string, content?: Record<string, unknown>) => {
    tm()?.respondElicitation(requestId, action as 'accept' | 'deny' | 'dismiss', content)
  })

  // ─── Permission mode / effort / model ─────────────
  ipcMain.on('change-permission-mode', (_event: IpcMainEvent, tabId: string, mode: string) => {
    const conn = tm()?.getConnection(tabId)
    if (conn) conn.setPermissionMode(mode as any)
  })

  ipcMain.on('change-effort', (_event: IpcMainEvent, tabId: string, effort: string) => {
    const conn = tm()?.getConnection(tabId)
    if (conn) conn.changeEffort(effort)
  })

  ipcMain.on('change-model', (_event: IpcMainEvent, tabId: string, model: string) => {
    const conn = tm()?.getConnection(tabId)
    if (conn) conn.changeModel(model)
  })

  ipcMain.handle('get-tab-model', (_event: IpcMainInvokeEvent, tabId: string) => {
    const conn = tm()?.getConnection(tabId)
    return conn?.getLastModel() ?? null
  })

  // ─── Session Resume ───────────────────────────────
  ipcMain.handle('session:resume', async (_event: IpcMainInvokeEvent, config: ConnectionConfig & { conversationId: string }) => {
    const userCwd = ctx.getUserCwd()
    if (!config.cwd && userCwd) {
      config.cwd = userCwd
    }
    ctx.configStore.set({ ...ctx.configStore.get(), serverUrl: config.serverUrl, token: config.token })
    return tm()?.resumeSession(config) ?? null
  })

  ipcMain.handle('session:get-saved', () => {
    return ctx.configStore.getSavedSessions()
  })

  ipcMain.handle('session:remove-saved', (_event: IpcMainInvokeEvent, conversationId: string) => {
    ctx.configStore.removeSavedSession(conversationId)
  })

  ipcMain.handle('session:delete-data', (_event: IpcMainInvokeEvent, conversationId: string) => {
    const conn = tm()?.getFirstConnection()
    if (conn) conn.sendRaw({ type: 'session:delete-data', conversationId })
  })

  // ─── Session Tree (re-send cached tree on renderer request) ───────────────
  // БЕЗ реплея JSONL: канал зовётся из useInit КАЖДОЙ страницы (включая все
  // customize-настройки) и реплей ВСЕХ табов здесь давал ~150 чанков × все
  // вью — «каждая страница настроек грузится 10 секунд» + гигабайты
  // транзиентных строк в хосте. Реплей живёт на своём канале
  // jsonl:request-replay, который зовут только панели, которым он нужен.
  ipcMain.on('tree:request', (event) => {
    if (ConnectionManager.lastTree) {
      if (ConnectionManager.enrichTree) {
        ConnectionManager.enrichTree(ConnectionManager.lastTree as any[])
      }
      event.sender.send('tree-update', ConnectionManager.lastTree)
    }
  })

  // ─── JSONL Replay (after renderer reload) ────────────────
  // A `tabId` targets ONE session. The renderer drops background tabs' entries
  // to keep the shared WebView2 renderer under its memory ceiling and refills
  // the tab it activates — replaying every open session for that would re-push
  // the very transcripts eviction just freed. No tabId = the reload case, where
  // the renderer really did lose everything.
  ipcMain.on('jsonl:request-replay', (_event: unknown, tabId?: string) => {
    const mgr = tm()
    if (!mgr) return
    const ids = typeof tabId === 'string' && tabId ? [tabId] : mgr.listTabs().map((t) => t.id)
    for (const id of ids) {
      const conn = mgr.getConnection(id)
      if (!conn) {
        // The webview asked for a tab this run does not have (a stale id from a
        // previous app run, or a tab torn down mid-request). Silence here left
        // the chat's load-cover spinning forever — answer with an explicit
        // "nothing will come" so the viewer settles into the empty state.
        const win = ctx.getMainWindow()
        if (win && !win.isDestroyed()) {
          win.webContents.send('jsonl-status', id, { status: 'watching', replayComplete: true, unknownTab: true })
        }
        continue
      }
      // Paint from the local copy FIRST — it is a disk read, not a download —
      // then let the in-memory replay land on top. The renderer dedups on uuid,
      // so the overlap is free; what the user gets is a chat that is already
      // there instead of a spinner. An empty or untrusted mirror simply serves
      // nothing and the path below is unchanged.
      void conn.replayFromMirror().then(() => { conn.replayJsonlToRenderer() })
        .catch(() => { conn.replayJsonlToRenderer() })
    }
  })

  // ─── JSONL older-page (scroll-up for the windowed store) ───────────────
  // The webview holds only a recent window; when the reader scrolls to the top
  // it asks for the page just before its oldest held byte offset. Served from
  // the local mirror (disk, not network), delivered on `jsonl:older-page`.
  ipcMain.on('jsonl:load-older', (_event: IpcMainEvent, tabId: string, beforePos: number, count: number) => {
    const conn = tm()?.getConnection(tabId)
    if (conn) void conn.loadOlderPage(beforePos, count)
  })

  // Full compact-boundary index (all segments, from the disk mirror), delivered
  // on `jsonl:boundaries-result`.
  ipcMain.on('jsonl:boundaries', (_event: IpcMainEvent, tabId: string) => {
    const conn = tm()?.getConnection(tabId)
    if (conn) void conn.loadBoundaries()
  })

  // Load one archived compact segment by timestamp range, delivered on
  // `jsonl:segment-page`; the renderer replaces its window with it.
  ipcMain.on('jsonl:load-segment', (_event: IpcMainEvent, tabId: string, fromTs: string, toTs: string) => {
    const conn = tm()?.getConnection(tabId)
    if (conn) void conn.loadSegment(fromTs, toTs)
  })

  // Применения тула для инлайн-просмотра в панели Tools (панельный стор —
  // только plan/todo-срез, полные записи живут в кэше хоста).
  ipcMain.handle('toolusage:entries', (_event: IpcMainInvokeEvent, tabId: string, toolName: string) => {
    const conn = tm()?.getConnection(tabId)
    return conn && typeof toolName === 'string' ? conn.toolUsageEntries(toolName) : []
  })

  // ─── JSONL Download ─────────────────────────────────────
  ipcMain.handle('jsonl:download', async (_event: IpcMainInvokeEvent, tabId: string) => {
    const conn = tm()?.getConnection(tabId)
    if (!conn) return { success: false, error: 'No connection' }

    // ASK WHERE FIRST, fetch second. The transcript used to be pulled over the
    // WS before the dialog opened — on a big session that's megabytes and
    // SECONDS during which the button looks dead and no save window appears,
    // and if the user then cancelled, the whole transfer was wasted. The
    // server names the file `basename(jsonlPath)` = `<conversationId>.jsonl`,
    // which we already know locally, so the dialog needs nothing from it.
    const { dialog } = await import('@kaminide/host-compat')
    const win = ctx.getMainWindow()
    if (!win) return { success: false, error: 'No main window' }
    const convId = conn.getConversationId()
    const { canceled, filePath } = await dialog.showSaveDialog(win, {
      defaultPath: convId ? `${convId}.jsonl` : 'session.jsonl',
      filters: [{ name: 'JSONL', extensions: ['jsonl'] }],
    })
    if (canceled || !filePath) return { success: false, error: 'Cancelled' }

    const result = await conn.requestJsonlDownload()
    if (result.error || !result.content) {
      return { success: false, error: result.error || 'No content' }
    }

    // Async write: a big transcript is megabytes, and writeFileSync blocked the
    // extension host's event loop for the whole flush.
    await fs.promises.writeFile(filePath, result.content, 'utf-8')
    return { success: true, filePath }
  })

  // ─── Bulk export: every transcript for the current token → a chosen folder ──
  // Pull each file in byte-range BATCHES (not one WS message like jsonl:download,
  // which times out on a big transcript) straight from the server's HTTP API,
  // which scopes the listing + every range to the caller's own token.
  ipcMain.handle('jsonl:download-all', async () => {
    const cfg = ctx.configStore.get()
    if (!cfg.token) return { success: false, error: 'No token configured' }
    // The server URL is a ws(s):// endpoint; the export API is HTTP on the same host.
    const httpBase = cfg.serverUrl.replace(/^ws(s?):\/\//i, 'http$1://').replace(/\/+$/, '')
    const headers = { Authorization: `Bearer ${cfg.token}` }

    const { dialog } = await import('@kaminide/host-compat')
    const win = ctx.getMainWindow()
    if (!win) return { success: false, error: 'No main window' }
    const picked = await dialog.showOpenDialog({
      title: 'Choose a folder for the exported session logs',
      properties: ['openDirectory', 'createDirectory'],
    })
    if (picked.canceled || !picked.filePaths[0]) return { success: false, error: 'Cancelled' }
    const dir = picked.filePaths[0]

    let transcripts: { conversationId: string; size: number }[]
    try {
      const res = await fetch(`${httpBase}/api/dashboard/transcripts`, { headers })
      if (!res.ok) return { success: false, error: `List failed (HTTP ${String(res.status)})` }
      transcripts = ((await res.json()) as { transcripts?: { conversationId: string; size: number }[] }).transcripts ?? []
    } catch (err) {
      return { success: false, error: `List failed: ${err instanceof Error ? err.message : String(err)}` }
    }
    if (transcripts.length === 0) return { success: true, dir, count: 0 }

    const BATCH = 4 * 1024 * 1024 // 4MB per range request — well under the server cap
    let done = 0
    const errors: string[] = []
    for (const t of transcripts) {
      // conversationId is a CLI-minted UUID (the JSONL basename) — no path
      // separators — but basename() defends the write path regardless.
      const outPath = path.join(dir, `${path.basename(t.conversationId)}.jsonl`)
      try {
        const fh = await fs.promises.open(outPath, 'w')
        try {
          let offset = 0
          // size can be stale (the file may have grown); loop until a short/empty
          // range says we've reached the end, not just until the listed size.
          for (;;) {
            const url = `${httpBase}/api/dashboard/transcripts/${encodeURIComponent(t.conversationId)}?offset=${String(offset)}&len=${String(BATCH)}`
            const res = await fetch(url, { headers })
            if (!res.ok) throw new Error(`HTTP ${String(res.status)} at offset ${String(offset)}`)
            const chunk = Buffer.from(await res.arrayBuffer())
            if (chunk.length === 0) break
            await fh.write(chunk)
            offset += chunk.length
            win.webContents.send('jsonl:download-all-progress', {
              fileIndex: done, fileTotal: transcripts.length, conversationId: t.conversationId, bytes: offset, size: t.size,
            })
            if (chunk.length < BATCH) break // last (short) range
          }
        } finally {
          await fh.close()
        }
      } catch (err) {
        errors.push(`${t.conversationId}: ${err instanceof Error ? err.message : String(err)}`)
      }
      done++
      win.webContents.send('jsonl:download-all-progress', {
        fileIndex: done, fileTotal: transcripts.length, conversationId: t.conversationId, bytes: t.size, size: t.size,
      })
    }
    if (errors.length > 0) {
      return { success: false, dir, count: done - errors.length, error: `${String(errors.length)}/${String(transcripts.length)} failed: ${errors.slice(0, 3).join('; ')}` }
    }
    return { success: true, dir, count: transcripts.length }
  })

  // ─── Session diagnostic → file ──────────────────────────
  // The webview collects the dump (it owns the chat store) and hands it here to
  // land on disk. It used to go to the CLIPBOARD, which is the wrong medium for
  // this: a big session's dump is megabytes of JSON — awkward to paste, easy to
  // truncate, and gone the moment anything else is copied. A file can just be
  // attached to a report.
  ipcMain.handle('diag:save', async (_event: IpcMainInvokeEvent, fileName: string, content: string) => {
    const { dialog } = await import('@kaminide/host-compat')
    const win = ctx.getMainWindow()
    if (!win) return { success: false, error: 'No main window' }
    const { canceled, filePath } = await dialog.showSaveDialog(win, {
      defaultPath: fileName,
      filters: [{ name: 'JSON', extensions: ['json'] }],
    })
    if (canceled || !filePath) return { success: false, error: 'Cancelled' }
    await fs.promises.writeFile(filePath, content, 'utf-8')
    return { success: true, filePath }
  })
}

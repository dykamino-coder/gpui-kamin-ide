// Service registration for kamin-host — fills the shared method table
// (served to BOTH the native shell's stdio RPC link and runtime WebSocket
// clients) and orchestrates the workspace lifecycle (watch + index
// follow the open folder). Split from `kamin-host.ts` so the entry
// stays a thin boot file.
import { existsSync, readFileSync } from "node:fs"
import { join } from "node:path"
import { JsonStore } from "../json-store.js"
import { HOST_WORKSPACE_SET } from "../protocol.js"
import { initAppPrefs, getAppPrefs, setAppPrefs } from "./app-prefs.js"
import * as config from "./config.js"
import * as documents from "./documents.js"
import * as editors from "./editors.js"
import * as fsClipboard from "./file-clipboard.js"
import * as fileIndex from "./file-index.js"
import * as fsIo from "./file-io.js"
import * as iconTheme from "./icon-theme.js"
import * as ptySvc from "./pty.js"
import * as search from "./search.js"
import * as sessions from "./sessions.js"
import { discoverShells } from "./shells.js"
import * as storage from "./storage.js"
import * as watcher from "./watcher.js"
import * as workspace from "./workspace.js"

export type Handler = (...params: unknown[]) => unknown

/** The Claude Bridge extension's id — its global storage holds the configured
 *  server URL that KaminIDE's self-updater points at. */
const BRIDGE_EXT_ID = "dykamino-studio.claude-bridge"

/** Read the Claude Bridge extension's configured server URL from its global
 *  storage (written by the extension's ConfigStore). Returns null when the
 *  extension hasn't been configured yet, so the renderer's updater falls back
 *  to the default localhost server. This lives host-side because only the host
 *  knows the data dir; reading the file directly (rather than via the ext-host)
 *  keeps it available even under crash isolation, where the child owns config. */
function readBridgeServerUrl(dataDir: string): string | null {
  try {
    const file = join(dataDir, "globalStorage", BRIDGE_EXT_ID, "open-claude-bridge-config.json")
    // The extension's ConfigStore persists `{ config: { serverUrl, token }, … }` —
    // the URL is NESTED under `config`, not top-level. Reading it flat always
    // yielded undefined → null → the renderer silently fell back to
    // http://localhost:3456, so a REMOTE server never got its update check (the
    // bug hid locally, where the fallback happens to be the real server).
    // The flat read stays as a fallback for any legacy/flat file.
    const raw = JSON.parse(readFileSync(file, "utf8")) as {
      config?: { serverUrl?: unknown }
      serverUrl?: unknown
    }
    const url = raw.config?.serverUrl ?? raw.serverUrl
    return typeof url === "string" && url ? url : null
  } catch {
    return null
  }
}

export interface ServiceBootOptions {
  dataDir: string
  /** LOCAL app data — rebuildable caches (file index) live here, never in
   *  the roaming profile. Falls back to dataDir on an older shell. */
  cacheDir?: string
  legacyWorkspacePath: string | null
  /** Folder passed by Explorer "Open with KaminIDE" (`--open-folder`). When
   *  set + existing, it overrides the persisted workspace for this launch. */
  openFolderPath?: string | null
  /** Fire-and-forget event toward the renderer (WS broadcast). */
  broadcast: (channel: string, payload: unknown) => void
  /** Default true. When false (ext-host runs in a forked CHILD for crash
   *  isolation) the parent does NOT init the services that only back extensions
   *  — config, storage, documents, editors — nor register their renderer
   *  methods; the child owns them and the renderer's doc/editor sync is
   *  forwarded to it. Everything the renderer also needs (fs/index/pty/sessions/
   *  workspace) stays here. */
  ownExtHostServices?: boolean
}

/** Отложенный старт слежения: даже с `ignoreInitial` и глубиной 4 chokidar
 *  ОБХОДИТ дерево, чтобы расставить наблюдатели, и на профильном корне это
 *  непрерывная CPU-работа. Замер: 16 пауз цикла по 400-670 мс подряд с 1.1 с
 *  до 8 с от старта — за ними стояли ВСЕ ответы хоста («0 active», «панелей
 *  нет»). В первые секунды слежение не нужно: дерево читается по запросу. */
const WATCH_START_DELAY_MS = 4000
let watchTimer: NodeJS.Timeout | undefined

async function onWorkspaceChanged(path: string | null): Promise<void> {
  if (path) {
    if (watchTimer) clearTimeout(watchTimer)
    watchTimer = setTimeout(() => {
      watchTimer = undefined
      watcher.watchWorkspace(path)
    }, WATCH_START_DELAY_MS)
    watchTimer.unref()
    await fileIndex.ensureIndex(path)
  } else {
    if (watchTimer) { clearTimeout(watchTimer); watchTimer = undefined }
    watcher.stopWorkspaceWatch()
    fileIndex.clearIndex()
  }
}

export function buildServiceMethods(opts: ServiceBootOptions): Map<string, Handler> {
  const { broadcast } = opts
  const ownExtHostServices = opts.ownExtHostServices ?? true
  fileIndex.initIndexCacheDir(opts.cacheDir ?? opts.dataDir)
  initAppPrefs(opts.dataDir)
  const initial = workspace.initWorkspace(opts.dataDir, opts.legacyWorkspacePath, opts.openFolderPath)
  // Config (B2) + extension persistence (B9) only back the extension host, so
  // they live with it: in-process by default, or in the forked child (which
  // inits them itself) under crash isolation.
  if (ownExtHostServices) {
    config.initConfig(opts.dataDir)
    storage.initStorage(opts.dataDir)
  }

  // Watcher feeds the index in-process (same tick, no RPC) and the
  // renderer over the WS broadcast.
  watcher.setIndexSink((batch) => {
    // One rebuild per batch — a `rm -rf`-sized flood of unlink events used to
    // trigger a full O(n) key rebuild PER event (O(n²)) and freeze the host.
    void fileIndex.applyFsEventBatch(batch)
  })
  watcher.setRendererSink((batch) => { broadcast("kamin:fs:event", batch) })

  ptySvc.initPtyEvents({
    onData: (ptyId, data) => { broadcast("kamin:pty:data", { ptyId, data }) },
    onExit: (ptyId, code, signal) => { broadcast("kamin:pty:exit", { ptyId, code, signal }) },
  })

  // Prime watcher+index for a folder restored from disk, before the
  // renderer's first Ctrl+P.
  if (initial.path) void onWorkspaceChanged(initial.path)

  // Single chokepoint for changing the open folder: persist + broadcast +
  // watch/index, no matter who triggered it (user picker, folder close, OR a
  // session switch below). The broadcast goes FIRST so the renderer's tree
  // re-roots instantly — it lists each folder lazily via kamin:fs:listDir and
  // never needs the file index. Building the index (which backs only Ctrl+P /
  // search / findFiles) is deferred to the background, so opening a session in
  // a huge folder doesn't freeze the UI behind a full tree walk.
  const applyWorkspaceFolder = (path: string | null): void => {
    // A session can point at a folder that was since deleted/moved. Without this,
    // setWorkspaceFolder's existsSync guard would silently KEEP the previous
    // folder and re-broadcast it — leaving the tree showing the PREVIOUS
    // session's files after switching. Coerce a missing path to null so the tree
    // clears to the empty state instead of lying with stale contents.
    const target = path !== null && !existsSync(path) ? null : path
    const next = workspace.setWorkspaceFolder(target)
    broadcast("kamin:workspace:changed", next.path)
    if (next.path) {
      // Surface the background index build so the user sees Ctrl+P / search is
      // still warming up (the renderer debounces, so a cached folder won't
      // flash the indicator).
      broadcast("kamin:index:status", { indexing: true })
      void onWorkspaceChanged(next.path).finally(() => { broadcast("kamin:index:status", { indexing: false }) })
    } else {
      void onWorkspaceChanged(next.path)
    }
  }

  // Sessions/Projects (Phase 2). Switching the active session re-roots the tree
  // to its project folder via the chokepoint above.
  sessions.initSessions(opts.dataDir, broadcast)
  sessions.onActiveSessionChange(() => { applyWorkspaceFolder(sessions.getActiveProjectFolder()) })
  // Boot: if a session was active last run, follow its project folder — UNLESS
  // this launch carried an explicit folder (Explorer "Open with KaminIDE").
  // That explicit intent, already applied by initWorkspace above, must win over
  // the restored session, otherwise right-clicking a folder would still open
  // whatever was open last time.
  const explicitOpen = !!(opts.openFolderPath && initial.path)
  const activeFolder = sessions.getActiveProjectFolder()
  if (activeFolder && !explicitOpen) applyWorkspaceFolder(activeFolder)

  const methods = new Map<string, Handler>()

  methods.set("kamin:workspace:get", () => workspace.getWorkspaceFolder())
  // Self-updater: the Rust `updater_check`/`updater_install` commands need the
  // Bridge server URL to build the update endpoint; only the host knows the
  // data dir where the extension stored it.
  methods.set("kamin:bridge:serverUrl", () => readBridgeServerUrl(opts.dataDir))
  // Native app preferences (background toasts, ConPTY, delete confirmation)
  // — see app-prefs.ts.
  methods.set("kamin:prefs:get", () => getAppPrefs())
  methods.set("kamin:prefs:set", (patch) => {
    const next = setAppPrefs(patch as Partial<ReturnType<typeof getAppPrefs>>)
    broadcast("kamin:prefs:changed", next) // renderer re-reads (toast gate) live
    return next
  })
  methods.set(HOST_WORKSPACE_SET, (path) => {
    applyWorkspaceFolder(path as string | null)
    return workspace.getWorkspaceFolder()
  })
  methods.set("kamin:workspace:close", () => {
    applyWorkspaceFolder(null)
    return workspace.getWorkspaceFolder()
  })

  // Sessions/Projects RPC.
  methods.set("kamin:sessions:list", () => sessions.listSessions())
  methods.set("kamin:sessions:newSessionInFolder", (folderPath) => sessions.newSessionInFolder(folderPath as string))
  methods.set("kamin:sessions:newSession", (projectId, name) => sessions.newSession(projectId as string | undefined, name as string | undefined))
  methods.set("kamin:sessions:newNoFolderSession", () => sessions.newNoFolderSession())
  methods.set("kamin:sessions:rename", (id, name) => sessions.renameSession(id as string, name as string))
  methods.set("kamin:sessions:setColor", (id, color) => sessions.setSessionColor(id as string, color as string | null))
  methods.set("kamin:sessions:setPinned", (id, pinned) => sessions.setSessionPinned(id as string, pinned as boolean))
  methods.set("kamin:sessions:delete", (id) => sessions.deleteSession(id as string))
  methods.set("kamin:sessions:deleteProject", (projectId) => sessions.deleteProject(projectId as string))
  methods.set("kamin:sessions:setActive", (id) => sessions.setActiveSession(id as string | null))
  methods.set("kamin:sessions:deactivate", (id) => sessions.deactivateSession(id as string))
  methods.set("kamin:sessions:reorder", (id, beforeId) => sessions.moveSessionBefore(id as string, beforeId as string | null))
  methods.set("kamin:sessions:setState", (id, st) => sessions.setSessionState(id as string, st as Parameters<typeof sessions.setSessionState>[1]))
  // Синхронный сброс JsonStore на диск: шелл зовёт на выходе — Job Object
  // убивает node раньше 200мс-дебаунса, и последний лейаут сессии терялся.
  methods.set("kamin:sessions:flush", () => { JsonStore.flushAllSync(); return true })
  methods.set("kamin:sessions:update", (id, patch) => sessions.updateSession(id as string, patch as { name?: string; metadata?: Record<string, unknown> }))
  // Legacy Electron Bridge cleanup: re-import saved sessions before the shell
  // deletes the old app's config (guarantees nothing is lost).
  methods.set("kamin:bridge:reimportSessions", () => sessions.reimportBridgeSessions())

  methods.set("kamin:fs:listDir", (abs) => fsIo.listDir(abs as string))
  methods.set("kamin:fs:readText", (abs) => fsIo.readText(abs as string))
  methods.set("kamin:fs:writeText", (abs, content) => fsIo.writeText(abs as string, content as string))
  methods.set("kamin:fs:mkdir", (abs) => fsIo.makeDir(abs as string))
  methods.set("kamin:fs:delete", (abs) => fsIo.deletePath(abs as string))
  methods.set("kamin:fs:trash", (abs) => fsIo.trashPath(abs as string))
  methods.set("kamin:fs:restoreTrash", (abs) => fsIo.restoreFromTrash(abs as string))
  methods.set("kamin:fs:revealInOS", (abs) => fsIo.revealInOS(abs as string))
  methods.set("kamin:fs:openExternal", (abs) => fsIo.openExternal(abs as string))
  methods.set("kamin:fs:openTerminal", (abs) => fsIo.openTerminalAt(abs as string))
  methods.set("kamin:fs:move", (src, dst) => fsIo.movePath(src as string, dst as string))
  methods.set("kamin:fs:copy", (src, dst) => fsIo.copyPath(src as string, dst as string, false))
  methods.set("kamin:fs:exists", (abs) => fsIo.pathExists(abs as string))
  methods.set("kamin:fs:clipboardWrite", (paths, cut) => fsClipboard.clipboardWriteFiles(paths as string[], cut as boolean))
  methods.set("kamin:fs:clipboardRead", () => fsClipboard.clipboardReadFiles())
  methods.set("kamin:iconTheme:load", (jsonPath) => iconTheme.loadIconThemeDoc(jsonPath as string))
  methods.set("kamin:iconTheme:icon", (abs) => iconTheme.readIconSvg(abs as string))

  // Document + active-editor mirror sync (B5/B5b) — renderer's Monaco reports
  // open editors. In-process only; under crash isolation these channels are
  // forwarded to the child (which owns documents/editors) by host-main.
  if (ownExtHostServices) {
    methods.set("kamin:doc:open", (doc) => { documents.syncDocOpen(doc as documents.DocState) })
    methods.set("kamin:doc:change", (uri, changes, version) => { documents.syncDocChange(uri as string, changes as documents.DocChange[], version as number) })
    methods.set("kamin:doc:setLanguage", (uri, lang) => { documents.syncDocSetLanguage(uri as string, lang as string) })
    methods.set("kamin:doc:close", (uri) => { documents.syncDocClose(uri as string); editors.dropEditor(uri as string) })
    methods.set("kamin:doc:save", (uri) => { documents.syncDocSave(uri as string) })
    methods.set("kamin:editor:active", (uri) => { editors.setActiveEditor(uri as string | null) })
    methods.set("kamin:editor:selections", (uri, sels) => { editors.setEditorSelections(uri as string, sels as editors.HostSelection[]) })
  }

  methods.set("kamin:index:findFile", (q) => search.findFile(q as string))
  methods.set("kamin:index:findInFiles", async (q) => await search.findInFiles(q as string))

  methods.set("kamin:shells:list", () => discoverShells())

  methods.set("kamin:pty:create", (opts2) => {
    const o = opts2 as ptySvc.PtyCreateOpts
    return ptySvc.createSession({ ...o, cwd: o.cwd ?? workspace.getWorkspaceFolder().path })
  })
  methods.set("kamin:pty:write", (ptyId, data) => { ptySvc.writeToSession(ptyId as string, data as string) })
  methods.set("kamin:pty:resize", (ptyId, cols, rows) => { ptySvc.resizeSession(ptyId as string, cols as number, rows as number) })
  methods.set("kamin:pty:dispose", (ptyId) => { ptySvc.disposeSession(ptyId as string) })

  return methods
}

/** App-quit path — graceful teardown of every live PTY. */
export function disposeServices(): void {
  ptySvc.disposeAllSessions()
  watcher.stopWorkspaceWatch()
}

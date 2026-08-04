// Filesystem watcher. Windows uses ONE native recursive `fs.watch` on the
// workspace root; other platforms (and `KAMIN_WATCHER=chokidar`) keep the
// chokidar wrapper.
//
// WHY the split: chokidar (non-Darwin) plants an fs.watch handle on EVERY
// file and directory it walks. On a real workspace that is thousands of
// native watcher starts executed synchronously on the host's event loop —
// a CPU profile showed 17.7s inside `FSWatcher.start` (chokidar handler.js
// `_handleFile`), i.e. the 4-7s loop stalls that starved every webview RPC
// at boot ("Restoring…" forever, empty palette). Under the user's corporate
// DLP each handle costs extra. One recursive watcher is what VS Code ships
// on Windows; the historical "recursive drops events under load" concern is
// covered by the tree reading on demand and the index rescanning on gaps.

import * as nodeFs from "node:fs"
import { join as joinPath } from "node:path"
import { watch as chokidarWatch, type FSWatcher } from "chokidar"
import { isProfileSizedRoot } from "./profile-root.js"

export interface FsEvent {
  kind: "add" | "change" | "unlink" | "addDir" | "unlinkDir"
  /** Absolute, normalised. */
  path: string
}

const DEBOUNCE_MS = 80                 // Rider-ish; balances ui freshness vs spam.
const STABILITY_MS = 100               // Only emit ADD after the file has stopped growing.
const POLL_FALLBACK_INTERVAL_MS = 200  // Network drives only — chokidar handles local with native events.
const IGNORE_PATTERNS = [
  /(^|[\\/])\../,         // dotfiles + dot-folders (.git, .vscode, …)
  /[\\/]node_modules[\\/]/,
  /[\\/]dist[\\/]/,
  /[\\/]out[\\/]/,
  /[\\/]\.kaminide-index[\\/]/,
  // Opening a user-profile root drags in the whole Windows profile: AppData
  // alone is hundreds of thousands of entries, and `Start Menu` / `Recent`
  // are ACL-protected, so chokidar fires an endless EPERM storm that
  // saturates the libuv pool — fs-RPC and extension activation then starve
  // for minutes (observed: claude-bridge stuck `idle`, panels never resolved).
  /[\\/](AppData|Application Data|Local Settings)[\\/]/i,
  /[\\/](OneDrive|Dropbox|Google Drive)[\\/]/i,
  /[\\/](Start Menu|Recent|SendTo|Templates|NetHood|PrintHood|Cookies)([\\/]|$)/i,
  /[\\/]\$Recycle\.Bin([\\/]|$)/i,
  /[\\/](Windows|Program Files( \(x86\))?|ProgramData)[\\/]/i,
  /[\\/]NTUSER\.DAT/i,
  /[\\/](target|build|__pycache__)[\\/]/,
]

/** Levels below the root watched when the root is profile-sized. A real
 *  project keeps chokidar's unlimited default; a home / drive root is capped
 *  so we never enumerate an entire user profile. */
const PROFILE_ROOT_DEPTH = 4

// `C:`, `%USERPROFILE%`, `C:\Users\<name>` — общий детект в profile-root.ts.

interface ActiveWatch {
  watcher: FSWatcher | nodeFs.FSWatcher
  root: string
}

type SinkFn = (batch: FsEvent[]) => void

let active: ActiveWatch | null = null

// Рестарт вотчера после ФАТАЛЬНОЙ ошибки (не пер-файловой EPERM/EACCES/ENOENT):
// умерший вотчер раньше оставлял дерево молча протухшим навсегда (ревью).
// Один отложенный рестарт за раз; окно 5с гасит крэш-луп на системной причине
// (ENOSPC-класс), счётчик ограничивает попытки на один и тот же корень.
const WATCHER_RESTART_DELAY_MS = 5000
const WATCHER_RESTART_MAX = 5
let restartTimer: ReturnType<typeof setTimeout> | null = null
let restartCount = 0
let restartRoot: string | null = null

function scheduleWatcherRestart(root: string): void {
  if (restartRoot !== root) { restartRoot = root; restartCount = 0 }
  if (restartTimer) return
  if (restartCount >= WATCHER_RESTART_MAX) {
    // Give-up: НЕ оставлять `active` указывать на мёртвый хэндл — иначе
    // watchWorkspace(root) навсегда no-op (idempotency guard) и дерево молча
    // протухает; после сброса переоткрытие папки поднимет живой вотчер.
    console.error(`[watch] вотчер ${root} не пережил ${WATCHER_RESTART_MAX} перезапусков — слежение снято, дерево обновится при переоткрытии папки`)
    stopWorkspaceWatch()
    return
  }
  restartCount++
  console.warn(`[watch] перезапуск вотчера ${root} через ${WATCHER_RESTART_DELAY_MS}мс (попытка ${restartCount}/${WATCHER_RESTART_MAX})`)
  restartTimer = setTimeout(() => {
    restartTimer = null
    // Актуален только если этот корень всё ещё наблюдаемый (watchWorkspace
    // мог смениться на другой — тогда его вотчер уже живой).
    if (active?.root !== root) return
    stopWorkspaceWatch()
    watchWorkspace(root)
  }, WATCHER_RESTART_DELAY_MS)
}
let pending: FsEvent[] = []
let flushTimer: ReturnType<typeof setTimeout> | null = null
let rendererSink: SinkFn | null = null
let indexSink: SinkFn | null = null
// Fan-out sinks for vscode FileSystemWatchers (B1b). Unlike the two
// fixed sinks above there can be many (one per extension watcher), so
// they live in a Set with per-listener unsubscribe.
const eventSinks = new Set<SinkFn>()

/** Batches forwarded to the renderer (via the shell transport). */
export function setRendererSink(fn: SinkFn | null): void {
  rendererSink = fn
}

/** In-process consumer — the file-index updates its in-memory state in
 *  lock-step with the renderer's tree refresh (same tick, no RPC). */
export function setIndexSink(fn: SinkFn | null): void {
  indexSink = fn
}

/** Subscribe to fs-event batches (backs `vscode.workspace.createFile
 *  SystemWatcher`). Returns an unsubscribe. Many may be active at once. */
export function addEventSink(fn: SinkFn): () => void {
  eventSinks.add(fn)
  return () => { eventSinks.delete(fn) }
}

/** Start watching `root`. Replaces any existing watcher. Idempotent —
 *  calling with the same root no-ops. */
export function watchWorkspace(root: string): void {
  if (active?.root === root) return
  stopWorkspaceWatch()
  // Корень размером с профиль пользователя НЕ watch-им вовсе: даже с
  // `ignoreInitial` и глубиной 4 chokidar обходит дерево, чтобы расставить
  // наблюдатели, и это непрерывная CPU-работа в процессе хоста — за ней
  // вставали ВСЕ его ответы (замер: 16 пауз цикла по 400-670 мс; «0 active»,
  // «панелей нет», переключение сессии с задержкой). Дерево и так читается по
  // запросу, а индекс переобходится при смене папки.
  if (isProfileSizedRoot(root)) {
    console.error(`[watch] пропуск профильного корня ${root} — слежение не ставим`)
    return
  }
  if (process.platform === "win32" && process.env.KAMIN_WATCHER !== "chokidar") {
    active = { watcher: watchRecursiveWin(root), root }
    return
  }
  const watcher = chokidarWatch(root, {
    ignoreInitial: true,
    ignored: IGNORE_PATTERNS,
    awaitWriteFinish: { stabilityThreshold: STABILITY_MS, pollInterval: 50 },
    interval: POLL_FALLBACK_INTERVAL_MS,
    // Junctions inside a profile would be re-enumerated forever.
    followSymlinks: false,
    ...(isProfileSizedRoot(root) ? { depth: PROFILE_ROOT_DEPTH } : {}),
  })
  watcher.on("add",       (p) => { enqueue({ kind: "add",       path: p }) })
  watcher.on("change",    (p) => { enqueue({ kind: "change",    path: p }) })
  watcher.on("unlink",    (p) => { enqueue({ kind: "unlink",    path: p }) })
  watcher.on("addDir",    (p) => { enqueue({ kind: "addDir",    path: p }) })
  watcher.on("unlinkDir", (p) => { enqueue({ kind: "unlinkDir", path: p }) })
  // ACL-protected paths raise EPERM/EACCES per entry; logging each one is
  // itself a load source (every warn crosses the log pipeline).
  watcher.on("error", (err: unknown) => {
    const code = (err as { code?: string } | null)?.code
    if (code === "EPERM" || code === "EACCES" || code === "ENOENT") return
    console.warn("fs/watcher error:", err)
    scheduleWatcherRestart(root)
  })
  active = { watcher, root }
}

export function stopWorkspaceWatch(): void {
  if (!active) return
  void active.watcher.close()
  active = null
  pending = []
  if (flushTimer) { clearTimeout(flushTimer); flushTimer = null }
}

/** Один нативный рекурсивный вотчер на всё дерево (Windows). Тип события
 *  восстанавливаем stat'ом: rename+существует → add/addDir, rename+нет →
 *  unlink (+unlinkDir — из события не отличить, потребители дешёво терпят
 *  оба), change по файлу → change. Игноры — те же паттерны, вручную. */
function watchRecursiveWin(root: string): nodeFs.FSWatcher {
  const w = nodeFs.watch(root, { recursive: true }, (eventType, filename) => {
    if (!filename) return
    const full = joinPath(root, filename)
    if (IGNORE_PATTERNS.some((re) => re.test(full))) return
    let st: nodeFs.Stats | null = null
    try { st = nodeFs.statSync(full) } catch { /* удалён */ }
    if (eventType === "rename") {
      if (st) enqueue({ kind: st.isDirectory() ? "addDir" : "add", path: full })
      else {
        enqueue({ kind: "unlink", path: full })
        enqueue({ kind: "unlinkDir", path: full })
      }
      return
    }
    if (st && !st.isDirectory()) enqueue({ kind: "change", path: full })
  })
  w.on("error", (err: unknown) => {
    const code = (err as { code?: string } | null)?.code
    if (code === "EPERM" || code === "EACCES" || code === "ENOENT") return
    console.warn("fs/watcher error:", err)
    scheduleWatcherRestart(root)
  })
  return w
}

function enqueue(ev: FsEvent): void {
  pending.push(ev)
  if (flushTimer) return
  flushTimer = setTimeout(() => {
    const batch = pending
    pending = []
    flushTimer = null
    if (indexSink) indexSink(batch)
    if (rendererSink) rendererSink(batch)
    for (const sink of eventSinks) sink(batch)
  }, DEBOUNCE_MS)
}

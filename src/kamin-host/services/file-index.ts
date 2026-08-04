// In-memory index of every file in the open workspace folder.
// Moved from `host/index/file-index.ts` in R1b — identical logic, with
// the cache root injected at boot instead of `app.getPath("userData")`
// (kamin-host is electron-free).
//
// Powers Quick Open (Ctrl+P), Find in Files (Ctrl+Shift+F) and the
// File Tree's flat scopes. Persistent cache lives under the LOCAL app data dir
// (rebuildable + tens of MB, so it must not ride the roaming profile):
// `<cacheDir>/.kaminide-index/<hash>/files.tsv`.

import { createHash } from "node:crypto"
import { readFile, writeFile, mkdir, readdir, stat } from "node:fs/promises"
import { join, relative, sep } from "node:path"
import * as v8 from "node:v8"
import { runInNewContext } from "node:vm"
import { isProfileSizedRoot } from "./profile-root.js"

// Обход уступает event loop каждые N файлов (было магическое 2000).
const SCAN_YIELD_EVERY = 2000

export interface IndexedFile {
  /** Workspace-relative POSIX path — stable across OSes for cache. */
  rel: string
  /** Absolute, OS-specific. */
  abs: string
  size: number
  mtimeMs: number
}

const IGNORED_DIRS = new Set([
  ".git", ".svn", ".hg", "node_modules", "dist", "out", "build",
  ".next", ".turbo", ".cache", "coverage", ".kaminide-index",
])
const IGNORED_FILE_EXT = new Set([
  ".lock", ".log",
])
// Корень размером с профиль пользователя: обход без границ занимал ~8 с CPU в
// процессе хоста, и на это время вставали ВСЕ его ответы — «панелей нет»,
// «0 active», данные страниц через десятки секунд (замер: 16 блоков цикла по
// 400-670 мс подряд с 1.1 с до 8 с от старта).
const PROFILE_DIRS = new Set([
  "appdata", "application data", "start menu", "recent", "ntuser.dat",
  "$recycle.bin", "windows", "programdata", "onedrive", "dropbox",
  "google drive", "yandexdisk", "searches", "contacts", "links",
  "saved games", "3d objects", "favorites", "cookies", "printhood",
  "nethood", "sendto", "templates", "local settings",
])
/** Профильный корень индексируем НЕГЛУБОКО: дальше живут кеши и мусор.
 *  Детект общий с вотчером (profile-root.ts) — раньше правила расходились. */
const PROFILE_ROOT_DEPTH = 4

let cacheRoot: string | null = null
let currentRoot: string | null = null
let files: IndexedFile[] = []
const byRel = new Map<string, number>() // rel → array index
// Bumped on every ensureIndex() call; a walk/load that finishes after a newer
// call started must NOT commit its (now-stale) result.
let indexGeneration = 0

/** Called once at host boot — all cache files live under dataDir. */
export function initIndexCacheDir(dataDir: string): void {
  cacheRoot = join(dataDir, ".kaminide-index")
}

/** Returns a hot snapshot the renderer can iterate. Same object the
 *  search functions walk — callers must NOT mutate. */
export function getIndex(): readonly IndexedFile[] {
  return files
}

/** Build / load index for a workspace root. Tries cache first, falls
 *  back to a full walk. Always writes the cache after a fresh walk. */
export async function ensureIndex(root: string): Promise<readonly IndexedFile[]> {
  if (currentRoot === root && files.length > 0) return files
  const gen = ++indexGeneration
  currentRoot = root
  const cache = await loadCache(root)
  // A newer ensureIndex (user switched folder mid-walk) took over — don't clobber
  // its index with this stale root's result, or Ctrl+P/search would serve the
  // wrong project until the folder changes again.
  if (gen !== indexGeneration) return files
  if (cache) {
    files = cache
    rebuildKey()
    // The on-disk cache is a snapshot from a prior launch — files added/deleted
    // while the app was closed are stale (deleted files linger in Ctrl+P; new
    // files never appear + can't trigger workspaceContains activation). Refresh
    // from a real walk in the BACKGROUND so results are instant AND converge to
    // truth without blocking this call.
    void refreshIndexInBackground(root, gen)
    return files
  }
  const walked = await walk(root)
  if (gen !== indexGeneration) return files
  files = walked
  rebuildKey()
  await saveCache(root, walked).catch(() => { /* non-fatal */ })
  fullGcAfterWalk()
  return files
}

/** Re-walk a cached root off the critical path and replace the index if this is
 *  still the current generation (folder not switched away meanwhile). */
async function refreshIndexInBackground(root: string, gen: number): Promise<void> {
  try {
    const walked = await walk(root)
    if (gen !== indexGeneration) return
    files = walked
    rebuildKey()
    await saveCache(root, walked).catch(() => { /* non-fatal */ })
    fullGcAfterWalk()
  } catch { /* non-fatal — keep the cached index */ }
}

/** Полный GC сразу после обхода. Обход профильного корня (202K файлов)
 *  оставляет в old-space сотни MB короткоживущего мусора (dirent'ы, строки,
 *  промисы), и без аллокационного давления V8 не собирает его ЧАСАМИ — RSS
 *  хоста висел на ~800MB при 74MB живых объектов (замер #74: принудительный
 *  GC вернул 806→214MB). GC — оптимизация: любой сбой молча игнорируется. */
function fullGcAfterWalk(): void {
  try {
    const g = (globalThis as { gc?: () => void }).gc
    if (g) { g(); return }
    // Без флага запуска `--expose-gc`: включаем его на лету только на время
    // одного вызова (документированный трюк setFlagsFromString+vm).
    v8.setFlagsFromString("--expose-gc")
    try {
      (runInNewContext("gc") as () => void)()
    } finally {
      v8.setFlagsFromString("--no-expose-gc")
    }
  } catch { /* не обязанность */ }
}

/** Reset index when the user closes the workspace folder. */
export function clearIndex(): void {
  currentRoot = null
  files = []
  byRel.clear()
}

interface FsEvent { kind: "add" | "change" | "unlink" | "addDir" | "unlinkDir"; path: string }

// Serialize batches: `applyFsEventBatchInner` awaits stat/walk, and the sink
// fires it per watcher batch without awaiting. Two overlapping runs would mutate
// the shared `files`/`byRel` mid-flight (one's sync filter racing the other's
// upserts) and could reorder add/delete. Chain each batch onto the previous so
// they apply strictly in arrival order.
let indexQueue: Promise<void> = Promise.resolve()

/** Apply a whole watcher batch, rebuilding the rel→index map AT MOST ONCE.
 *  The single-event path used to `splice`+`rebuildKey()` per unlink — a
 *  `rm -rf node_modules` (thousands of unlink events in one batch) turned that
 *  into O(n²) synchronous work on the host loop and froze the window ("Not
 *  Responding"). Here we collect all removals, do ONE filter pass + ONE
 *  rebuild, then upsert (upsert is O(1) against the fresh map). */
export function applyFsEventBatch(batch: readonly FsEvent[]): Promise<void> {
  indexQueue = indexQueue.then(() => applyFsEventBatchInner(batch)).catch(() => { /* keep the chain alive */ })
  return indexQueue
}

async function applyFsEventBatchInner(batch: readonly FsEvent[]): Promise<void> {
  if (!currentRoot || batch.length === 0) return
  const root = currentRoot
  // Collapse to the LAST intent per exact path — batch order matters: an
  // atomic save emits add(tmp)+unlink(tmp) in one batch, and applying all
  // removals then all upserts (order-blind) would leave the tmp lingering in
  // the index for the whole session. A per-path map (upsert=file, remove=null)
  // keeps the original per-event "last write wins" semantics with one rebuild.
  const intent = new Map<string, IndexedFile | null>()
  const removePrefixes: string[] = []  // unlinkDir subtree prefixes ("dir/")
  for (const ev of batch) {
    const rel = toPosix(relative(root, ev.path))
    if (rel.startsWith("..")) continue // outside workspace — ignore
    if (ev.kind === "unlink") {
      intent.set(rel, null)
    } else if (ev.kind === "unlinkDir") {
      const prefix = `${rel}/`
      intent.set(rel, null); removePrefixes.push(prefix)
      // Null out any child already queued as an upsert earlier in THIS batch
      // (an addDir walk of the same dir): the prefix filter only runs over the
      // pre-existing `files`, so a still-non-null intent child would slip past
      // it and get re-inserted — a ghost entry for a since-deleted folder.
      for (const k of intent.keys()) { if (k.startsWith(prefix)) intent.set(k, null) }
    } else if (ev.kind === "addDir") {
      const sub = await walk(ev.path)
      for (const f of sub) intent.set(f.rel, f)
    } else {
      let s
      try { s = await stat(ev.path) } catch { intent.delete(rel); continue } // vanished — drop any pending upsert
      intent.set(rel, { rel, abs: ev.path, size: s.size, mtimeMs: s.mtimeMs })
    }
  }
  const removeRels = new Set<string>()
  const upserts: IndexedFile[] = []
  for (const [rel, val] of intent) {
    if (val === null) removeRels.add(rel)
    else upserts.push(val)
  }
  if (removeRels.size > 0 || removePrefixes.length > 0) {
    files = files.filter((f) =>
      !removeRels.has(f.rel) && !removePrefixes.some((p) => f.rel.startsWith(p)))
    rebuildKey() // ONCE for the whole batch, not per removed file
  }
  for (const f of upserts) upsert(f)
}

// ─── Persistence ────────────────────────────────────────────────────

// SHA1 hex is 40 chars — keep the first 16 (64-bit collision space)
// for the cache subdir name. Plenty for a per-workspace folder.
const HASH_PREFIX_LEN = 16

function cacheDirFor(root: string): string {
  if (!cacheRoot) throw new Error("file-index: initIndexCacheDir not called")
  const hash = createHash("sha1").update(root).digest("hex").slice(0, HASH_PREFIX_LEN)
  return join(cacheRoot, hash)
}

// Line-based, not JSON. Measured on a real 248 660-file root: JSON was 71.8MB
// and cost 521ms to read+parse; the same data as tab-separated lines is 28.8MB
// and 180ms. Most of the difference is `abs`, which was stored per entry although
// it is just `root + rel` — so it is derived on load instead.
//
// The file name carries the version (`files.tsv` vs the old `files.json`), so an
// old cache is simply not found and the index rebuilds once.
const CACHE_FILE = "files.tsv"
const CACHE_HEADER = "v2"
const SEP = "\t"
const EOL = "\n"

async function loadCache(root: string): Promise<IndexedFile[] | null> {
  const dir = cacheDirFor(root)
  try {
    const raw = await readFile(join(dir, CACHE_FILE), "utf8")
    const nl = raw.indexOf(EOL)
    if (nl < 0) return null
    const [version, cachedRoot] = raw.slice(0, nl).split(SEP)
    if (version !== CACHE_HEADER || cachedRoot !== root) return null
    const out: IndexedFile[] = []
    let from = nl + 1
    while (from < raw.length) {
      let to = raw.indexOf(EOL, from)
      if (to < 0) to = raw.length
      const line = raw.slice(from, to)
      from = to + 1
      const t1 = line.indexOf(SEP)
      if (t1 < 0) continue
      const t2 = line.indexOf(SEP, t1 + 1)
      if (t2 < 0) continue
      const rel = line.slice(0, t1)
      out.push({
        rel,
        abs: join(root, rel),
        size: Number(line.slice(t1 + 1, t2)),
        mtimeMs: Number(line.slice(t2 + 1)),
      })
    }
    return out
  } catch { return null } // missing/corrupt cache → caller rebuilds the index
}

async function saveCache(root: string, list: IndexedFile[]): Promise<void> {
  const dir = cacheDirFor(root)
  await mkdir(dir, { recursive: true })
  const parts = [`${CACHE_HEADER}${SEP}${root}`]
  for (const f of list) parts.push(`${f.rel}${SEP}${String(f.size)}${SEP}${String(f.mtimeMs)}`)
  await writeFile(join(dir, CACHE_FILE), parts.join(EOL), "utf8")
}

// ─── Walker ─────────────────────────────────────────────────────────

async function walk(root: string): Promise<IndexedFile[]> {
  const out: IndexedFile[] = []
  const profile = isProfileSizedRoot(root)
  const t0 = Date.now()
  let scanned = 0
  const stack: { dir: string; depth: number }[] = [{ dir: root, depth: 0 }]
  while (stack.length > 0) {
    const top = stack.pop()
    if (!top) continue
    const { dir, depth } = top
    let entries
    try { entries = await readdir(dir, { withFileTypes: true }) } catch { continue } // unreadable dir (perms/gone) → skip
    for (const e of entries) {
      if (IGNORED_DIRS.has(e.name)) continue
      if (profile && PROFILE_DIRS.has(e.name.toLowerCase())) continue
      const abs = join(dir, e.name)
      if (e.isDirectory()) {
        if (profile && depth + 1 > PROFILE_ROOT_DEPTH) continue
        stack.push({ dir: abs, depth: depth + 1 })
        continue
      }
      if (!e.isFile()) continue
      if (IGNORED_FILE_EXT.has(extOf(e.name))) continue
      let s
      try { s = await stat(abs) } catch { continue } // file vanished mid-walk → skip
      out.push({ rel: toPosix(relative(root, abs)), abs, size: s.size, mtimeMs: s.mtimeMs })
      scanned += 1
    }
    // Уступаем цикл: обход больше не держит ответы хоста
    if (scanned >= SCAN_YIELD_EVERY) { scanned = 0; await new Promise((r) => setImmediate(r)) }
  }
  console.error(`[index] ${out.length} файлов за ${Date.now() - t0}ms`)
  out.sort((a, b) => a.rel.localeCompare(b.rel))
  return out
}

function extOf(name: string): string {
  const i = name.lastIndexOf(".")
  return i >= 0 ? name.slice(i).toLowerCase() : ""
}

function toPosix(p: string): string {
  return p.split(sep).join("/")
}

// ─── Mutation helpers ───────────────────────────────────────────────

function rebuildKey(): void {
  byRel.clear()
  for (let i = 0; i < files.length; i += 1) {
    const f = files[i]
    if (f) byRel.set(f.rel, i)
  }
}

function upsert(f: IndexedFile): void {
  const idx = byRel.get(f.rel)
  if (idx !== undefined) {
    files[idx] = f
  } else {
    byRel.set(f.rel, files.length)
    files.push(f)
  }
}

/** Cache internals, exposed for the format test only. The on-disk shape is what
 *  a wrong change breaks silently — a bad `abs` reconstruction hands out paths
 *  that resolve nowhere — so it is worth pinning. */
export const __testing = { loadCache, saveCache, cacheDirFor, CACHE_FILE }

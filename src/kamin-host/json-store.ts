// Atomic JSON-on-disk store — kamin-host twin of `host/json-file-store.ts`
// with the storage directory injected instead of read from Electron's
// `app.getPath("userData")` (this process must stay electron-free).
// Same guarantees: shallow-merge cache + tmp-write + rename.
//
// Persistence is DEBOUNCED + ASYNC: `set()` updates the in-memory cache
// synchronously (so `get()` is always current) but the disk write is coalesced
// onto a timer and done with async fs. The former per-`set()` `writeFileSync`
// blocked the host event loop on EVERY session mutation (editor-state autosave
// fires rapidly) + every extension globalState/secrets/config write, stalling
// all renderer IPC → UI jank. A synchronous last-chance flush on process exit
// preserves durability on clean shutdown.
import { existsSync, mkdirSync, readFileSync, renameSync, writeFileSync } from "node:fs"
import { rename, writeFile } from "node:fs/promises"
import { dirname, join } from "node:path"

const FLUSH_DEBOUNCE_MS = 200

export class JsonStore<T extends object> {
  private cache: T
  private readonly path: string
  private dirty = false
  private flushTimer: ReturnType<typeof setTimeout> | null = null
  private writing = false

  // Stores with an unflushed write, flushed synchronously on process exit.
  private static readonly pending = new Set<JsonStore<object>>()
  private static exitHookInstalled = false

  constructor(dataDir: string, name: string, defaults: T) {
    this.path = join(dataDir, `${name}.json`)
    this.cache = this.hydrate(defaults)
  }

  get(): T { return this.cache }

  set(patch: Partial<T>): void {
    this.cache = { ...this.cache, ...patch } // in-memory truth is immediate
    this.dirty = true
    this.scheduleFlush()
  }

  private scheduleFlush(): void {
    JsonStore.pending.add(this)
    JsonStore.installExitHook()
    if (this.flushTimer) return
    this.flushTimer = setTimeout(() => { this.flushTimer = null; void this.flush() }, FLUSH_DEBOUNCE_MS)
  }

  /** Coalesced async write: loops while `dirty` so a `set()` that lands during
   *  the write is captured, then drops out of the exit-flush set. tmp+rename
   *  keeps the on-disk file atomic. */
  private async flush(): Promise<void> {
    if (this.writing || !this.dirty) return
    this.writing = true
    try {
      while (this.dirty) {
        this.dirty = false
        const tmp = `${this.path}.tmp`
        try {
          await writeFile(tmp, JSON.stringify(this.cache, null, 2))
          await rename(tmp, this.path)
        } catch (err) {
          // Write failed (disk full, EPERM, AV lock on Windows). Re-mark dirty so
          // the value isn't silently lost: a later set() or the exit-flush picks
          // it up, and we reschedule a debounced retry below. Stay in `pending`.
          this.dirty = true
          console.warn(`JsonStore(${this.path}): flush failed, will retry:`, err)
          break
        }
      }
      // Only leave the exit-flush set once the disk is actually up to date.
      if (!this.dirty) JsonStore.pending.delete(this)
    } finally {
      this.writing = false
    }
    if (this.dirty) this.scheduleFlush() // debounced retry after a failed write
  }

  /** Synchronous last-chance flush — `process.on('exit')` handlers can't await. */
  private flushSync(): void {
    if (!this.dirty) return
    this.dirty = false
    try {
      const tmp = `${this.path}.tmp`
      writeFileSync(tmp, JSON.stringify(this.cache, null, 2))
      renameSync(tmp, this.path)
    } catch { /* best-effort on shutdown */ }
  }

  /** Force every store with an unflushed write to disk, synchronously — what
   *  process exit does. Writes are debounced (FLUSH_DEBOUNCE_MS), so this is the
   *  only way to make "everything is on disk" true at a chosen moment; a test
   *  that re-opens a data dir without it isn't simulating a restart, it's
   *  simulating a crash inside the debounce window. */
  static flushAllSync(): void {
    for (const s of JsonStore.pending) s.flushSync()
  }

  private static installExitHook(): void {
    if (JsonStore.exitHookInstalled) return
    JsonStore.exitHookInstalled = true
    process.on("exit", () => { JsonStore.flushAllSync() })
  }

  private hydrate(defaults: T): T {
    if (!existsSync(this.path)) {
      mkdirSync(dirname(this.path), { recursive: true })
      return { ...defaults }
    }
    try {
      const raw = readFileSync(this.path, "utf8")
      const parsed = JSON.parse(raw) as Partial<T>
      return { ...defaults, ...parsed }
    } catch (err) {
      console.warn(`JsonStore(${this.path}): corrupt JSON, falling back to defaults:`, err)
      return { ...defaults }
    }
  }
}

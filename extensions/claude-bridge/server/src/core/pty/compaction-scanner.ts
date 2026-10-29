// ============================================================================
// Shared compaction discovery. Directory events mark changed JSONL metadata;
// one debounced scan fans out to every watcher in the directory. A periodic
// reconciliation covers dropped events. Transcript content is never opened.
// ============================================================================

import fs from 'node:fs'
import fsp from 'node:fs/promises'
import path from 'node:path'

export interface CompactionSub {
  /** Directory to scan (the session's slug dir). Fixed for the sub's lifetime. */
  readonly dir: string
  /** File the watcher currently tails — the watcher mutates this on switch. */
  currentFilePath: string
  /** PTY session create time — a candidate must be born after this to count. */
  readonly sessionCreatedAt: number
  /** Invoked with the newer file when a compaction / new conversation appears. */
  onNewer: (candidate: string) => void
}

// Matches the old per-session COMPACTION_CHECK_MS — compaction is rare, so 6s
// detection latency is fine and keeps the sweep cost negligible.
const SWEEP_MS = 6000
const RECONCILE_MS = 60_000
const CACHE_LIMIT = 4096

interface DirectoryState {
  stats: Map<string, fs.Stats>
  dirty: Set<string>
  watcher: fs.FSWatcher | null
  fullAt: number
}

export class CompactionScanner {
  private byDir = new Map<string, Set<CompactionSub>>()
  private timer: ReturnType<typeof setInterval> | null = null
  private sweeping = false
  private directories = new Map<string, DirectoryState>()

  /** Register a subscriber; returns an unsubscribe fn. The shared timer starts
   *  on the first subscriber and stops when the last one leaves. */
  subscribe(sub: CompactionSub): () => void {
    let set = this.byDir.get(sub.dir)
    if (!set) {
      set = new Set()
      this.byDir.set(sub.dir, set)
      const state: DirectoryState = { stats: new Map(), dirty: new Set(), watcher: null, fullAt: 0 }
      this.directories.set(sub.dir, state)
      try {
        state.watcher = fs.watch(sub.dir, (_event, name) => {
          // The six-second shared tick debounces bursts of appends/renames.
          if (!name || state.dirty.size >= CACHE_LIMIT) state.fullAt = 0
          else if (String(name).endsWith('.jsonl')) state.dirty.add(String(name))
        })
        state.watcher.on('error', () => {
          state.watcher?.close()
          state.watcher = null
          state.fullAt = 0
        })
        state.watcher.unref?.()
      } catch {
        /* safety sweep remains available on unsupported filesystems */
      }
    }
    set.add(sub)
    if (!this.timer) {
      this.timer = setInterval(() => {
        void this.sweep()
      }, SWEEP_MS)
      this.timer.unref?.() // never keep the process alive just for the sweep
    }
    return () => {
      const s = this.byDir.get(sub.dir)
      if (s) {
        s.delete(sub)
        if (s.size === 0) {
          this.byDir.delete(sub.dir)
          this.directories.get(sub.dir)?.watcher?.close()
          this.directories.delete(sub.dir)
        }
      }
      if (this.byDir.size === 0 && this.timer) {
        clearInterval(this.timer)
        this.timer = null
      }
    }
  }

  /** Run one sweep across all subscribed directories. The shared timer calls
   *  this; also invoked directly by tests. Guarded so sweeps never overlap. */
  async sweep(): Promise<void> {
    if (this.sweeping) return // never overlap sweeps (slow FS shouldn't stack)
    this.sweeping = true
    try {
      // Snapshot the dir list — onNewer callbacks can mutate byDir mid-loop.
      for (const dir of [...this.byDir.keys()]) {
        const subs = this.byDir.get(dir)
        if (!subs || subs.size === 0) continue

        const state = this.directories.get(dir)!
        const full = !state.watcher || Date.now() - state.fullAt >= RECONCILE_MS
        let names: string[]
        const dirty = [...state.dirty]
        state.dirty.clear()
        if (full) {
          try {
            names = (await fsp.readdir(dir, { withFileTypes: true }))
              .filter((d) => d.isFile() && d.name.endsWith('.jsonl'))
              .map((d) => d.name)
          } catch {
            continue
          }
          state.stats.clear()
          state.fullAt = Date.now()
        } else names = dirty
        const stats = state.stats
        // Bound filesystem concurrency and retained metadata. Very large
        // directories use a safety rescan rather than growing the cache forever.
        const overflow: Array<[string, fs.Stats]> = []
        for (const name of names) {
          const p = path.join(dir, name)
          try {
            const st = await fsp.stat(p)
            if (!st.isFile()) {
              stats.delete(p)
              continue
            }
            if (stats.has(p) || stats.size < CACHE_LIMIT) stats.set(p, st)
            else {
              overflow.push([p, st])
              state.fullAt = 0
            }
          } catch {
            stats.delete(p)
          }
          // Overflow candidates are compared immediately, never retained.
          for (const [candidate, st] of overflow.splice(0)) {
            for (const sub of [...subs]) {
              const cur = stats.get(sub.currentFilePath) ?? (await fsp.stat(sub.currentFilePath).catch(() => null))
              if (
                cur &&
                candidate !== sub.currentFilePath &&
                st.mtimeMs > cur.mtimeMs &&
                st.birthtimeMs > sub.sessionCreatedAt
              )
                sub.onNewer(candidate)
            }
          }
        }

        for (const sub of [...subs]) {
          let cur = stats.get(sub.currentFilePath)
          // Current file not in this dir's cache (rare edge) — stat it directly
          // rather than miss a compaction; skip the tick if it too is gone.
          if (!cur) {
            try {
              cur = await fsp.stat(sub.currentFilePath)
            } catch {
              continue
            }
          }
          for (const [p, st] of stats) {
            if (p === sub.currentFilePath) continue
            if (st.mtimeMs > cur.mtimeMs && st.birthtimeMs > sub.sessionCreatedAt) {
              sub.onNewer(p) // first qualifying candidate wins (parity with old scan)
              break
            }
          }
        }
      }
    } finally {
      this.sweeping = false
    }
  }
}

/** Process-wide shared scanner instance. */
export const compactionScanner = new CompactionScanner()

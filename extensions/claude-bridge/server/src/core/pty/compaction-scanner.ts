// ============================================================================
// Shared compaction scanner — ONE readdir + stat sweep per unique directory
// per interval, fanned out to every JsonlWatcher tailing a file in that dir.
//
// Before this, each session ran its own 6s timer that readdir'd its slug dir
// and stat'd every candidate .jsonl to detect a compaction (a newer JSONL file
// appearing). N sessions in the same project therefore re-scanned the SAME
// directory every tick: 36 sessions → 36 redundant readdirs + N×files stats
// per 6s. This collapses that to one readdir + one stat-per-file per directory
// per tick, dispatched to all subscribers on that dir. Read-only, so sharing is
// safe; each subscriber still applies its own current-file / birth-time filter.
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

export class CompactionScanner {
  private byDir = new Map<string, Set<CompactionSub>>()
  private timer: ReturnType<typeof setInterval> | null = null
  private sweeping = false

  /** Register a subscriber; returns an unsubscribe fn. The shared timer starts
   *  on the first subscriber and stops when the last one leaves. */
  subscribe(sub: CompactionSub): () => void {
    let set = this.byDir.get(sub.dir)
    if (!set) { set = new Set(); this.byDir.set(sub.dir, set) }
    set.add(sub)
    if (!this.timer) {
      this.timer = setInterval(() => { void this.sweep() }, SWEEP_MS)
      this.timer.unref?.() // never keep the process alive just for the sweep
    }
    return () => {
      const s = this.byDir.get(sub.dir)
      if (s) { s.delete(sub); if (s.size === 0) this.byDir.delete(sub.dir) }
      if (this.byDir.size === 0 && this.timer) { clearInterval(this.timer); this.timer = null }
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

        let dirents: fs.Dirent[]
        try { dirents = await fsp.readdir(dir, { withFileTypes: true }) } catch { continue }

        // Stat each .jsonl ONCE for the whole directory, cache by full path.
        const stats = new Map<string, fs.Stats>()
        await Promise.all(
          dirents
            .filter((d) => d.isFile() && d.name.endsWith('.jsonl'))
            .map(async (d) => {
              const p = path.join(dir, d.name)
              try { stats.set(p, await fsp.stat(p)) } catch { /* vanished mid-sweep */ }
            }),
        )

        for (const sub of [...subs]) {
          let cur = stats.get(sub.currentFilePath)
          // Current file not in this dir's cache (rare edge) — stat it directly
          // rather than miss a compaction; skip the tick if it too is gone.
          if (!cur) { try { cur = await fsp.stat(sub.currentFilePath) } catch { continue } }
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

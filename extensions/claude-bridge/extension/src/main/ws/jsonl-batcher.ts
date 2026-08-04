// JSONL IPC batcher extracted from connection-manager.ts (Sprint 5 / E1).
//
// Bursts of JSONL entries (one IPC per server chunk) were flooding the
// renderer at ~125/sec during active sessions and causing frame drops.
// We coalesce every arriving entry for up to BATCH_MS, then flush once
// per tab with a single IPC. Subagent entries keyed by agentId/name are
// batched in their own queues so they stay separated.

import type { BrowserWindow } from 'electron'

const BATCH_MS = 50

// Ceiling on how many entries ride in ONE IPC. Coalescing without a size cap
// just trades "many small tasks" for "one enormous task": a session replay
// delivers thousands of entries inside a single BATCH_MS window, and the whole
// pile went out in one `webContents.send` → one postMessage → the renderer had
// to JSON.parse + structured-clone MEGABYTES on its main thread in a single
// uninterruptible task. Measured in the wild: `[FREEZE] renderer main-thread
// silent 3410ms, lastCh=kamin:webview:post, heap=7MB, counts=[]` — one message,
// tiny heap, no flood: pure payload size. Chunk the flush and yield between
// chunks so the renderer can pump its message queue (Windows marks a process
// "not responding" at ~5s without one).
const MAX_ENTRIES_PER_IPC = 150

interface SubagentSlot {
  agentName: string
  agentId?: string
  entries: any[]
}

export class JsonlBatcher {
  // Background (warm-pool) tab: its transcript is cached extension-side but NOT
  // forwarded live — a boot-time warm pool replaying several sessions at once
  // saturated the host→shell pipe with multi-MB frames for tens of seconds and
  // control messages (tab:switched) queued behind them. The webview pulls the
  // cache on demand when the tab is first shown (JsonlViewer's empty-store
  // replay request), so nothing is lost — only the unwatched firehose.
  private muted = false
  private mainBatch: any[] = []
  // Two distinct timers, and `drainTimer !== null` IS "a drain is in flight" —
  // an explicit `draining` flag drifted out of sync with the timer it described.
  private mainTimer: ReturnType<typeof setTimeout> | null = null   // BATCH_MS coalescing window
  private drainTimer: ReturnType<typeof setTimeout> | null = null  // yield between chunks
  private subagentBatch = new Map<string, SubagentSlot>()
  private subagentTimer: ReturnType<typeof setTimeout> | null = null
  // Continuation of an in-flight `drainMainThen`. Held here (not captured in the
  // step closure) so flushMain/clear can RUN or drop it deterministically — an
  // orphaned continuation means the replay's trailing sends never happen.
  private pendingAfter: (() => void) | null = null

  constructor(private window: BrowserWindow, private tabId: string) {}

  /** Toggle background mode (see `muted`). Un-muting does NOT replay what was
   *  dropped — the webview's activation-time replay request serves the cache. */
  setMuted(muted: boolean): void { this.muted = muted }

  queueMain(entries: any[]): void {
    if (this.muted) return // background tab: cache-only, no live forwarding
    if (entries.length === 0) return
    // Appending while a drain is in flight is safe AND keeps order: the drain
    // splices off the FRONT, so late arrivals simply ride a later chunk. No new
    // timer while draining — the in-flight drain already picks them up.
    this.mainBatch.push(...entries)
    if (this.mainTimer === null && this.drainTimer === null) {
      this.mainTimer = setTimeout(() => this.drainMain(), BATCH_MS)
    }
  }

  /** One bounded chunk onto the wire. False = window gone, nothing more to do. */
  private sendChunk(): boolean {
    if (this.window.isDestroyed()) { this.mainBatch = []; return false }
    const chunk = this.mainBatch.splice(0, MAX_ENTRIES_PER_IPC)
    if (chunk.length > 0) this.window.webContents.send('jsonl-entries', this.tabId, chunk)
    return true
  }

  /** Normal path: one chunk per turn, yielding so the renderer keeps pumping. */
  private drainMain(): void {
    this.mainTimer = null
    this.drainTimer = null
    if (!this.sendChunk()) return
    if (this.mainBatch.length > 0) this.drainTimer = setTimeout(() => this.drainMain(), 0)
  }

  /** Put everything queued on the wire NOW, in order, before the caller sends
   *  something else. `replayJsonlToRenderer` DEPENDS on that: it dumps its own
   *  snapshot on the next line, so anything still in flight must already be out
   *  or the two interleave (overlapping entries → dedup + scroll-pin confusion).
   *  This must therefore NOT no-op when a drain is already in flight — that was
   *  the bug: the leftover chunks kept trickling via the yield timer and landed
   *  AFTER the snapshot, which is exactly what the caller flushes to prevent.
   *  Chunking still holds (many bounded IPCs, never one huge one) — only the
   *  yield between them is given up, and this is a reload path, not a hot one. */
  flushMain(): void {
    if (this.mainTimer !== null) { clearTimeout(this.mainTimer); this.mainTimer = null }
    if (this.drainTimer !== null) { clearTimeout(this.drainTimer); this.drainTimer = null }
    while (this.mainBatch.length > 0) { if (!this.sendChunk()) break }
    // RUN a pending continuation rather than dropping it: this is how a second
    // replay starting mid-drain still emits the first one's trailing sends
    // (jsonl-status / tree / streaming-entry) BEFORE its own chunks. Dropping it
    // would strand the renderer in replay mode (replayComplete never sent).
    this.runPendingAfter()
  }

  private runPendingAfter(): void {
    const after = this.pendingAfter
    this.pendingAfter = null
    if (after) after()
  }

  /** Send `entries` in bounded, YIELDING chunks, then run `after`.
   *
   *  WHY this exists: `replayJsonlToRenderer` used to `webContents.send` the
   *  ENTIRE cached transcript in ONE ipc, bypassing MAX_ENTRIES_PER_IPC — the
   *  only jsonl-entries send that did. Measured on a real session: a 46MB frame,
   *  191ms just to JSON.parse in the renderer; at the 20k cache cap ≈169MB and
   *  ≈985ms of serialization in ONE uninterruptible task. Prod freeze logs show
   *  its signature exactly: `lastCh=kamin:webview:post`, `counts=[]` (the
   *  renderer processed NOTHING — one long task, not a flood) and heap spiking
   *  to ~105MB against a 27MB baseline.
   *
   *  The hard part is ORDER, not chunking. Once the snapshot itself yields, the
   *  caller's trailing sends would overtake the still-draining chunks — so they
   *  must ride `after`, not the next line of the caller. */
  drainMainThen(entries: any[], after: () => void): void {
    // A previous replay's continuation must not be lost — flush settles it.
    this.flushMain()
    this.mainBatch.push(...entries)
    this.pendingAfter = after
    const step = (): void => {
      this.drainTimer = null
      if (!this.sendChunk()) { this.runPendingAfter(); return } // window gone: still settle
      if (this.mainBatch.length > 0) this.drainTimer = setTimeout(step, 0)
      else this.runPendingAfter()
    }
    step()
  }

  queueSubagent(cacheKey: string, agentName: string, agentId: string | undefined, entries: any[]): void {
    if (this.muted) return // background tab: cache-only, no live forwarding
    if (entries.length === 0) return
    const slot = this.subagentBatch.get(cacheKey) ?? { agentName, agentId, entries: [] }
    slot.entries.push(...entries)
    this.subagentBatch.set(cacheKey, slot)
    if (this.subagentTimer === null) {
      this.subagentTimer = setTimeout(() => this.flushSubagent(), BATCH_MS)
    }
  }

  flushSubagent(): void {
    if (this.subagentTimer !== null) {
      clearTimeout(this.subagentTimer)
      this.subagentTimer = null
    }
    if (this.subagentBatch.size === 0) return
    const batched = [...this.subagentBatch.entries()]
    this.subagentBatch.clear()
    if (this.window.isDestroyed()) return
    for (const [, slot] of batched) {
      this.window.webContents.send('jsonl-subagent-entries', this.tabId, slot.agentName, slot.entries, slot.agentId)
    }
  }

  /** Drop in-flight batches — used on compaction so pre-compact entries
   *  don't leak into the new file's stream. */
  clear(): void {
    this.mainBatch = []
    if (this.mainTimer) { clearTimeout(this.mainTimer); this.mainTimer = null }
    // Also kill an in-flight drain: it used to survive `clear()` and only
    // settled by accident, because each step re-read the (now empty) batch.
    if (this.drainTimer) { clearTimeout(this.drainTimer); this.drainTimer = null }
    // A cleared batch has nothing left to announce — drop the continuation
    // instead of firing it over entries that no longer exist.
    this.pendingAfter = null
    this.subagentBatch.clear()
    if (this.subagentTimer) { clearTimeout(this.subagentTimer); this.subagentTimer = null }
  }
}

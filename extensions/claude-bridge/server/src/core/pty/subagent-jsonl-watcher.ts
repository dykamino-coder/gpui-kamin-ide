// Lightweight tail-watcher for a single subagent JSONL file. Extracted from
// `jsonl-watcher.ts` (Sprint 2 / Stage C, C7) — the parent file shed a
// self-contained class without losing any functionality. Delegates parsing
// to `parseJsonlLine` (no `file-history-snapshot` lines apply here, but the
// helper keeps that defensive byte-skip logic in one place anyway).

import fs from 'fs'
import fsp from 'fs/promises'
import type { JsonlEntry } from '../../shared/jsonl-types'
import { SKIP_LINE_MARKER } from './jsonl-watcher'

const DISCOVERY_POLL_MS = 1000
const SUBAGENT_POLL_MS = 1000
// Chunk the initial subagent replay instead of one giant WS frame — a large
// team/subagent transcript sent whole floods the client + spikes the WebView2
// renderer heap (prod OOM class). Same size/gap as the main watcher's replay.
const REPLAY_CHUNK = 200
const REPLAY_CHUNK_DELAY_MS = 10
// Safety-net poll cadence when fs.watch is live — pure backup, so slow (10s).
// Mirrors the main watcher's constant: how soon to re-try a tail read after the
// WS refused a frame (16MB bufferedAmount cap).
const TAIL_BACKPRESSURE_RETRY_MS = 400
const WATCH_SAFETY_POLL_MS = 10_000
const SUBAGENT_DISCOVERY_TIMEOUT_MS = 120_000

async function pathExists(p: string): Promise<boolean> {
  try { await fsp.access(p); return true } catch { return false }
}

export class SubagentJsonlWatcher {
  private agentToolUseId: string
  private filePath: string
  // Returns false when the WS dropped the frame (bufferedAmount past the 16MB
  // cap). Typed `void` before, which SILENTLY DISCARDED that signal — see the
  // offset handling in tailOnce/startTailing.
  private sendEntries: (entries: JsonlEntry[]) => boolean | void
  private stopped = false
  private lastOffset = 0
  private reading = false
  // Last time this file grew (or was first read). The parent watcher retires
  // watchers idle past a threshold so completed subagents don't accumulate live
  // timers + inotify handles for the whole (possibly hours-long) session.
  private lastActivityAt = Date.now()
  private discoveryTimer: ReturnType<typeof setInterval> | null = null
  private discoveryTimeout: ReturnType<typeof setTimeout> | null = null
  private tailTimer: ReturnType<typeof setInterval> | null = null
  private tailWatcher: fs.FSWatcher | null = null
  private retryTimer: ReturnType<typeof setTimeout> | null = null

  constructor(
    agentToolUseId: string,
    filePath: string,
    sendEntries: (entries: JsonlEntry[]) => boolean | void,
  ) {
    this.agentToolUseId = agentToolUseId
    this.filePath = filePath
    this.sendEntries = sendEntries
  }

  async start(): Promise<void> {
    if (await pathExists(this.filePath)) {
      void this.startTailing()
      return
    }
    this.discoveryTimer = setInterval(() => {
      if (this.stopped) return
      void (async () => {
        if (await pathExists(this.filePath)) {
          if (this.discoveryTimer) { clearInterval(this.discoveryTimer); this.discoveryTimer = null }
          if (this.discoveryTimeout) { clearTimeout(this.discoveryTimeout); this.discoveryTimeout = null }
          void this.startTailing()
        }
      })()
    }, DISCOVERY_POLL_MS)

    this.discoveryTimeout = setTimeout(() => {
      if (this.stopped || this.tailTimer) return
      // Give up waiting for file
      if (this.discoveryTimer) {
        clearInterval(this.discoveryTimer)
        this.discoveryTimer = null
      }
    }, SUBAGENT_DISCOVERY_TIMEOUT_MS)
  }

  /** Epoch ms of the last file growth — used by the parent to retire idle
   *  (completed) subagent watchers instead of holding them for the session. */
  getLastActivityAt(): number {
    return this.lastActivityAt
  }

  stop(): void {
    this.stopped = true
    if (this.discoveryTimer) { clearInterval(this.discoveryTimer); this.discoveryTimer = null }
    if (this.discoveryTimeout) { clearTimeout(this.discoveryTimeout); this.discoveryTimeout = null }
    if (this.tailTimer) { clearInterval(this.tailTimer); this.tailTimer = null }
    if (this.retryTimer) { clearTimeout(this.retryTimer); this.retryTimer = null }
    if (this.tailWatcher) { try { this.tailWatcher.close() } catch {}; this.tailWatcher = null }
  }

  private async startTailing(): Promise<void> {
    if (this.stopped) return

    try {
      const stat = await fsp.stat(this.filePath)
      if (stat.size > 0) {
        const entries = await this.readEntries(0)
        for (let i = 0; i < entries.length; i += REPLAY_CHUNK) {
          if (this.stopped) return
          const chunk = entries.slice(i, i + REPLAY_CHUNK)
          // Same deliver-then-advance rule as the tail below: a dropped chunk
          // must NOT be counted as consumed. Leave lastOffset where it is and
          // bail — the tail poll re-reads from there and re-sends the rest.
          if (chunk.length > 0 && this.sendEntries(chunk) === false) return
          if (i + REPLAY_CHUNK < entries.length) {
            await new Promise((r) => setTimeout(r, REPLAY_CHUNK_DELAY_MS))
          }
        }
        this.lastOffset = stat.size
      }
    } catch { /* file may be empty or briefly locked */ }

    // Prefer native fs.watch + lazy poll fallback; see main-session watcher above.
    try {
      let debounceTimer: ReturnType<typeof setTimeout> | null = null
      const schedule = () => {
        if (debounceTimer) clearTimeout(debounceTimer)
        debounceTimer = setTimeout(() => {
          debounceTimer = null
          if (this.stopped || this.reading) return
          void this.checkForNew()
        }, 20)
      }
      this.tailWatcher = fs.watch(this.filePath, { persistent: false }, (eventType) => {
        if (this.stopped) return
        if (eventType === 'rename') {
          if (this.tailWatcher) { try { this.tailWatcher.close() } catch {}; this.tailWatcher = null }
        }
        schedule()
      })
      // Async FSWatcher 'error' (inotify limits / FS quirks) would otherwise
      // throw uncaught → crash; close + fall back to the interval poll below.
      this.tailWatcher.on('error', () => {
        if (this.tailWatcher) { try { this.tailWatcher.close() } catch {}; this.tailWatcher = null }
      })
    } catch {
      this.tailWatcher = null
    }

    this.tailTimer = setInterval(() => {
      if (this.stopped || this.reading) return
      void this.checkForNew()
    }, this.tailWatcher ? WATCH_SAFETY_POLL_MS : SUBAGENT_POLL_MS)
  }

  private async checkForNew(): Promise<void> {
    if (this.stopped) return
    try {
      const stat = await fsp.stat(this.filePath)
      if (stat.size <= this.lastOffset) return
      this.lastActivityAt = Date.now()
      this.reading = true
      const entries = await this.readEntries(this.lastOffset)
      if (entries.length === 0) { this.lastOffset = stat.size; return }
      // Deliver BEFORE advancing the offset — the same rule the MAIN watcher was
      // given in 6.3.25 (jsonl-watcher.ts). `sendEntries` returns false when the
      // WS dropped the frame (bufferedAmount past the 16MB cap, or a frozen
      // client); advancing then strands these lines FOREVER, because the tail
      // never re-reads a consumed range. This file advanced first and typed the
      // callback `void`, so a backpressured client silently lost subagent
      // transcript lines — the identical bug, in the sibling watcher.
      if (this.sendEntries(entries) === false) {
        if (this.retryTimer) clearTimeout(this.retryTimer)
        this.retryTimer = setTimeout(() => {
          this.retryTimer = null
          if (!this.stopped && !this.reading) void this.checkForNew()
        }, TAIL_BACKPRESSURE_RETRY_MS)
        return
      }
      this.lastOffset = stat.size
    } catch { /* ignore */ }
    finally { this.reading = false }
  }

  private async readEntries(offset: number): Promise<JsonlEntry[]> {
    // Buffered tail-read; see jsonl-watcher.ts for why we don't use
    // fs.createReadStream + readline (libuv threadpool starvation hangs
    // the async iterator silently).
    const stat = await fsp.stat(this.filePath)
    if (stat.size <= offset) return []
    const fd = await fsp.open(this.filePath, 'r')
    let chunkText: string
    try {
      const buf = Buffer.alloc(stat.size - offset)
      await fd.read(buf, 0, buf.length, offset)
      chunkText = buf.toString('utf8')
    } finally {
      await fd.close()
    }
    const entries: JsonlEntry[] = []
    for (const line of chunkText.split('\n')) {
      if (!line.trim()) continue
      if (line.includes(SKIP_LINE_MARKER)) continue
      try {
        const parsed = JSON.parse(line) as JsonlEntry
        entries.push(parsed)
      } catch { /* skip */ }
    }
    return entries
  }
}

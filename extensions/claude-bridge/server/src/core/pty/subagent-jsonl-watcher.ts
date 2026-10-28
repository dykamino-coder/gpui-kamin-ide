// Lightweight tail-watcher for a single subagent JSONL file.
//
// One delivery path for the initial replay and the live tail (INC-2026-0078,
// F09): the watch, the safety poll and the retry timer are armed BEFORE the
// first delivery, so a refused first frame can never leave the watcher with no
// way to continue. The cursor advances per DELIVERED batch (partial progress
// is kept), only past complete records (a torn last line is re-read), and on
// byte-framed lines (multi-byte characters never split). Temporary pressure is
// retried on a timer; a record that can never fit a frame is replaced by an
// explicit stub instead of being retried forever.

import fs from 'fs'
import fsp from 'fs/promises'
import type { JsonlEntry } from '../../shared/jsonl-types'
import { SKIP_LINE_MARKER } from './jsonl-watcher'
import { readJsonlLines } from './jsonl-lines'
import {
  normalizeOutcome,
  oversizedStub,
  REPLAY_BATCH_BYTES,
  REPLAY_BATCH_RECORDS,
  type SendOutcome,
  type Tagged,
} from './jsonl-replay'

const DISCOVERY_POLL_MS = 1000
const SUBAGENT_POLL_MS = 1000
// Pause between delivered batches of a large backlog, so a subagent replay
// never floods the client in one burst.
const BATCH_GAP_MS = 10
// Re-try after the WS refused a frame (temporary pressure / not attached yet).
const TAIL_BACKPRESSURE_RETRY_MS = 400
const WATCH_SAFETY_POLL_MS = 10_000
const SUBAGENT_DISCOVERY_TIMEOUT_MS = 120_000

async function pathExists(p: string): Promise<boolean> {
  try {
    await fsp.access(p)
    return true
  } catch {
    return false
  }
}

export class SubagentJsonlWatcher {
  private agentToolUseId: string
  private filePath: string
  private sendEntries: (entries: JsonlEntry[]) => boolean | void | SendOutcome
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
    sendEntries: (entries: JsonlEntry[]) => boolean | void | SendOutcome,
    private options: { startOffset?: number } = {},
  ) {
    this.agentToolUseId = agentToolUseId
    this.filePath = filePath
    this.sendEntries = sendEntries
    this.lastOffset = options.startOffset ?? 0
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
          if (this.discoveryTimer) {
            clearInterval(this.discoveryTimer)
            this.discoveryTimer = null
          }
          if (this.discoveryTimeout) {
            clearTimeout(this.discoveryTimeout)
            this.discoveryTimeout = null
          }
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

  getLastOffset(): number {
    return this.lastOffset
  }

  stop(): void {
    this.stopped = true
    if (this.discoveryTimer) {
      clearInterval(this.discoveryTimer)
      this.discoveryTimer = null
    }
    if (this.discoveryTimeout) {
      clearTimeout(this.discoveryTimeout)
      this.discoveryTimeout = null
    }
    if (this.tailTimer) {
      clearInterval(this.tailTimer)
      this.tailTimer = null
    }
    if (this.retryTimer) {
      clearTimeout(this.retryTimer)
      this.retryTimer = null
    }
    if (this.tailWatcher) {
      try {
        this.tailWatcher.close()
      } catch {}
      this.tailWatcher = null
    }
  }

  private async startTailing(): Promise<void> {
    if (this.stopped) return
    // Arm every way of continuing FIRST; only then deliver anything.
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
          if (this.tailWatcher) {
            try {
              this.tailWatcher.close()
            } catch {}
            this.tailWatcher = null
          }
        }
        schedule()
      })
      // Async FSWatcher 'error' (inotify limits / FS quirks) would otherwise
      // throw uncaught → crash; close + fall back to the interval poll below.
      this.tailWatcher.on('error', () => {
        if (this.tailWatcher) {
          try {
            this.tailWatcher.close()
          } catch {}
          this.tailWatcher = null
        }
      })
    } catch {
      this.tailWatcher = null
    }
    this.tailTimer = setInterval(
      () => {
        if (this.stopped || this.reading) return
        void this.checkForNew()
      },
      this.tailWatcher ? WATCH_SAFETY_POLL_MS : SUBAGENT_POLL_MS,
    )
    // Initial replay (from 0, or the resumed offset) is the same tail path.
    await this.checkForNew()
  }

  private scheduleRetry(): void {
    if (this.stopped) return
    if (this.retryTimer) clearTimeout(this.retryTimer)
    this.retryTimer = setTimeout(() => {
      this.retryTimer = null
      if (!this.stopped && !this.reading) void this.checkForNew()
    }, TAIL_BACKPRESSURE_RETRY_MS)
  }

  /** Complete records from `offset` to the current end, tagged with byte
   *  positions (diagnostics/tests; delivery uses the batched path below). */
  async readEntries(offset: number): Promise<Tagged[]> {
    const out: Tagged[] = []
    for await (const line of readJsonlLines(this.filePath, { start: offset })) {
      if (line.text.includes(SKIP_LINE_MARKER)) continue
      try {
        const e = JSON.parse(line.text) as Tagged
        e._pos = line.start
        e._posEnd = line.end
        out.push(e)
      } catch {
        /* torn or malformed */
      }
    }
    return out
  }

  /** Try one batch; true when it was delivered (or explicitly stubbed). */
  private deliver(batch: Tagged[]): boolean {
    const r = normalizeOutcome(this.sendEntries(batch))
    if (r === 'sent') return true
    if (r !== 'oversized') return false
    // Never fits: split, and stub a single record that cannot fit alone.
    if (batch.length > 1) {
      const mid = batch.length >> 1
      return this.deliver(batch.slice(0, mid)) && this.deliver(batch.slice(mid))
    }
    const e = batch[0]!
    const stub = oversizedStub(e, (e._posEnd ?? 0) - (e._pos ?? 0))
    const s = normalizeOutcome(this.sendEntries([stub]))
    return s === 'sent' || s === 'oversized'
  }

  private async checkForNew(): Promise<void> {
    if (this.stopped || this.reading) return
    this.reading = true
    try {
      const size = (await fsp.stat(this.filePath)).size
      if (size < this.lastOffset) this.lastOffset = 0 // rewritten/truncated: start over
      if (size <= this.lastOffset) return
      this.lastActivityAt = Date.now()
      let batch: Tagged[] = []
      let batchBytes = 0
      let batchEnd = this.lastOffset
      let sentBatches = 0
      const flush = async (): Promise<boolean> => {
        if (batch.length > 0) {
          // Space consecutive batches of a backlog; the first goes at once.
          if (sentBatches > 0) await new Promise((r) => setTimeout(r, BATCH_GAP_MS))
          if (this.stopped || !this.deliver(batch)) return false
          sentBatches++
        }
        // Delivered: the cursor moves past exactly this batch.
        this.lastOffset = batchEnd
        batch = []
        batchBytes = 0
        return !this.stopped
      }
      for await (const line of readJsonlLines(this.filePath, { start: this.lastOffset, end: size })) {
        if (this.stopped) return
        if (!line.terminated) {
          let complete = true
          try {
            JSON.parse(line.text)
          } catch {
            complete = false
          }
          if (!complete) break // torn write: re-read once complete
        }
        batchEnd = line.end
        if (line.text.includes(SKIP_LINE_MARKER)) continue
        let parsed: Tagged
        try {
          parsed = JSON.parse(line.text) as Tagged
        } catch {
          continue
        }
        parsed._pos = line.start
        parsed._posEnd = line.end
        batch.push(parsed)
        batchBytes += line.end - line.start
        if (batch.length >= REPLAY_BATCH_RECORDS || batchBytes >= REPLAY_BATCH_BYTES) {
          if (!(await flush())) {
            this.scheduleRetry()
            return
          }
        }
      }
      if (!(await flush())) this.scheduleRetry()
    } catch {
      /* briefly unavailable — the poll/watch try again */
    } finally {
      this.reading = false
    }
  }
}

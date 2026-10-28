// ============================================================================
// JSONL File Watcher — discovers and tails Claude Code session JSONL files
//
// Key design: each PTY session gets its own settingsDir as CWD.
// Claude CLI computes a project slug from CWD and stores JSONL files in
// ~/.claude/projects/<slug>/<sessionId>.jsonl
//
// This watcher only looks in the correct slug directory for the session,
// NOT in the entire ~/.claude/projects/ tree (which would pick up unrelated
// conversations like the user's own Claude Code sessions).
//
// Compaction support: when Claude CLI compacts (creates a new conversation),
// a new JSONL file appears in the same slug directory. The watcher detects
// this and switches to the new file, notifying the client.
// ============================================================================

import fs from 'fs'
import fsp from 'fs/promises'
import path from 'path'
import os from 'os'
import type { JsonlEntry, JsonlStatus } from '../../shared/jsonl-types'
import { SubagentJsonlWatcher } from './subagent-jsonl-watcher'
import { compactionScanner, type CompactionSub } from './compaction-scanner'
import { fingerprintTranscript } from './jsonl-fingerprint'
import { SegmentIndexBuilder, type SegmentIndex } from './segment-index'
import { readJsonlLines, readRange } from './jsonl-lines'
import {
  RecordIndex,
  REPLAY_BATCH_BYTES,
  REPLAY_BATCH_RECORDS,
  batchBackward,
  batchForward,
  batchSpan,
  loadBatch,
  normalizeOutcome,
  oversizedStub,
  type SendOutcome,
  type Tagged,
} from './jsonl-replay'
import { debugLog } from '../logging'

// Cheap pre-filter before JSON.parse — skips ~60-80% of lines on sessions
// with heavy file-history-snapshot logging (Write/Edit tool heavy sessions).
export const SKIP_LINE_MARKER = '"type":"file-history-snapshot"'

/** Parse a single JSONL line into a `JsonlEntry`. Skips empty lines and
 *  the `file-history-snapshot` hot-path bytes without ever invoking
 *  JSON.parse. Returns `null` on malformed JSON (caller drops it).
 *
 *  Extracted as a pure helper so the line-by-line decision tree can be
 *  unit-tested without spinning up a watcher or the FS. */
export function parseJsonlLine(line: string): JsonlEntry | null {
  if (!line || !line.trim()) return null
  if (line.includes(SKIP_LINE_MARKER)) return null
  try {
    return JSON.parse(line) as JsonlEntry
  } catch {
    return null
  }
}

/** Conversation ID is the basename (sans `.jsonl`) of the session's JSONL
 *  file. CLI assigns it as the session's UUID at create-time. */
export function conversationIdFromPath(jsonlPath: string): string {
  return path.basename(jsonlPath, '.jsonl')
}

async function pathExists(p: string): Promise<boolean> {
  try {
    await fsp.access(p)
    return true
  } catch {
    return false
  }
}

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const PROJECTS_DIR = path.join(os.homedir(), '.claude', 'projects')
const DISCOVERY_POLL_MS = 1000
const DISCOVERY_TIMEOUT_MS = 300_000 // 5 min — then slow-poll instead of error
const TAIL_POLL_MS = 200
// Re-check delay after a backpressure DROP — fast enough that a briefly-slow
// client recovers its missed lines within a blink, not the 10s safety poll.
const TAIL_BACKPRESSURE_RETRY_MS = 400
// Safety-net poll cadence WHEN fs.watch is live — it's pure backup for the rare
// FS that drops inotify events, so it can be slow. (Without fs.watch we fall to
// the aggressive TAIL_POLL_MS for streaming responsiveness.)
const WATCH_SAFETY_POLL_MS = 10_000
// Live tail: complete records read per pass (a longer single record is read
// whole). Further growth is picked up by an immediate follow-up pass.
const TAIL_BATCH_BYTES = 4 * 1024 * 1024
// First re-check delay while the client socket is over the replay high-water
// mark; doubles up to REPLAY_BACKPRESSURE_RETRY_MS.
const DRAIN_POLL_MS = 25
const INITIAL_BATCH_DELAY_MS = 10
// Pause-and-retry the SAME replay batch when the client socket is backpressured
// (sendEntries → false, bufferedAmount past the 16MB cap). Without this the
// initial/reattach replay fired the whole 26MB session at the socket in ~0.5s;
// frames past the cap were silently dropped AND the flood overran the WebView2
// renderer heap → prod OOM crash. Slightly longer than the tail retry: replay is
// a bulk burst, so give the socket real time to drain before resending.
const REPLAY_BACKPRESSURE_RETRY_MS = 250
// How many times the tail preview re-offers itself while the client's socket is
// not yet attached (a fresh resume attaches within a second or two).
const PREVIEW_SEND_ATTEMPTS = 12
const SUBAGENT_POLL_MS = 1000
const SUBAGENT_DISCOVERY_TIMEOUT_MS = 120_000 // 2 min to wait for file to appear
// Retire a subagent watcher after this long with no file growth — a completed
// subagent shouldn't hold a live timer + inotify handle for the whole session.
const SUBAGENT_RETIRE_MS = 5 * 60_000
// Bound retired-file stats to once per minute per path, rather than every 4s
// scan across hundreds of open sessions. A resumed subagent may wait up to 60s.
const RETIRED_SUBAGENT_CHECK_MS = 60_000

interface RetiredSubagent {
  offset: number
  size: number
  dev: number
  ino: number
  mtimeMs: number
  birthtimeMs: number
  nextCheckAt: number
}

// ---------------------------------------------------------------------------
// Slug computation — must match Claude CLI's xfA()
// ---------------------------------------------------------------------------

/**
 * Compute project slug from a directory path.
 * Must match CLI's computation: replace ALL non-alphanumeric chars with '-',
 * max 200 chars + hash if longer.
 */
export function computeProjectSlug(dir: string): string {
  const resolved = path.resolve(dir)
  const slug = resolved.replace(/[^a-zA-Z0-9]/g, '-')
  if (slug.length <= 200) return slug
  // Hash for long paths (same algorithm as CLI)
  let hash = 0
  for (let i = 0; i < resolved.length; i++) {
    hash = ((hash << 5) - hash + resolved.charCodeAt(i)) | 0
  }
  return `${slug.slice(0, 200)}-${Math.abs(hash).toString(36)}`
}

// ---------------------------------------------------------------------------
// JsonlWatcher
// ---------------------------------------------------------------------------

// Monotonic base for reverse-replay `_ord` tags. Process-global (not
// per-watcher) so ords stay unique across compaction file-switches AND
// across watcher instances (model/effort restart, exit→reconnect on the
// same tab). The client dedups replayed entries on `_ord`; a per-instance
// counter restarting at 0 collided a new watcher's ords with the previous
// one's and silently dropped never-seen entries.
let globalOrdCounter = 0

export class JsonlWatcher {
  private sessionCreatedAt: number
  /** Returns false when the frame was DROPPED (WS backpressure / closed socket).
   *  The tail reader uses this to NOT advance its offset past undelivered
   *  entries, so a slow/frozen client can't permanently lose transcript lines. */
  // `replay` = записи истории/превью, приходящие ВНЕ файла-порядка. Клиент по
  // этому флагу НЕ двигает курсор зеркала: tail-preview шлёт хвост файла первым,
  // и курсор прыгал на конец — недосланный резюмом промежуток фильтровался как
  // «дубль» навсегда (прод-дыра 7092-7116, «сообщение не отрисовалось»).
  private sendEntries: (entries: JsonlEntry[], replay?: boolean) => boolean | SendOutcome
  /** True when the client socket can take another frame (open and under the
   *  replay high-water mark). Absent = always writable (tests). */
  private writable?: () => boolean
  /** Aborts the reads of the running replay generation. */
  private replayAbort: AbortController | null = null
  private sendStatus: (status: JsonlStatus) => void
  private onUserEntry?: (entry: JsonlEntry) => void
  private onConversationId?: (conversationId: string) => void
  // `boolean | void`, NOT void: the subagent watcher must see a dropped frame to
  // avoid advancing its offset past lines the client never got (the 6.3.25 rule).
  // Typing this `void` silently discarded that signal all the way down.
  private sendSubagentEntries?: (agentName: string, entries: JsonlEntry[], agentId?: string) => boolean | void
  // Live dashboard-stat hooks: onReplayReset fires once at the start of every
  // full replay; onStatsEntry fires for every parsed entry (replay + tail).
  private onReplayReset?: () => void
  private onStatsEntry?: (entry: JsonlEntry) => void

  /** The slug directory where this session's JSONL files live */
  private slugDir: string | null

  private discoveryTimer: ReturnType<typeof setInterval> | null = null
  private discoveryTimeout: ReturnType<typeof setTimeout> | null = null
  private tailTimer: ReturnType<typeof setInterval> | null = null
  /** Optional native fs.watch handle — used when available to avoid polling. */
  private tailWatcher: fs.FSWatcher | null = null
  // Compaction detection is handled by the process-wide shared scanner (one
  // readdir+stat sweep per directory, not one per session). We hold our sub
  // (to mutate currentFilePath on switch) + the unsubscribe fn.
  private compactionSub: CompactionSub | null = null
  private compactionUnsub: (() => void) | null = null
  private stopped = false
  /** Wall-clock of `start()`, the zero for `timing()`. */
  private startedAt = 0
  private filePath: string | null = null
  private lastOffset = 0
  private fileIdentity: string | null = null
  private reading = false
  // Fast re-check after a backpressure DROP (offset not advanced) so recovery
  // doesn't wait on the 10s safety poll.
  private tailRetryTimer: ReturnType<typeof setTimeout> | null = null
  // Invalidation token for in-flight initial replays: switchToFile/stop/replayAll
  // bump it, and readInitialContent aborts after any await if its captured value
  // went stale. Without this an old file's replay kept emitting after a
  // compaction switch and clobbered lastOffset with the OLD file's size,
  // silencing the tail on the new file.
  private replayGen = 0

  // Coalescing guards for the live-compaction segment-index rebuild (see
  // rebuildSegmentIndex): only one full re-parse runs at a time; a boundary that
  // lands mid-rebuild sets `Again` to trigger exactly one more pass afterwards.
  private rebuildingIndex = false
  private rebuildIndexAgain = false

  /** Active subagent JSONL watchers (keyed by file path) */
  private subagentWatchers = new Map<string, SubagentJsonlWatcher>()
  private retiredSubagents = new Map<string, RetiredSubagent>()
  private scanningSubagents = false
  private subagentScanTimer: ReturnType<typeof setInterval> | null = null
  private loggedScanSkip = false

  /**
   * @param sessionCreatedAt — timestamp when PTY session was created
   * @param sendEntries — callback to send parsed JSONL entries to client
   * @param sendStatus — callback to send watcher status to client
   * @param onUserEntry — callback for user-type entries (input counting)
   * @param settingsDir — the PTY session's settingsDir (used as CWD, determines slug)
   * @param onConversationId — callback when conversationId is discovered from JSONL filename
   * @param sendSubagentEntries — callback to send subagent JSONL entries to client
   */
  constructor(
    sessionCreatedAt: number,
    sendEntries: (entries: JsonlEntry[], replay?: boolean) => boolean | SendOutcome,
    sendStatus: (status: JsonlStatus) => void,
    onUserEntry?: (entry: JsonlEntry) => void,
    settingsDir?: string,
    onConversationId?: (conversationId: string) => void,
    sendSubagentEntries?: (agentName: string, entries: JsonlEntry[], agentId?: string) => boolean | void,
    onReplayReset?: () => void,
    onStatsEntry?: (entry: JsonlEntry) => void,
    writable?: () => boolean,
  ) {
    this.writable = writable
    this.sessionCreatedAt = sessionCreatedAt
    this.sendEntries = sendEntries
    this.sendStatus = sendStatus
    this.onUserEntry = onUserEntry
    this.onConversationId = onConversationId
    this.sendSubagentEntries = sendSubagentEntries
    this.onReplayReset = onReplayReset
    this.onStatsEntry = onStatsEntry

    // Compute the slug directory for this session
    if (settingsDir) {
      const slug = computeProjectSlug(settingsDir)
      this.slugDir = path.join(PROJECTS_DIR, slug)
    } else {
      // Fallback: scan all projects (legacy behavior)
      this.slugDir = null
    }
  }

  start(): void {
    this.startedAt = Date.now()
    this.timing('watcher.start')
    this.sendStatus({ status: 'searching' })
    this.startDiscovery()
  }

  /** Milestones of the open path, stamped from `start()`.
   *
   *  Which step owns a slow chat open cannot be reasoned out from the code —
   *  three separate guesses were wrong before these numbers existed (the CLI
   *  writing the file, the mtime gate, the 512KB read). Cheap enough to leave
   *  on: a handful of lines per session open. */
  private timing(step: string, extra = ''): void {
    const ms = this.startedAt ? Date.now() - this.startedAt : 0
    debugLog(`[jsonl-timing] +${String(ms)}ms ${step}${extra ? ' ' + extra : ''}`)
  }

  /**
   * Scan subagents/ directory for JSONL files and start watchers.
   * Uses .meta.json to map agent file → agent name for WS messages.
   * Key = agent name from meta.json (e.g. "alice", "bob")
   */
  async scanSubagents(): Promise<void> {
    if (this.stopped || !this.sendSubagentEntries || this.scanningSubagents) return
    this.scanningSubagents = true
    try {
      await this.scanSubagentFiles()
    } finally {
      this.scanningSubagents = false
    }
  }

  private async scanSubagentFiles(): Promise<void> {
    // Retire watchers for subagents that finished (JSONL is append-only +
    // terminal, so no growth for SUBAGENT_RETIRE_MS means done). Without this
    // every subagent a session ever spawned kept a live 2s stat timer + inotify
    // handle until the whole session ended — thousands accumulate over hours of
    // team-spawning and burn a core / hit the inotify limit. Keep the consumed
    // offset so rediscovery cannot replay a completed transcript forever.
    const now = Date.now()
    for (const [p, w] of this.subagentWatchers) {
      if (now - w.getLastActivityAt() > SUBAGENT_RETIRE_MS) {
        let stat: fs.Stats
        try {
          stat = await fsp.stat(p)
        } catch {
          continue
        }
        if (this.stopped) return
        w.stop()
        this.retiredSubagents.set(p, {
          offset: w.getLastOffset(),
          size: stat.size,
          dev: stat.dev,
          ino: stat.ino,
          mtimeMs: stat.mtimeMs,
          birthtimeMs: stat.birthtimeMs,
          nextCheckAt: now + RETIRED_SUBAGENT_CHECK_MS,
        })
        this.subagentWatchers.delete(p)
      }
    }

    if (!this.filePath || !this.slugDir) {
      if (!this.loggedScanSkip) {
        this.loggedScanSkip = true
        console.log(`[subagent] scan skip: filePath=${!!this.filePath} slugDir=${!!this.slugDir}`)
      }
      return
    }

    const conversationId = path.basename(this.filePath, '.jsonl')
    const subagentsDir = path.join(this.slugDir, conversationId, 'subagents')

    let files: string[]
    try {
      files = await fsp.readdir(subagentsDir)
    } catch {
      if (!this.loggedScanSkip) {
        this.loggedScanSkip = true
        console.log(`[subagent] scan: no dir ${subagentsDir}`)
      }
      return
    }
    if (!this.loggedScanSkip) {
      this.loggedScanSkip = true
      console.log(`[subagent] scan: ${files.length} files in ${subagentsDir}`)
    }
    try {
      for (const file of files) {
        if (this.stopped) return
        if (!file.endsWith('.jsonl')) continue
        const jsonlPath = path.join(subagentsDir, file)
        // Already watching this file?
        if (this.subagentWatchers.has(jsonlPath)) continue

        const retired = this.retiredSubagents.get(jsonlPath)
        let startOffset: number | undefined
        if (retired) {
          if (now < retired.nextCheckAt) continue
          retired.nextCheckAt = now + RETIRED_SUBAGENT_CHECK_MS
          let stat: fs.Stats
          try {
            stat = await fsp.stat(jsonlPath)
          } catch {
            continue
          }
          if (this.stopped) return
          // dev/ino survive appends; birthtime/mtime are fallbacks for file
          // systems without inode IDs. A changed identity or shrink is new data.
          const sameFile =
            stat.ino && retired.ino
              ? stat.dev === retired.dev && stat.ino === retired.ino
              : stat.birthtimeMs && retired.birthtimeMs
                ? stat.birthtimeMs === retired.birthtimeMs
                : stat.mtimeMs === retired.mtimeMs
          if (sameFile && stat.size >= retired.size) {
            if (stat.size <= retired.offset) continue
            startOffset = retired.offset
          }
        }

        // Read .meta.json to get agent name
        const metaPath = jsonlPath.replace('.jsonl', '.meta.json')
        // agentId = unique file hash (e.g. "agent-abb9aa0ce2ca83d38"), agentName = display name from meta
        const agentId = file.replace('.jsonl', '')
        let agentName = agentId // fallback: file hash
        try {
          const raw = await fsp.readFile(metaPath, 'utf-8')
          const meta = JSON.parse(raw)
          if (meta.agentType) agentName = meta.agentType
        } catch {
          /* use fallback name */
        }
        if (this.stopped) return

        console.log(`[subagent] watcher start: ${agentName} (${path.basename(jsonlPath)})`)
        const watcher = new SubagentJsonlWatcher(
          agentName,
          jsonlPath,
          (entries) => {
            console.log(`[subagent] send ${agentName}: ${entries.length} entries`)
            return this.sendSubagentEntries!(agentName, entries, agentId)
          },
          { startOffset },
        )
        this.retiredSubagents.delete(jsonlPath)
        this.subagentWatchers.set(jsonlPath, watcher)
        void watcher.start()
      }
    } catch {
      /* directory read error */
    }
  }

  /** Start periodic scanning of subagents directory */
  private startSubagentScanning(): void {
    if (!this.sendSubagentEntries) return
    // Scan immediately, then every 4s (also retires idle watchers each pass).
    void this.scanSubagents()
    this.subagentScanTimer = setInterval(() => {
      if (this.stopped) return
      void this.scanSubagents()
    }, 4000)
  }

  getFilePath(): string | null {
    return this.filePath
  }

  /** Stream everything from a byte offset to the end, in the same byte-bounded,
   *  backpressure-aware batches the initial replay uses.
   *
   *  This is the whole point of tagging entries with `_pos`: a client that
   *  mirrors the transcript locally reconnects with "I have up to N bytes" and
   *  gets only the tail, instead of the entire file being re-read, re-sent and
   *  re-parsed on every restart. The caller MUST have verified the file's
   *  fingerprint first — resuming into a rewritten file would splice new records
   *  onto stale ones.
   *
   *  Returns the boundary just past the last DELIVERED record. The live stats
   *  hooks are not fired: this is a client top-up, the session's own replay and
   *  tail already counted these records. */
  async streamFrom(offset: number, live: () => boolean): Promise<number> {
    if (!this.filePath) return offset
    const filePath = this.filePath
    let delivered = offset
    let readTo = offset
    let batch: JsonlEntry[] = []
    let batchBytes = 0
    const flush = async (): Promise<boolean> => {
      if (batch.length === 0) return true
      const ok = await this.deliver(batch, true, live)
      batch = []
      batchBytes = 0
      if (!ok) return false
      delivered = readTo
      await delay(INITIAL_BATCH_DELAY_MS)
      return true
    }
    const abort = new AbortController()
    try {
      for await (const line of readJsonlLines(filePath, { start: offset, signal: abort.signal })) {
        if (!live()) {
          abort.abort()
          break
        }
        if (line.text.includes(SKIP_LINE_MARKER)) {
          readTo = line.end
          if (batch.length === 0) delivered = readTo
          continue
        }
        let parsed: Tagged
        try {
          parsed = JSON.parse(line.text) as Tagged
        } catch {
          if (!line.terminated) break // torn tail — the live tail delivers it
          readTo = line.end
          if (batch.length === 0) delivered = readTo
          continue
        }
        parsed._pos = line.start
        parsed._posEnd = line.end
        batch.push(parsed)
        batchBytes += line.end - line.start
        readTo = line.end
        if (batch.length >= REPLAY_BATCH_RECORDS || batchBytes >= REPLAY_BATCH_BYTES) {
          if (!(await flush())) return delivered
        }
      }
      await flush()
    } catch {
      /* aborted or unreadable — report what was delivered */
    }
    return delivered
  }

  stop(): void {
    this.stopped = true
    this.cancelReplay() // cancel any in-flight replay, including its reads
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
    if (this.tailRetryTimer) {
      clearTimeout(this.tailRetryTimer)
      this.tailRetryTimer = null
    }
    if (this.tailWatcher) {
      try {
        this.tailWatcher.close()
      } catch {}
      this.tailWatcher = null
    }
    if (this.compactionUnsub) {
      this.compactionUnsub()
      this.compactionUnsub = null
      this.compactionSub = null
    }
    // Stop subagent scanning
    if (this.subagentScanTimer) {
      clearInterval(this.subagentScanTimer)
      this.subagentScanTimer = null
    }
    // Stop all subagent watchers
    for (const watcher of this.subagentWatchers.values()) {
      watcher.stop()
    }
    this.subagentWatchers.clear()
    this.retiredSubagents.clear()
  }

  // -------------------------------------------------------------------------
  // Discovery — scan slug directory (or all projects) for newest .jsonl
  // -------------------------------------------------------------------------

  private startDiscovery(): void {
    let discovering = false
    let announcedNothingToReplay = false
    const tick = async (): Promise<void> => {
      if (this.stopped || discovering) return
      discovering = true
      try {
        const found = await this.scanForJsonl()
        this.timing('discovery.tick', found ? 'FOUND' : 'no-file')
        if (found && !this.stopped) {
          if (this.discoveryTimer) {
            clearInterval(this.discoveryTimer)
            this.discoveryTimer = null
          }
          if (this.discoveryTimeout) {
            clearTimeout(this.discoveryTimeout)
            this.discoveryTimeout = null
          }
          this.filePath = found
          this.startTailing()
        } else if (!this.stopped && !announcedNothingToReplay) {
          // No transcript yet — the case for EVERY brand-new chat, since the CLI
          // writes its JSONL only once the first turn happens. `replayComplete`
          // is the client's sole "the chat is loaded" signal (it binds the tab,
          // drops the load-cover and enables the composer), so staying quiet
          // parked every new chat under the cover for the client's full 20s
          // give-up watchdog. There is nothing to replay, which is an ANSWER.
          // Discovery keeps polling; when the file appears the normal replay
          // sends the entries and its own replayComplete.
          announcedNothingToReplay = true
          this.sendStatus({ status: 'searching', replayComplete: true })
        }
      } finally {
        discovering = false
      }
    }

    // Scan immediately: waiting a poll interval delayed every session's open by
    // that much, and delayed the "nothing to replay" answer above with it.
    void tick()
    this.discoveryTimer = setInterval(() => {
      void tick()
    }, DISCOVERY_POLL_MS)

    this.discoveryTimeout = setTimeout(() => {
      if (this.stopped || this.filePath) return
      if (this.discoveryTimer) clearInterval(this.discoveryTimer)
      this.sendStatus({ status: 'searching' })
      this.discoveryTimer = setInterval(() => {
        void tick()
      }, 5000)
    }, DISCOVERY_TIMEOUT_MS)
  }

  private async scanForJsonl(): Promise<string | null> {
    if (this.slugDir) {
      return this.scanDirectory(this.slugDir)
    }

    // Fallback: scan all slug directories under ~/.claude/projects/
    let slugDirs: fs.Dirent[]
    try {
      slugDirs = await fsp.readdir(PROJECTS_DIR, { withFileTypes: true })
    } catch {
      return null
    }

    let bestPath: string | null = null
    let bestMtime = 0

    const results = await Promise.all(
      slugDirs.filter((d) => d.isDirectory()).map((d) => this.scanDirectory(path.join(PROJECTS_DIR, d.name))),
    )
    for (const found of results) {
      if (!found) continue
      try {
        const stat = await fsp.stat(found)
        if (stat.mtimeMs > bestMtime) {
          bestMtime = stat.mtimeMs
          bestPath = found
        }
      } catch {
        /* skip */
      }
    }
    return bestPath
  }

  /**
   * Scan a single directory for the newest .jsonl file created after session start.
   */
  private async scanDirectory(dir: string): Promise<string | null> {
    let files: fs.Dirent[]
    try {
      files = await fsp.readdir(dir, { withFileTypes: true })
    } catch {
      return null
    }

    let bestPath: string | null = null
    let bestMtime = 0

    await Promise.all(
      files
        .filter((f) => f.isFile() && f.name.endsWith('.jsonl'))
        .map(async (file) => {
          const filePath = path.join(dir, file.name)
          try {
            const stat = await fsp.stat(filePath)
            if (stat.mtimeMs > this.sessionCreatedAt && stat.mtimeMs > bestMtime) {
              bestMtime = stat.mtimeMs
              bestPath = filePath
            }
          } catch {
            /* skip */
          }
        }),
    )

    return bestPath
  }

  // -------------------------------------------------------------------------
  // Tailing — read existing content, then poll for new data
  // -------------------------------------------------------------------------

  private startTailing(): void {
    if (this.stopped || !this.filePath) return

    // Extract conversationId from filename (UUID without .jsonl extension)
    if (this.onConversationId) {
      const conversationId = path.basename(this.filePath, '.jsonl')
      this.onConversationId(conversationId)
    }

    this.sendStatus({ status: 'watching', filePath: this.filePath })

    // Start scanning subagents/ directory for JSONL files
    this.startSubagentScanning()

    this.readInitialContent()

    // Compaction detection: subscribe to the shared scanner instead of running
    // our own 6s readdir timer — N sessions in one project would otherwise each
    // re-scan the same slug dir every tick.
    this.compactionSub = {
      dir: this.slugDir || path.dirname(this.filePath),
      currentFilePath: this.filePath,
      sessionCreatedAt: this.sessionCreatedAt,
      onNewer: (candidate) => {
        if (!this.stopped) this.switchToFile(candidate)
      },
    }
    this.compactionUnsub = compactionScanner.subscribe(this.compactionSub)
  }

  /**
   * Switch to a new JSONL file (compaction detected).
   * Stops polling old file, reads new file from beginning.
   */
  private switchToFile(newPath: string): void {
    // Stop current polling + native watcher
    if (this.tailTimer) {
      clearInterval(this.tailTimer)
      this.tailTimer = null
    }
    if (this.tailWatcher) {
      try {
        this.tailWatcher.close()
      } catch {}
      this.tailWatcher = null
    }
    // Kill any pending backpressure retry — it was scheduled against the OLD
    // file/offset; letting it fire after the switch would re-read the new file
    // from a stale offset and re-fire the per-entry stats/user hooks (#78).
    if (this.tailRetryTimer) {
      clearTimeout(this.tailRetryTimer)
      this.tailRetryTimer = null
    }

    this.filePath = newPath
    this.lastOffset = 0
    this.reading = false
    // Keep the shared compaction scanner pointed at the file we now tail, so it
    // compares candidates against the NEW current file, not the compacted one.
    if (this.compactionSub) this.compactionSub.currentFilePath = newPath

    // Extract conversationId from new filename (compaction creates a new conversation)
    if (this.onConversationId) {
      const conversationId = path.basename(newPath, '.jsonl')
      this.onConversationId(conversationId)
    }

    // Notify client about the switch — compacted flag tells renderer to clear old entries
    this.sendStatus({ status: 'watching', filePath: newPath, compacted: true })

    // Read new file from beginning
    this.readInitialContent()
  }

  /** Rebuild the compact-segment index from the CURRENT file and push it to the
   *  client, so a live /compact updates the segment strip without a reload.
   *
   *  A streaming pass that keeps only the running counts (SegmentIndexBuilder),
   *  never the parsed records. It does NOT resend entries or touch
   *  globalOrdCounter — only boundaries + counts are needed.
   *
   *  Coalesced: a boundary arriving mid-rebuild sets a re-run flag rather than
   *  overlapping two parses (which could deliver stale-then-fresh out of order). */
  private async rebuildSegmentIndex(): Promise<void> {
    if (!this.filePath || this.stopped) return
    if (this.rebuildingIndex) {
      this.rebuildIndexAgain = true
      return
    }
    this.rebuildingIndex = true
    try {
      do {
        this.rebuildIndexAgain = false
        const filePath = this.filePath
        if (!filePath || this.stopped) return
        const gen = this.replayGen
        const signal = this.replayAbort?.signal
        const live = () => !this.stopped && this.filePath === filePath && this.replayGen === gen
        const builder = new SegmentIndexBuilder()
        try {
          for await (const line of readJsonlLines(filePath, { signal })) {
            if (!live()) return
            const parsed = parseJsonlLine(line.text)
            if (parsed) builder.add(parsed)
          }
        } catch {
          return
        }
        if (!live()) return
        this.sendStatus({ status: 'watching', filePath, segmentIndex: builder.build() })
      } while (this.rebuildIndexAgain && !this.stopped)
    } finally {
      this.rebuildingIndex = false
      if (this.rebuildIndexAgain && !this.stopped) void this.rebuildSegmentIndex()
    }
  }

  /** Paint the newest turns BEFORE the full parse.
   *
   *  The full replay is two streaming passes over the whole transcript; the user
   *  only ever looks at the tail first, and the tail is a bounded read: seek
   *  near the end, parse a few hundred records, send.
   *
   *  Safety, since the authoritative pass re-sends these same records:
   *   - Only records WITH a uuid are previewed. The client dedups those by
   *     uuid, so the second delivery is a no-op. A uuid-less row (system/tool)
   *     is deduped by `_ord` alone, and the two passes number `_ord`
   *     differently — previewing one would duplicate it on screen.
   *   - `_ord` here is `base + byte offset`, and `globalOrdCounter` is then
   *     advanced past the whole file, so the authoritative pass's `_ord`s
   *     cannot collide with a preview value (a collision would make the
   *     client's `_ord` dedup drop a DIFFERENT, unsent record).
   *   - `_pos`/`_posEnd` are exact byte offsets, so a mirrored preview record
   *     is as valid as the authoritative one.
   *   - Stats hooks are deliberately NOT fired: the full pass counts these
   *     records, and firing here would double them. */
  private async emitTailPreview(size: number, live: () => boolean, signal: AbortSignal): Promise<void> {
    const PREVIEW_BYTES = 512 * 1024
    const PREVIEW_MAX_RECORDS = 200
    // Below the window the full parse is already fast; a preview would only add
    // a redundant delivery.
    if (!this.filePath || size <= PREVIEW_BYTES) return

    const start = size - PREVIEW_BYTES
    const buf = await readRange(this.filePath, start, size)
    if (!live()) return

    // The window almost certainly cuts through a record; its bytes belong to a
    // line we cannot parse, so skip to the first newline. Framing is on bytes,
    // so a multi-byte character can never be split.
    const firstNl = buf.indexOf(0x0a)
    if (firstNl === -1) return

    const base = globalOrdCounter
    const out: JsonlEntry[] = []
    for (let a = firstNl + 1; a < buf.length;) {
      const nl = buf.indexOf(0x0a, a)
      if (nl === -1) break // torn last line — the full pass decides about it
      const thisLineStart = start + a
      const lineEnd = start + nl + 1
      const text = buf.toString('utf8', a, nl)
      a = nl + 1
      if (!text.trim() || text.includes(SKIP_LINE_MARKER)) continue
      let parsed: JsonlEntry
      try {
        parsed = JSON.parse(text) as JsonlEntry
      } catch {
        continue
      }
      if (!parsed.uuid) continue
      const tagged = parsed as Tagged
      tagged._pos = thisLineStart
      tagged._posEnd = lineEnd
      tagged._ord = base + thisLineStart
      out.push(parsed)
    }
    // Reserve the whole file's offset range so the authoritative pass numbers
    // strictly above every preview `_ord`.
    globalOrdCounter = base + size + 1
    const tail = out.slice(-PREVIEW_MAX_RECORDS)
    if (tail.length === 0) return

    // Deliver it, don't just fire it: right after a resume the client's socket
    // is often not attached yet. Retry a bounded number of times.
    for (let attempt = 0; attempt < PREVIEW_SEND_ATTEMPTS; attempt++) {
      if (!live() || signal.aborted) return
      if (this.writable && !this.writable()) {
        await delay(REPLAY_BACKPRESSURE_RETRY_MS)
        continue
      }
      const r = normalizeOutcome(this.sendEntries(tail, true))
      if (r === 'sent' || r === 'oversized') return
      await delay(REPLAY_BACKPRESSURE_RETRY_MS)
    }
  }

  /** Invalidate any running replay: bump the generation and abort its I/O. */
  private cancelReplay(): void {
    this.replayGen++
    this.replayAbort?.abort()
    this.replayAbort = null
  }

  /** Wait until the client socket can take another frame (or the caller is no
   *  longer live). Polls a cheap probe — nothing is re-read or re-serialised. */
  private async waitWritable(live: () => boolean): Promise<boolean> {
    let wait = DRAIN_POLL_MS
    while (live()) {
      if (!this.writable || this.writable()) return true
      await delay(wait)
      wait = Math.min(REPLAY_BACKPRESSURE_RETRY_MS, wait * 2)
    }
    return false
  }

  /** Deliver a batch with backpressure. Temporary pressure waits for drain;
   *  a frame that can never fit is split by halves, and a single record that
   *  can never fit is replaced by an explicit stub — never retried forever. */
  private async deliver(entries: JsonlEntry[], replay: boolean, live: () => boolean): Promise<boolean> {
    if (entries.length === 0) return true
    for (;;) {
      if (!(await this.waitWritable(live))) return false
      const r = normalizeOutcome(this.sendEntries(entries, replay))
      if (r === 'sent') return true
      if (r === 'oversized') {
        if (entries.length > 1) {
          const mid = entries.length >> 1
          return (
            (await this.deliver(entries.slice(0, mid), replay, live)) && this.deliver(entries.slice(mid), replay, live)
          )
        }
        const e = entries[0] as Tagged
        const bytes =
          typeof e._pos === 'number' && typeof e._posEnd === 'number'
            ? e._posEnd - e._pos
            : Buffer.byteLength(JSON.stringify(e), 'utf8')
        console.warn('[jsonl] record too large for one frame; sending a stub', { uuid: e.uuid, bytes })
        const stub = normalizeOutcome(this.sendEntries([oversizedStub(e, bytes)], replay))
        if (stub === 'sent' || stub === 'oversized') return true
        continue // the stub met ordinary pressure: wait and retry the record
      }
      // backpressured / closed: wait for drain (closed → live() decides).
      await delay(DRAIN_POLL_MS)
    }
  }

  private async readInitialContent(): Promise<void> {
    if (this.stopped || !this.filePath) return
    const filePath = this.filePath
    // Capture the generation: any switchToFile/stop/replayAll after this point
    // invalidates this run at the next await boundary and aborts its reads.
    this.cancelReplay()
    const gen = this.replayGen
    const abort = new AbortController()
    this.replayAbort = abort
    const signal = abort.signal
    const live = () => !this.stopped && gen === this.replayGen
    let segmentIndex: SegmentIndex | undefined

    // Full replay restarts the live dashboard counters from zero — the reads
    // below re-accumulate the whole file, so a reattach won't double-count.
    this.onReplayReset?.()

    try {
      const stat = await fsp.stat(filePath)
      this.timing('readInitialContent.stat', `${String(Math.round(stat.size / 1024))}KB`)
      if (!live()) return
      this.fileIdentity = `${stat.dev}:${stat.ino}`
      if (stat.size === 0) {
        this.lastOffset = 0
        this.startPolling()
        // An empty transcript still has to SAY it finished: `replayComplete` is
        // the client's only signal that the chat is done loading.
        this.sendStatus({ status: 'watching', filePath, replayComplete: true })
        return
      }

      // Show the newest turns now, instead of after the whole file is indexed.
      // Best-effort: a failure here must not cost the real replay.
      try {
        await this.emitTailPreview(stat.size, live, signal)
        this.timing('tailPreview.done')
      } catch (err) {
        if (!live()) return
        console.warn('[jsonl] tail preview failed (continuing with full replay)', err)
      }
      if (!live()) return

      // Pass 1 — index. One forward read; each record is parsed once for the
      // stats hooks (which need file order) and the segment index, and only
      // its byte range is kept.
      const idx = new RecordIndex()
      const builder = new SegmentIndexBuilder()
      const bounds: number[] = []
      let endPos = 0
      for await (const line of readJsonlLines(filePath, { signal, end: stat.size })) {
        if (line.text.includes(SKIP_LINE_MARKER)) {
          endPos = line.end
          continue
        }
        let parsed: JsonlEntry
        try {
          parsed = JSON.parse(line.text) as JsonlEntry
        } catch {
          // A torn LAST line stays unconsumed so the tail re-reads it whole;
          // a malformed line in the middle is skipped for good.
          if (line.terminated) endPos = line.end
          continue
        }
        endPos = line.end
        if (parsed.type === 'system' && (parsed as { subtype?: string }).subtype === 'compact_boundary') {
          bounds.push(idx.length)
        }
        idx.push(line.start, line.end)
        builder.add(parsed)
        if (!live()) return
        // Stats are reset per replay and re-accumulated, so they fold every
        // record. `onUserEntry` is NOT fired for history: it records a NEW
        // user input (a request row + inputCount), and firing it on every
        // replay re-inserted one fresh request row per historical prompt on
        // each resume/reattach — thousands of unawaited DuckDB writes per
        // replay of a large transcript, and an ever-growing requests table.
        this.onStatsEntry?.(parsed)
      }
      if (!live()) return
      // An atomic replacement can happen while the old descriptor is read.
      // Never commit its offset as the cursor of the replacement file.
      const currentStat = await fsp.stat(filePath)
      if (!live()) return
      if (`${currentStat.dev}:${currentStat.ino}` !== this.fileIdentity || currentStat.size < endPos) {
        this.replayRewrittenFile(filePath)
        return
      }
      this.timing('index.done', `${String(idx.length)} records`)
      const base = globalOrdCounter
      globalOrdCounter = base + idx.length
      // The tail picks up from where THIS read actually stopped — not from a
      // pre-read stat.size, which raced with concurrent CLI appends.
      this.lastOffset = endPos
      segmentIndex = builder.build()
      const ranges: Array<[number, number]> = []
      let segStart = 0
      for (const b of bounds) {
        ranges.push([segStart, b])
        segStart = b
      }
      ranges.push([segStart, idx.length])

      // Byte-progress of the replay, for the client's "loading history … N%".
      // Records don't overlap, so summing each delivered batch's own byte span
      // is honest monotonic coverage regardless of delivery order.
      const total = stat.size
      let sentBytes = 0
      const emitProgress = (from: number, to: number): void => {
        if (idx.length <= REPLAY_BATCH_RECORDS) return // single batch — no bar
        sentBytes = Math.min(total, sentBytes + batchSpan(idx, from, to))
        this.sendStatus({ status: 'watching', filePath, replayProgress: { sent: sentBytes, total } })
      }
      // Pass 2 — emit, one byte-bounded batch in memory at a time.
      const emitBatch = async (from: number, to: number): Promise<boolean> => {
        if (!live()) return false
        const batch = await loadBatch(filePath, idx, from, to, base)
        if (!live()) return false
        if (!(await this.deliver(batch, true, live))) return false
        emitProgress(from, to)
        await delay(INITIAL_BATCH_DELAY_MS)
        return true
      }
      const emitRange = async ([s, e]: [number, number]): Promise<boolean> => {
        for (let i = s; i < e;) {
          const to = batchForward(idx, i, e)
          if (!(await emitBatch(i, to))) return false
          i = to
        }
        return true
      }
      /** Emit a range from its END backwards, newest batch first: the current
       *  segment's own tail is what the user looks at. Out-of-order delivery
       *  is safe by construction — every entry carries `_ord`. */
      const emitRangeFromEnd = async ([s, e]: [number, number]): Promise<boolean> => {
        for (let end = e; end > s;) {
          const from = batchBackward(idx, end, s)
          if (!(await emitBatch(from, end))) return false
          end = from
        }
        return true
      }
      // Tail can start as soon as offset is known — emit replay concurrently.
      this.startPolling()
      if (!(await emitRangeFromEnd(ranges[ranges.length - 1]!))) return // current dialog first
      for (let s = ranges.length - 2; s >= 0; s--) {
        if (!(await emitRange(ranges[s]!))) return // older segments, newest → oldest
      }
      if (!live()) return
    } catch (err) {
      if (!live()) return
      this.sendStatus({
        status: 'error',
        error: `Failed to read initial content: ${String(err)}`,
      })
      this.startPolling()
      this.sendStatus({ status: 'watching', filePath, replayComplete: true })
      return
    } finally {
      if (this.replayAbort === abort) this.replayAbort = null
    }

    // Signal that all historical entries have been sent — and hand over the
    // file's fingerprint, so a client mirroring the transcript can later prove
    // its copy is a prefix of this same file.
    const fp = await fingerprintTranscript(filePath)
    if (!live()) return
    this.sendStatus({
      status: 'watching',
      filePath,
      replayComplete: true,
      ...(fp ? { fileHead: fp.head, fileSize: fp.size } : {}),
      ...(segmentIndex ? { segmentIndex } : {}),
    })
  }

  /**
   * Re-emit the whole current file (all segments) to the client. Used when a
   * client reattaches to a live session — its chat state starts empty and
   * needs the same replay a fresh resume would get. Tailing continues; the
   * generation bump cancels any still-running previous replay.
   */
  replayAll(): void {
    if (this.stopped || !this.filePath) return
    // Stop the live tail BEFORE re-reading from byte 0 (like switchToFile does):
    // otherwise the running poller/watcher and this full replay both parse any
    // line the CLI appends during the reattach window, double-folding it into
    // the live stat counters. readInitialContent → startPolling re-arms the tail
    // from the fresh offset once it's known.
    if (this.tailTimer) {
      clearInterval(this.tailTimer)
      this.tailTimer = null
    }
    if (this.tailWatcher) {
      try {
        this.tailWatcher.close()
      } catch {}
      this.tailWatcher = null
    }
    // Same as switchToFile: a pending backpressure retry would race this fresh
    // replay and re-fire stats hooks against re-read entries (#78).
    if (this.tailRetryTimer) {
      clearTimeout(this.tailRetryTimer)
      this.tailRetryTimer = null
    }
    // The new replay owns subsequent reads. An old tail continuation must not
    // keep the new poller locked or clear its lock on eventual completion.
    this.reading = false
    // Субагенты ТОЖЕ с нуля: живой watcher — skip в scanSubagents (его реплей
    // не повторится), завершённый — retired через 5 мин. В обоих случаях
    // реаттачнутый клиент открывал агента на «No messages from this agent
    // yet» — история агента живёт только в этих файлах. Пересоздание
    // watcher'ов реплеит их целиком; клиент дедупит по uuid.
    for (const w of this.subagentWatchers.values()) w.stop()
    this.subagentWatchers.clear()
    this.retiredSubagents.clear()
    void this.scanSubagents()
    void this.readInitialContent()
  }

  private replayRewrittenFile(filePath: string): void {
    if (this.stopped || this.filePath !== filePath) return
    // Reuse the existing cache/mirror reset event, without changing the
    // conversation identity: a same-path repair still belongs to this chat.
    this.sendStatus({ status: 'watching', filePath, compacted: true, replayComplete: false })
    this.lastOffset = 0
    this.replayAll()
  }

  private startPolling(): void {
    // Clear previous timers/watchers if any (from switchToFile)
    if (this.tailTimer) {
      clearInterval(this.tailTimer)
      this.tailTimer = null
    }
    if (this.tailWatcher) {
      try {
        this.tailWatcher.close()
      } catch {}
      this.tailWatcher = null
    }
    if (!this.filePath) return

    // Primary: fs.watch (native FS events) — 0 CPU when idle, reacts within ms
    // of a write. Slight debounce because editors can emit several events per
    // save; also guards against re-entrant reads.
    try {
      let debounceTimer: ReturnType<typeof setTimeout> | null = null
      const schedule = () => {
        if (debounceTimer) clearTimeout(debounceTimer)
        debounceTimer = setTimeout(() => {
          debounceTimer = null
          if (this.stopped || this.reading) return
          this.checkForNewContent()
        }, 20)
      }
      this.tailWatcher = fs.watch(this.filePath, { persistent: false }, (eventType) => {
        if (this.stopped) return
        if (eventType === 'rename') {
          // The underlying file was renamed/replaced — fall back to a fresh
          // poll so we pick up compactions / atomic writes without crashing
          // the watcher.
          if (this.tailWatcher) {
            try {
              this.tailWatcher.close()
            } catch {}
            this.tailWatcher = null
          }
          schedule()
          return
        }
        schedule()
      })
      // The FSWatcher can emit an async 'error' (inotify limits, file removed on
      // some FSes) AFTER creation; without this listener that throws → server
      // crash. Close it and lean on the polling fallback instead.
      this.tailWatcher.on('error', () => {
        if (this.tailWatcher) {
          try {
            this.tailWatcher.close()
          } catch {}
          this.tailWatcher = null
        }
        if (!this.stopped) schedule()
      })
    } catch {
      // fs.watch may fail on network drives / certain FSes — keep going with the
      // polling fallback below.
      this.tailWatcher = null
    }

    // Safety-net poll at a much lower rate than the old 200ms — fs.watch
    // misses events only on rare filesystems (network, sshfs). When fs.watch is
    // live this is pure backup so it runs slowly (10s); without it we fall to
    // the aggressive TAIL_POLL_MS for streaming responsiveness.
    this.tailTimer = setInterval(
      () => {
        if (this.stopped || this.reading) return
        this.checkForNewContent()
      },
      this.tailWatcher ? WATCH_SAFETY_POLL_MS : TAIL_POLL_MS,
    )
  }

  private async checkForNewContent(): Promise<void> {
    if (!this.filePath || this.stopped) return
    // Set BEFORE the first await — fs.watch debounce and the safety-net
    // interval both check this flag, and an async gap here let them race
    // into a double read of the same range.
    if (this.reading) return
    this.reading = true
    const filePath = this.filePath
    const gen = this.replayGen
    const live = () => !this.stopped && this.filePath === filePath && this.replayGen === gen
    let more = false

    try {
      const stat = await fsp.stat(filePath)
      if (!live()) return
      if (stat.size < this.lastOffset || (this.fileIdentity && `${stat.dev}:${stat.ino}` !== this.fileIdentity)) {
        this.replayRewrittenFile(filePath)
        return
      }
      if (stat.size <= this.lastOffset) return

      // Bounded read: at most TAIL_BATCH_BYTES of complete records per pass
      // (a single longer record is still read whole). A client that was frozen
      // while the CLI wrote tens of MiB no longer costs one giant Buffer, one
      // giant string and one giant frame.
      const { entries, endPos, truncated } = await this.readEntriesFromOffset(filePath, this.lastOffset, stat.size)
      if (!live()) return
      const currentStat = await fsp.stat(filePath)
      if (!live()) return
      if (
        currentStat.size < endPos ||
        (this.fileIdentity && `${currentStat.dev}:${currentStat.ino}` !== this.fileIdentity)
      ) {
        this.replayRewrittenFile(filePath)
        return
      }

      if (entries.length === 0) {
        // Only skipped/torn lines (no complete entries yet) — advance past the
        // clean prefix and wait for the rest.
        this.lastOffset = Math.max(this.lastOffset, endPos)
        more = truncated
        return
      }

      // Deliver BEFORE advancing the offset: if the socket cannot take the
      // frame, wait for it to drain (no re-read, no re-serialise); the offset
      // only moves past entries the client actually got, so a slow client can
      // never lose transcript lines.
      if (!(await this.deliver(entries, false, live))) return
      // Delivered — commit the offset and fire the per-entry hooks (stats +
      // user-entry) exactly once, now that the range won't be re-read.
      this.lastOffset = Math.max(this.lastOffset, endPos)
      more = truncated
      let sawBoundary = false
      for (const parsed of entries) {
        this.onStatsEntry?.(parsed)
        if (parsed.type === 'user' && this.onUserEntry) this.onUserEntry(parsed)
        if (parsed.type === 'system' && (parsed as { subtype?: string }).subtype === 'compact_boundary')
          sawBoundary = true
      }
      // A live /compact appends a compact_boundary to the SAME file, so it rides
      // the tail — rebuild + resend the segment index now.
      if (sawBoundary) void this.rebuildSegmentIndex()
    } catch {
      // File might be temporarily unavailable, ignore
    } finally {
      if (live()) {
        this.reading = false
        if (more) setImmediate(() => void this.checkForNewContent())
      }
    }
  }

  /** Read complete records from `offset`, stopping once TAIL_BATCH_BYTES of
   *  them are collected. A torn trailing line is left unconsumed. */
  private async readEntriesFromOffset(
    filePath: string,
    offset: number,
    size: number,
  ): Promise<{ entries: JsonlEntry[]; endPos: number; truncated: boolean }> {
    const entries: JsonlEntry[] = []
    let endPos = offset
    let truncated = false
    for await (const line of readJsonlLines(filePath, { start: offset, end: size })) {
      if (line.text.includes(SKIP_LINE_MARKER)) {
        endPos = line.end
      } else {
        let parsed: JsonlEntry | null
        try {
          parsed = JSON.parse(line.text) as JsonlEntry
        } catch {
          parsed = null
        }
        if (parsed === null) {
          // Torn write: leave its bytes for the next poll. A malformed line
          // that IS terminated will never complete — skip it.
          if (!line.terminated) break
          endPos = line.end
        } else {
          // Live records stay untagged (unchanged wire contract for the tail).
          entries.push(parsed)
          endPos = line.end
        }
      }
      if (endPos - offset >= TAIL_BATCH_BYTES && endPos < size) {
        truncated = true
        break
      }
    }
    return { entries, endPos, truncated }
  }
}

// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function delay(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms))
}

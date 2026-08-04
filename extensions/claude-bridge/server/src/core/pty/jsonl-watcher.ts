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
import { buildSegmentIndex, type SegmentIndex } from './segment-index'
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
  try { await fsp.access(p); return true } catch { return false }
}

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const PROJECTS_DIR = path.join(os.homedir(), '.claude', 'projects')
const DISCOVERY_POLL_MS = 1000
const DISCOVERY_TIMEOUT_MS = 300_000  // 5 min — then slow-poll instead of error
const TAIL_POLL_MS = 200
// Re-check delay after a backpressure DROP — fast enough that a briefly-slow
// client recovers its missed lines within a blink, not the 10s safety poll.
const TAIL_BACKPRESSURE_RETRY_MS = 400
// Safety-net poll cadence WHEN fs.watch is live — it's pure backup for the rare
// FS that drops inotify events, so it can be slow. (Without fs.watch we fall to
// the aggressive TAIL_POLL_MS for streaming responsiveness.)
const WATCH_SAFETY_POLL_MS = 10_000
const INITIAL_BATCH_SIZE = 200
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
const SUBAGENT_DISCOVERY_TIMEOUT_MS = 120_000  // 2 min to wait for file to appear
// Retire a subagent watcher after this long with no file growth — a completed
// subagent shouldn't hold a live timer + inotify handle for the whole session.
const SUBAGENT_RETIRE_MS = 5 * 60_000

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
  private sendEntries: (entries: JsonlEntry[], replay?: boolean) => boolean
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
    sendEntries: (entries: JsonlEntry[], replay?: boolean) => boolean,
    sendStatus: (status: JsonlStatus) => void,
    onUserEntry?: (entry: JsonlEntry) => void,
    settingsDir?: string,
    onConversationId?: (conversationId: string) => void,
    sendSubagentEntries?: (agentName: string, entries: JsonlEntry[], agentId?: string) => boolean | void,
    onReplayReset?: () => void,
    onStatsEntry?: (entry: JsonlEntry) => void,
  ) {
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
    if (this.stopped || !this.sendSubagentEntries) return

    // Retire watchers for subagents that finished (JSONL is append-only +
    // terminal, so no growth for SUBAGENT_RETIRE_MS means done). Without this
    // every subagent a session ever spawned kept a live 2s stat timer + inotify
    // handle until the whole session ended — thousands accumulate over hours of
    // team-spawning and burn a core / hit the inotify limit. A retired subagent
    // that somehow resumes is re-discovered below (client dedups by uuid).
    const now = Date.now()
    for (const [p, w] of this.subagentWatchers) {
      if (now - w.getLastActivityAt() > SUBAGENT_RETIRE_MS) {
        w.stop()
        this.subagentWatchers.delete(p)
      }
    }

    if (!this.filePath || !this.slugDir) {
      if (!this.loggedScanSkip) { this.loggedScanSkip = true; console.log(`[subagent] scan skip: filePath=${!!this.filePath} slugDir=${!!this.slugDir}`) }
      return
    }

    const conversationId = path.basename(this.filePath, '.jsonl')
    const subagentsDir = path.join(this.slugDir, conversationId, 'subagents')

    let files: string[]
    try {
      files = await fsp.readdir(subagentsDir)
    } catch {
      if (!this.loggedScanSkip) { this.loggedScanSkip = true; console.log(`[subagent] scan: no dir ${subagentsDir}`) }
      return
    }
    if (!this.loggedScanSkip) { this.loggedScanSkip = true; console.log(`[subagent] scan: ${files.length} files in ${subagentsDir}`) }
    try {
      for (const file of files) {
        if (!file.endsWith('.jsonl')) continue
        const jsonlPath = path.join(subagentsDir, file)
        // Already watching this file?
        if (this.subagentWatchers.has(jsonlPath)) continue

        // Read .meta.json to get agent name
        const metaPath = jsonlPath.replace('.jsonl', '.meta.json')
        // agentId = unique file hash (e.g. "agent-abb9aa0ce2ca83d38"), agentName = display name from meta
        const agentId = file.replace('.jsonl', '')
        let agentName = agentId // fallback: file hash
        try {
          const raw = await fsp.readFile(metaPath, 'utf-8')
          const meta = JSON.parse(raw)
          if (meta.agentType) agentName = meta.agentType
        } catch { /* use fallback name */ }

        console.log(`[subagent] watcher start: ${agentName} (${path.basename(jsonlPath)})`)
        const watcher = new SubagentJsonlWatcher(
          agentName,
          jsonlPath,
          (entries) => {
            console.log(`[subagent] send ${agentName}: ${entries.length} entries`)
            return this.sendSubagentEntries!(agentName, entries, agentId)
          },
        )
        this.subagentWatchers.set(jsonlPath, watcher)
        void watcher.start()
      }
    } catch { /* directory read error */ }
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

  /** Stream everything from a byte offset to the end, in the same bounded,
   *  backpressure-aware batches the initial replay uses.
   *
   *  This is the whole point of tagging entries with `_pos`: a client that
   *  mirrors the transcript locally reconnects with "I have up to N bytes" and
   *  gets only the tail, instead of the entire 85MB file being re-read, re-sent
   *  and re-parsed on every restart. The caller MUST have verified the file's
   *  fingerprint first — resuming into a rewritten file would splice new records
   *  onto stale ones. */
  async streamFrom(offset: number, live: () => boolean): Promise<number> {
    if (!this.filePath) return offset
    return this.readEntriesStreaming(this.filePath, offset, async (batch) => {
      for (let i = 0; i < batch.length;) {
        if (!live()) return
        const slice = batch.slice(i, i + INITIAL_BATCH_SIZE)
        if (this.sendEntries(slice, true) === false) { await delay(REPLAY_BACKPRESSURE_RETRY_MS); continue }
        i += INITIAL_BATCH_SIZE
        await delay(INITIAL_BATCH_DELAY_MS)
      }
    }, live)
  }

  stop(): void {
    this.stopped = true
    this.replayGen++ // cancel any in-flight initial replay
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
      try { this.tailWatcher.close() } catch {}
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
          if (this.discoveryTimer) { clearInterval(this.discoveryTimer); this.discoveryTimer = null }
          if (this.discoveryTimeout) { clearTimeout(this.discoveryTimeout); this.discoveryTimeout = null }
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
    this.discoveryTimer = setInterval(() => { void tick() }, DISCOVERY_POLL_MS)

    this.discoveryTimeout = setTimeout(() => {
      if (this.stopped || this.filePath) return
      if (this.discoveryTimer) clearInterval(this.discoveryTimer)
      this.sendStatus({ status: 'searching' })
      this.discoveryTimer = setInterval(() => { void tick() }, 5000)
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
      slugDirs
        .filter((d) => d.isDirectory())
        .map((d) => this.scanDirectory(path.join(PROJECTS_DIR, d.name))),
    )
    for (const found of results) {
      if (!found) continue
      try {
        const stat = await fsp.stat(found)
        if (stat.mtimeMs > bestMtime) {
          bestMtime = stat.mtimeMs
          bestPath = found
        }
      } catch { /* skip */ }
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
          } catch { /* skip */ }
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
      onNewer: (candidate) => { if (!this.stopped) this.switchToFile(candidate) },
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
      try { this.tailWatcher.close() } catch {}
      this.tailWatcher = null
    }
    // Kill any pending backpressure retry — it was scheduled against the OLD
    // file/offset; letting it fire after the switch would re-read the new file
    // from a stale offset and re-fire the per-entry stats/user hooks (#78).
    if (this.tailRetryTimer) { clearTimeout(this.tailRetryTimer); this.tailRetryTimer = null }

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
   *  Deliberately a fresh FULL parse (same as readInitialContent's), not an
   *  incremental patch: buildSegmentIndex counts VISIBLE entries per segment,
   *  which needs every record, and compaction is rare enough that the re-read
   *  cost is a non-issue. It does NOT resend entries or touch globalOrdCounter —
   *  only boundaries + counts are needed — so it can't perturb entry ordering.
   *
   *  Coalesced: a boundary arriving mid-rebuild sets a re-run flag rather than
   *  overlapping two parses (which could deliver stale-then-fresh out of order). */
  private async rebuildSegmentIndex(): Promise<void> {
    if (!this.filePath || this.stopped) return
    if (this.rebuildingIndex) { this.rebuildIndexAgain = true; return }
    this.rebuildingIndex = true
    try {
      do {
        this.rebuildIndexAgain = false
        const filePath = this.filePath
        if (!filePath || this.stopped) return
        const all: JsonlEntry[] = []
        const live = () => !this.stopped && this.filePath === filePath
        try {
          await this.readEntriesStreaming(filePath, 0, async (batch) => { all.push(...batch) }, live)
        } catch { return }
        if (!live()) return
        const bounds: number[] = []
        all.forEach((e, i) => { if (e.type === 'system' && (e as { subtype?: string }).subtype === 'compact_boundary') bounds.push(i) })
        this.sendStatus({ status: 'watching', filePath, segmentIndex: buildSegmentIndex(all, bounds) })
      } while (this.rebuildIndexAgain && !this.stopped)
    } finally {
      this.rebuildingIndex = false
    }
  }

  /** Paint the newest turns BEFORE the full parse.
   *
   *  `readInitialContent` reads and JSON-parses the WHOLE transcript into
   *  memory before it emits its first batch — on a 15MB session that measured
   *  17.3s of empty load-cover, with even the progress percent not appearing
   *  until then (the streaming reader's batches were all accumulated, never
   *  forwarded). The user only ever looks at the tail first, and the tail is a
   *  bounded read: seek near the end, parse a few hundred records, send.
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
  private async emitTailPreview(size: number, live: () => boolean): Promise<void> {
    const PREVIEW_BYTES = 512 * 1024
    const PREVIEW_MAX_RECORDS = 200
    // Below the window the full parse is already fast; a preview would only add
    // a redundant delivery.
    if (!this.filePath || size <= PREVIEW_BYTES) return

    const start = size - PREVIEW_BYTES
    let text: string
    const fd = await fsp.open(this.filePath, 'r')
    try {
      const buf = Buffer.alloc(PREVIEW_BYTES)
      const { bytesRead } = await fd.read(buf, 0, PREVIEW_BYTES, start)
      text = buf.toString('utf8', 0, bytesRead)
    } finally {
      await fd.close()
    }
    if (!live()) return

    // The window almost certainly cuts through a record; its bytes belong to a
    // line we cannot parse, so skip to the first newline.
    const firstNl = text.indexOf('\n')
    if (firstNl === -1) return
    let lineStart = start + Buffer.byteLength(text.slice(0, firstNl + 1), 'utf8')

    const base = globalOrdCounter
    const out: JsonlEntry[] = []
    for (const line of text.slice(firstNl + 1).split('\n')) {
      const thisLineStart = lineStart
      lineStart += Buffer.byteLength(line, 'utf8') + 1
      if (!line.trim() || line.includes(SKIP_LINE_MARKER)) continue
      let parsed: JsonlEntry
      try { parsed = JSON.parse(line) as JsonlEntry } catch { continue }
      if (!parsed.uuid) continue
      const tagged = parsed as JsonlEntry & { _pos?: number; _posEnd?: number; _ord?: number }
      tagged._pos = thisLineStart
      tagged._posEnd = lineStart
      tagged._ord = base + thisLineStart
      out.push(parsed)
    }
    // Reserve the whole file's offset range so the authoritative pass numbers
    // strictly above every preview `_ord`.
    globalOrdCounter = base + size + 1
    const tail = out.slice(-PREVIEW_MAX_RECORDS)
    if (tail.length === 0) return

    // Deliver it, don't just fire it. `sendEntries` returns false when the frame
    // went nowhere — and right after a resume the client's socket is often not
    // attached yet, because the tab was created milliseconds ago. Ignoring that
    // made the preview a coin toss: measured, it left the server at 126ms while
    // the chat stayed empty until the FULL parse landed 14.6s later, since
    // nothing re-sent it. Retry on the same cadence the bulk replay uses.
    for (let attempt = 0; attempt < PREVIEW_SEND_ATTEMPTS; attempt++) {
      if (!live()) return
      if (this.sendEntries(tail, true) !== false) return
      await delay(REPLAY_BACKPRESSURE_RETRY_MS)
    }
  }

  private async readInitialContent(): Promise<void> {
    if (this.stopped || !this.filePath) return
    // Capture the generation: any switchToFile/stop/replayAll after this point
    // invalidates this run at the next await boundary.
    const gen = ++this.replayGen
    const live = () => !this.stopped && gen === this.replayGen
    // Built from the full parse below and handed to the client with
    // replayComplete — the authoritative compact-segment index (every segment +
    // visible counts), which the client's bounded mirror cannot produce.
    let segmentIndex: SegmentIndex | undefined

    // Full replay restarts the live dashboard counters from zero — the reads
    // below re-accumulate the whole file, so a reattach won't double-count.
    this.onReplayReset?.()

    try {
      const stat = await fsp.stat(this.filePath)
      this.timing('readInitialContent.stat', `${String(Math.round(stat.size / 1024))}KB`)
      if (!live()) return
      if (stat.size === 0) {
        this.lastOffset = 0
        this.startPolling()
        // An empty transcript still has to SAY it finished. `replayComplete` is
        // the client's only signal that the chat is done loading — it binds the
        // tab, drops the load-cover and enables the composer on it. Returning
        // silently left a brand-new session (whose file is empty by definition)
        // covered until the client's 20s give-up watchdog, with the phase
        // timeline reading "waiting on replay-status". There is nothing to
        // replay, which is an ANSWER, not a reason to stay quiet.
        this.sendStatus({ status: 'watching', filePath: this.filePath, replayComplete: true })
        return
      }

      // Show the newest turns now, instead of after the whole file is parsed.
      // Best-effort: a failure here must not cost the real replay.
      try {
        await this.emitTailPreview(stat.size, live)
        this.timing('tailPreview.done')
      } catch (err) {
        console.warn('[jsonl] tail preview failed (continuing with full replay)', err)
      }
      if (!live()) return

      // Reverse-segment replay: paint the CURRENT (post-last-compact) dialog
      // FIRST, then stream older segments newest→oldest in the background so a
      // long session doesn't block on parsing its whole history. Each entry
      // carries `_ord` (its position in file order) so the client keeps them
      // chronological regardless of this out-of-order delivery.
      const all: JsonlEntry[] = []
      const endPos = await this.readEntriesStreaming(this.filePath, 0, async (batch) => { all.push(...batch) }, live)
      if (!live()) return
      this.timing('fullParse.done', `${String(all.length)} records`)
      const base = globalOrdCounter
      all.forEach((e, i) => { (e as JsonlEntry & { _ord?: number })._ord = base + i })
      globalOrdCounter = base + all.length
      // The tail picks up from where THIS read actually stopped — not from a
      // pre-read stat.size, which raced with concurrent CLI appends.
      this.lastOffset = endPos
      // Split into segments at each compact boundary.
      const bounds: number[] = []
      all.forEach((e, i) => { if (e.type === 'system' && (e as { subtype?: string }).subtype === 'compact_boundary') bounds.push(i) })
      // Same pass, same data: the full compact-segment index for the strip.
      segmentIndex = buildSegmentIndex(all, bounds)
      const ranges: Array<[number, number]> = []
      let segStart = 0
      for (const b of bounds) { ranges.push([segStart, b]); segStart = b }
      ranges.push([segStart, all.length])
      // Byte-progress of the replay, for the client's "loading history … N%".
      // `total` is the file size; `sentBytes` accumulates the byte span of every
      // batch actually delivered. Delivery is newest-first and out of order, but
      // the records don't overlap, so summing each batch's own byte span gives
      // honest monotonic coverage regardless of order — the same reason `_ord`
      // makes out-of-order delivery safe. Emitted per batch (a few hundred tiny
      // messages over a multi-second replay), skipped for a file small enough to
      // arrive in one batch (nothing to show progress for).
      const total = stat.size
      let sentBytes = 0
      const batchSpan = (batch: JsonlEntry[]): number => {
        let span = 0
        for (const e of batch) {
          const r = e as JsonlEntry & { _pos?: number; _posEnd?: number }
          if (typeof r._pos === 'number' && typeof r._posEnd === 'number') span += r._posEnd - r._pos
        }
        return span
      }
      const emitProgress = (batch: JsonlEntry[]): void => {
        if (all.length <= INITIAL_BATCH_SIZE) return // single batch — no bar
        sentBytes = Math.min(total, sentBytes + batchSpan(batch))
        this.sendStatus({ status: 'watching', filePath: this.filePath!, replayProgress: { sent: sentBytes, total } })
      }
      const emitRange = async ([s, e]: [number, number]): Promise<void> => {
        for (let i = s; i < e; ) {
          if (!live()) return
          const batch = all.slice(i, Math.min(e, i + INITIAL_BATCH_SIZE))
          // Honor backpressure (mirrors checkForNewContent / the #78 tail fix):
          // if the socket is over the cap, DON'T advance — wait for it to drain
          // and resend the same batch. Flooding regardless dropped frames + OOM'd
          // the renderer. Bounded by the session lifecycle: a truly dead socket
          // closes → stop() → live() false ends the loop.
          const delivered = this.sendEntries(batch, true)
          if (delivered === false) { await delay(REPLAY_BACKPRESSURE_RETRY_MS); continue }
          emitProgress(batch)
          i += INITIAL_BATCH_SIZE
          await delay(INITIAL_BATCH_DELAY_MS)
        }
      }
      /** Emit a range from its END backwards, newest batch first.
       *
       *  `emitRange` walks a segment from its start, so the CURRENT segment's
       *  own tail — the turns from a minute ago — was the last thing sent of
       *  it. Until then the chat's bottom showed the segment's oldest part, and
       *  on a session compacted days ago that reads as "the chat jumped back
       *  four days" (it recovers when the walk finishes, which is why it looked
       *  intermittent). On a congested socket the backpressure retry stretches
       *  that window badly.
       *
       *  Out-of-order delivery is safe by construction: every entry carries
       *  `_ord` (its position in the file) and the client sorts on it — the same
       *  property the newest→oldest segment order already relies on. */
      const emitRangeFromEnd = async ([s, e]: [number, number]): Promise<void> => {
        for (let end = e; end > s; end -= INITIAL_BATCH_SIZE) {
          if (!live()) return
          const start = Math.max(s, end - INITIAL_BATCH_SIZE)
          const batch = all.slice(start, end)
          const delivered = this.sendEntries(batch, true)
          if (delivered === false) { await delay(REPLAY_BACKPRESSURE_RETRY_MS); end += INITIAL_BATCH_SIZE; continue }
          emitProgress(batch)
          await delay(INITIAL_BATCH_DELAY_MS)
        }
      }
      // Tail can start as soon as offset is known — emit replay concurrently.
      this.startPolling()
      await emitRangeFromEnd(ranges[ranges.length - 1]!) // current dialog, newest turns first
      for (let s = ranges.length - 2; s >= 0; s--) {
        if (!live()) break
        await emitRange(ranges[s]!) // older segments, newest → oldest, in background
      }
      if (!live()) return

    } catch (err) {
      if (!live()) return
      this.sendStatus({
        status: 'error',
        error: `Failed to read initial content: ${String(err)}`,
      })
      this.startPolling()
      this.sendStatus({ status: 'watching', filePath: this.filePath!, replayComplete: true })
      return
    }

    // Signal that all historical entries have been sent — and hand over the
    // file's fingerprint. A client mirroring the transcript stores it beside
    // the records; on the next launch it offers that fingerprint back and the
    // server can tell "you hold a prefix of this same file, here is the rest"
    // from "this file was rewritten, drop what you have". Without it the mirror
    // has nothing to prove its copy is still valid, so it could never be
    // trusted — and would be pointless.
    const fp = await fingerprintTranscript(this.filePath!)
    this.sendStatus({
      status: 'watching',
      filePath: this.filePath!,
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
    if (this.tailTimer) { clearInterval(this.tailTimer); this.tailTimer = null }
    if (this.tailWatcher) { try { this.tailWatcher.close() } catch {}; this.tailWatcher = null }
    // Same as switchToFile: a pending backpressure retry would race this fresh
    // replay and re-fire stats hooks against re-read entries (#78).
    if (this.tailRetryTimer) { clearTimeout(this.tailRetryTimer); this.tailRetryTimer = null }
    // Субагенты ТОЖЕ с нуля: живой watcher — skip в scanSubagents (его реплей
    // не повторится), завершённый — retired через 5 мин. В обоих случаях
    // реаттачнутый клиент открывал агента на «No messages from this agent
    // yet» — история агента живёт только в этих файлах. Пересоздание
    // watcher'ов реплеит их целиком; клиент дедупит по uuid.
    for (const w of this.subagentWatchers.values()) w.stop()
    this.subagentWatchers.clear()
    void this.scanSubagents()
    void this.readInitialContent()
  }

  private startPolling(): void {
    // Clear previous timers/watchers if any (from switchToFile)
    if (this.tailTimer) { clearInterval(this.tailTimer); this.tailTimer = null }
    if (this.tailWatcher) { try { this.tailWatcher.close() } catch {}; this.tailWatcher = null }
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
          if (this.tailWatcher) { try { this.tailWatcher.close() } catch {}; this.tailWatcher = null }
          schedule()
          return
        }
        schedule()
      })
      // The FSWatcher can emit an async 'error' (inotify limits, file removed on
      // some FSes) AFTER creation; without this listener that throws → server
      // crash. Close it and lean on the polling fallback instead.
      this.tailWatcher.on('error', () => {
        if (this.tailWatcher) { try { this.tailWatcher.close() } catch {}; this.tailWatcher = null }
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
    this.tailTimer = setInterval(() => {
      if (this.stopped || this.reading) return
      this.checkForNewContent()
    }, this.tailWatcher ? WATCH_SAFETY_POLL_MS : TAIL_POLL_MS)
  }

  private async checkForNewContent(): Promise<void> {
    if (!this.filePath || this.stopped) return
    // Set BEFORE the first await — fs.watch debounce and the safety-net
    // interval both check this flag, and an async gap here let them race
    // into a double read of the same range.
    if (this.reading) return
    this.reading = true

    try {
      const stat = await fsp.stat(this.filePath)
      if (stat.size <= this.lastOffset) return

      const { entries, endPos } = await this.readEntriesFromOffset(this.filePath, this.lastOffset)

      if (entries.length === 0) {
        // Only a torn trailing line (no complete entries yet) — advance past the
        // clean prefix and wait for the rest.
        this.lastOffset = Math.max(this.lastOffset, endPos)
        return
      }

      // Try to deliver BEFORE advancing the offset. `sendEntries` returns false
      // when the WS dropped the frame (bufferedAmount past the 16MB cap, or a
      // slow/frozen client) — advancing then would strand these lines forever
      // (they're in the file, but the tail never re-reads a consumed range). On a
      // drop: keep the offset, don't fire per-entry side effects, and retry soon.
      const delivered = this.sendEntries(entries)
      if (delivered === false) {
        if (this.tailRetryTimer) clearTimeout(this.tailRetryTimer)
        this.tailRetryTimer = setTimeout(() => {
          this.tailRetryTimer = null
          if (!this.stopped && !this.reading) void this.checkForNewContent()
        }, TAIL_BACKPRESSURE_RETRY_MS)
        return
      }
      // Delivered — commit the offset and fire the per-entry hooks (stats +
      // user-entry) exactly once, now that the range won't be re-read.
      this.lastOffset = Math.max(this.lastOffset, endPos)
      let sawBoundary = false
      for (const parsed of entries) {
        this.onStatsEntry?.(parsed)
        if (parsed.type === 'user' && this.onUserEntry) this.onUserEntry(parsed)
        if (parsed.type === 'system' && (parsed as { subtype?: string }).subtype === 'compact_boundary') sawBoundary = true
      }
      // A live /compact appends a compact_boundary to the SAME file, so it rides
      // the tail — not switchToFile (a NEW file, which re-runs readInitialContent
      // and rebuilds the index anyway). The segment strip is driven by the
      // server's authoritative segmentIndex, built ONCE at replayComplete; without
      // this the strip kept showing the pre-compact "Current" segment (stale count,
      // no new segment) until a reload. Rebuild + resend it now.
      if (sawBoundary) void this.rebuildSegmentIndex()
    } catch {
      // File might be temporarily unavailable, ignore
    } finally {
      this.reading = false
    }
  }

  private async readEntriesFromOffset(
    filePath: string,
    offset: number,
  ): Promise<{ entries: JsonlEntry[]; endPos: number }> {
    // Buffered tail-read instead of fs.createReadStream + readline — the
    // streaming variant hangs silently on Linux when the libuv threadpool
    // is contended (DuckDB native + node-pty + crypto). Tail is bounded
    // by lastOffset, typically <100KB per poll, so one-shot read is fine.
    const stat = await fsp.stat(filePath)
    if (stat.size <= offset) return { entries: [], endPos: offset }
    const fd = await fsp.open(filePath, 'r')
    let chunkText: string
    try {
      const buf = Buffer.alloc(stat.size - offset)
      await fd.read(buf, 0, buf.length, offset)
      chunkText = buf.toString('utf8')
    } finally {
      await fd.close()
    }
    const entries: JsonlEntry[] = []
    const lines = chunkText.split('\n')
    // A non-empty final element means the chunk ended mid-line (torn write) —
    // exclude those bytes from endPos so the next poll re-reads the full line.
    const tail = lines[lines.length - 1]!
    let endPos = stat.size
    let parseable = lines
    if (tail.trim()) {
      try { JSON.parse(tail) }
      catch {
        endPos = stat.size - Buffer.byteLength(tail, 'utf8')
        parseable = lines.slice(0, -1)
      }
    }
    for (const line of parseable) {
      if (!line.trim()) continue
      if (line.includes(SKIP_LINE_MARKER)) continue
      try {
        // Parse only — the stats + user-entry hooks fire in checkForNewContent
        // AFTER a successful send, so a backpressure re-read can't double-count.
        entries.push(JSON.parse(line) as JsonlEntry)
      } catch { /* skip malformed */ }
    }
    return { entries, endPos }
  }

  /**
   * Streaming variant: emits entry batches while parsing, keeps memory bounded
   * on large initial files. onBatch is awaited between batches to give the
   * event loop (and the WS send queue) time to drain.
   * Returns the file position where reading stopped — the caller uses it as
   * the tail offset (a pre-read stat.size races with concurrent appends).
   */
  private async readEntriesStreaming(
    filePath: string,
    offset: number,
    onBatch: (batch: JsonlEntry[]) => Promise<void>,
    // When provided, stat accumulation stops the moment this replay is
    // superseded (a newer replay/switch bumped replayGen) — otherwise two
    // overlapping full parses both fold entries into the same counters.
    isLive?: () => boolean,
  ): Promise<number> {
    // Manual chunked read replaces createReadStream + readline (the
    // async-iterator variant hangs under libuv threadpool contention).
    // We still stream — large initial JSONL files (tens of MB on long
    // sessions) can't be fully buffered — but use repeated fd.read into
    // a fixed-size buffer and split lines ourselves, carrying the partial
    // tail across chunks. await onBatch(...) yields between batches.
    const CHUNK_SIZE = 1024 * 1024 // 1MB per read
    const fd = await fsp.open(filePath, 'r')
    try {
      let pos = offset
      let leftover = ''
      let batch: JsonlEntry[] = []
      const buf = Buffer.alloc(CHUNK_SIZE)
      // Byte offset of the line being parsed. This is the ONLY cursor that
      // survives a replay: `_ord` is a per-process counter and gets re-tagged
      // every time the file is read again, so a client storing it could never
      // ask "send me everything after what I already hold".
      let lineStart = offset
      while (true) {
        const { bytesRead } = await fd.read(buf, 0, CHUNK_SIZE, pos)
        if (bytesRead === 0) break
        pos += bytesRead
        const text = leftover + buf.toString('utf8', 0, bytesRead)
        const lines = text.split('\n')
        // Last element is a partial line if the chunk didn't end on \n.
        leftover = lines.pop() ?? ''
        for (const line of lines) {
          const thisLineStart = lineStart
          lineStart += Buffer.byteLength(line, 'utf8') + 1 // + the \n we split on
          if (!line.trim()) continue
          if (line.includes(SKIP_LINE_MARKER)) continue
          try {
            const parsed = JSON.parse(line) as JsonlEntry
            // `_pos` is where this record STARTS; `_posEnd` is the boundary just
            // past it. A client resumes from `_posEnd` — resuming from `_pos + 1`
            // lands one byte INSIDE the record and the server then streams a
            // truncated first line. Caught on a real 30 971-record transcript:
            // 10 972 records came back where 10 971 were due, the extra one being
            // that fragment. Counting records would not have shown it; comparing
            // offsets did.
            ;(parsed as JsonlEntry & { _pos?: number; _posEnd?: number })._pos = thisLineStart
            ;(parsed as JsonlEntry & { _pos?: number; _posEnd?: number })._posEnd = lineStart
            batch.push(parsed)
            if (!isLive || isLive()) this.onStatsEntry?.(parsed)
            if (parsed.type === 'user' && this.onUserEntry) {
              this.onUserEntry(parsed)
            }
            if (batch.length >= INITIAL_BATCH_SIZE) {
              await onBatch(batch)
              batch = []
            }
          } catch { /* skip malformed */ }
        }
      }
      // Trailing partial line — file may have been mid-write. Try to parse;
      // if it fails (truncated JSON), leave its bytes OUT of the returned
      // offset so the next tail poll re-reads the completed line.
      let consumedLeftover = true
      if (leftover.trim() && !leftover.includes(SKIP_LINE_MARKER)) {
        try {
          const parsed = JSON.parse(leftover) as JsonlEntry
          ;(parsed as JsonlEntry & { _pos?: number; _posEnd?: number })._pos = lineStart
          ;(parsed as JsonlEntry & { _pos?: number; _posEnd?: number })._posEnd = lineStart + Buffer.byteLength(leftover, 'utf8') + 1
          batch.push(parsed)
          if (!isLive || isLive()) this.onStatsEntry?.(parsed)
          if (parsed.type === 'user' && this.onUserEntry) {
            this.onUserEntry(parsed)
          }
        } catch { consumedLeftover = false /* truncated — tail will retry */ }
      }
      if (batch.length > 0) await onBatch(batch)
      return consumedLeftover ? pos : pos - Buffer.byteLength(leftover, 'utf8')
    } finally {
      await fd.close()
    }
  }
}

// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function delay(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms))
}

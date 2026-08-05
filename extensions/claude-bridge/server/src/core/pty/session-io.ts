// ============================================================================
// PTY Session I/O — input/output buffering, debounce, startup auto-responder
// ============================================================================

import type { WebSocket as WS } from 'ws'
import type { PtySession } from './types'
import { eventBus } from '../events/bus'
import { debugLog } from '../logging'

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const OUTPUT_DEBOUNCE_MS = 8
const OUTPUT_MAX_WAIT_MS = 32

// CLI prints retry log lines like:
//   "ConnectionRefused · https://api.anthropic.com/v1/messages?beta=true · retry in 529ms (attempt 1/10)"
// We capture every match into the api_errors table for the dashboard
// errors badge + chart. Pattern is intentionally permissive on the
// error-type slug (CLI varies it: ConnectionRefused, Timeout, 429, etc.).
const API_ERROR_RE = /([\w\d_-]+)\s*·\s*(https:\/\/api\.anthropic\.com\/[^\s·]+)\s*·\s*retry in (\d+)ms\s*\(attempt\s*(\d+)\/(\d+)\)/g
// Non-retryable fatal errors — CLI prints `API Error: <Type> fetching "<url>"`
// and aborts the turn without a retry-counter line. Without a separate match
// these never make it into api_errors and the dashboard shows 0.
const API_ERROR_FATAL_RE = /API Error:\s*(\w+(?:Error)?)\s+fetching\s+"(https:\/\/api\.anthropic\.com\/[^"]+)"/g

// Dedupe nearby matches — CLI sometimes prints the same retry line
// twice as it redraws the status. Key on (errorType+url+attempt) and
// suppress duplicates within 2 seconds.
const recentErrorKey = new Map<string, number>()
const ERROR_DEDUPE_MS = 2000
// Server-side scrollback snapshot — replayed to clients on
// reconnect/reattach. Bumped from 200/50KB → 5000/500KB to match VSCode's
// `terminal.integrated.persistentSessionScrollback` default. Memory cost
// per active session is bounded; idle sessions get reaped after 30 min
// regardless. The line cap is hit far more often than the byte cap on
// chat-style streams (assistant text rarely exceeds ~80 chars/line).
const OUTPUT_BUFFER_MAX_LINES = 5000
const OUTPUT_BUFFER_MAX_BYTES = 500_000
// Eviction hysteresis: only trim once the buffer is this far past a cap, so the
// O(n) splice runs once per ~slack of output instead of on every chunk.
const OUTPUT_BUFFER_SLACK_LINES = 512
const OUTPUT_BUFFER_SLACK_BYTES = 131_072

// Status-line detection (see attachOutputDebounce). Module-level so they're
// compiled once, not per chunk. SPIN_SCAN (global) locates spinner glyphs in the
// RAW chunk so we ANSI-strip only a small window around the footer status line
// instead of the whole (~5KB full-repaint) chunk on every frame.
// Anchor scan uses only the DISTINCTIVE status glyphs (Braille spinner frames +
// ✳/✻) — NOT '·'/'*', which recur as content/separator chars (the status detail
// itself is "(58s · ↓ …)") and would make the localized scan degenerate to
// dozens of false windows per frame. The CLI status line always leads with one
// of these distinctive glyphs; STATUS_RE below still allows '·'/'*' in the match.
const SPIN_SCAN = /[✳✻⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏]/g
const ANSI_STRIP_CSI = /\x1b\[[0-9;?]*[A-Za-z]/g
const ANSI_STRIP_OSC = /\x1b\][^\x07\x1b]*(?:\x07|\x1b\\)/g
const STATUS_RE = /[·*✳✻⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏]\s*([A-Za-zА-Яа-я][A-Za-zА-Яа-я ]{2,30}[…])\s*\(([^)]{2,80})\)/
const OSC_TITLE_RE = /\x1b\](?:0|2);([^\x07\x1b]+)(?:\x07|\x1b\\)/
const STATUS_WINDOW = 400 // bytes stripped around a spinner glyph (status ≤ ~120 rendered)

// Throttle session:updated emits for PTY input (max 1 per 2 sec per session)
const lastInputEmit = new Map<string, number>()
const INPUT_EMIT_INTERVAL = 2000

/** Drop a session's input-throttle bookkeeping. Called from cleanupSession so
 *  this module Map doesn't grow unbounded over the whole process lifetime
 *  (one stale entry per session ever created). */
export function clearInputThrottle(sessionId: string): void {
  lastInputEmit.delete(sessionId)
}

// ---------------------------------------------------------------------------
// Send helper (shared)
// ---------------------------------------------------------------------------

// Hard cap on a single connection's outbound buffer. A stalled or slow client
// (dead socket not yet detected, or a genuinely slow link) would otherwise let
// the streaming/JSONL firehose grow ws.bufferedAmount without bound → a server
// OOM that kills EVERY session on the box. Past the cap we DROP the frame:
// streaming self-heals (boundary snapshots + the JSONL watcher re-deliver), and
// dropping one connection's frames is far better than crashing the process.
const WS_SEND_MAX_BUFFER = 16 * 1024 * 1024 // 16 MB

export function sendToClient(ws: WS, msg: Record<string, unknown>): boolean {
  if (ws.readyState !== 1 /* WS.OPEN */) return false
  if ((ws.bufferedAmount ?? 0) > WS_SEND_MAX_BUFFER) return false
  // Guard the widest chokepoint: JSON.stringify throws on a circular/BigInt value
  // (a malformed synthesized entry) and ws.send can throw on a racing socket
  // state. Many callers run inside pty.onData / setTimeout / onExit callbacks
  // where a throw would escape to uncaughtException — dropping this one frame is
  // always better than that.
  try {
    ws.send(JSON.stringify(msg))
    return true
  } catch {
    return false
  }
}

// ---------------------------------------------------------------------------
// Input
// ---------------------------------------------------------------------------

const INPUT_CLEAR_SETTLE_MS = 50
const INPUT_ECHO_QUIET_MS = 80
const INPUT_POST_ENTER_SETTLE_MS = 80
const INPUT_HARD_MAX_MS = 2000
const MAINTENANCE_INPUT_QUIET_MS = 300

interface QueuedSubmission {
  kind: 'user' | 'maintenance'
  text: string
  maintenanceKey?: string
  maintenanceRevision?: number
}

interface ActiveSubmission extends QueuedSubmission {
  clearTimer: ReturnType<typeof setTimeout> | null
  quietTimer: ReturnType<typeof setTimeout> | null
  hardTimer: ReturnType<typeof setTimeout> | null
  postEnterTimer: ReturnType<typeof setTimeout> | null
  outputSubscription: { dispose(): void } | null
  fired: boolean
}

interface PendingMaintenance {
  text: string
  revision: number
}

interface SessionInputState {
  promptReady: boolean
  rawInputDirty: boolean
  rawPasteMode: boolean
  lastRawInputAt: number
  queue: QueuedSubmission[]
  active: ActiveSubmission | null
  bufferedRawInput: string[]
  pendingMaintenance: Map<string, PendingMaintenance>
  maintenanceRevision: number
  maintenanceTimer: ReturnType<typeof setTimeout> | null
}

const sessionInputStates = new WeakMap<PtySession, SessionInputState>()

function inputState(session: PtySession): SessionInputState {
  let state = sessionInputStates.get(session)
  if (!state) {
    state = {
      promptReady: false,
      rawInputDirty: false,
      rawPasteMode: false,
      lastRawInputAt: 0,
      queue: [],
      active: null,
      bufferedRawInput: [],
      pendingMaintenance: new Map(),
      maintenanceRevision: 0,
      maintenanceTimer: null,
    }
    sessionInputStates.set(session, state)
  }
  return state
}

function cancelMaintenanceTimer(state: SessionInputState): void {
  if (!state.maintenanceTimer) return
  clearTimeout(state.maintenanceTimer)
  state.maintenanceTimer = null
}

function canRunMaintenance(session: PtySession, state: SessionInputState): boolean {
  return session.state === 'running'
    && !session.detachedAt
    && state.promptReady
    && !state.rawInputDirty
    && !state.active
    && state.queue.length === 0
    && state.pendingMaintenance.size > 0
}

function schedulePendingMaintenance(session: PtySession, state = inputState(session)): void {
  if (!canRunMaintenance(session, state)) {
    cancelMaintenanceTimer(state)
    return
  }
  if (state.maintenanceTimer) return

  // A raw Ctrl+U is immediately followed by session:submitText in older
  // clients. Keeping a short reservation window here prevents maintenance
  // from occupying the freshly-cleared prompt between those two WS frames.
  const delay = Math.max(0, state.lastRawInputAt + MAINTENANCE_INPUT_QUIET_MS - Date.now())
  state.maintenanceTimer = setTimeout(() => {
    state.maintenanceTimer = null
    if (!canRunMaintenance(session, state)) return
    const first = state.pendingMaintenance.entries().next().value as [string, PendingMaintenance] | undefined
    if (!first) return
    const [maintenanceKey, pending] = first
    startSubmission(session, state, {
      kind: 'maintenance',
      text: pending.text,
      maintenanceKey,
      maintenanceRevision: pending.revision,
    })
  }, delay)
}

function updateRawInputState(state: SessionInputState, data: string): void {
  const pasteStart = '\x1b[200~'
  const pasteEnd = '\x1b[201~'
  for (let i = 0; i < data.length;) {
    if (data.startsWith(pasteStart, i)) {
      state.rawPasteMode = true
      i += pasteStart.length
      continue
    }
    if (data.startsWith(pasteEnd, i)) {
      state.rawPasteMode = false
      i += pasteEnd.length
      continue
    }

    const ch = data[i]!
    i++
    if (state.rawPasteMode) {
      // Newlines inside bracketed paste are content, not a submitted line.
      state.rawInputDirty = true
      continue
    }
    if (ch === '\x15' || ch === '\x03') {
      // Ctrl+U clears Ink's line; Ctrl+C cancels the current line/turn.
      state.rawInputDirty = false
      continue
    }
    if (ch === '\r' || ch === '\n') {
      state.rawInputDirty = false
      state.promptReady = false
      continue
    }
    // Cursor/navigation/control sequences do not prove that the line contains
    // text. Backspace is deliberately conservative: we cannot know whether it
    // removed the final grapheme, so maintenance waits for Ctrl+U/Enter.
    if (ch >= ' ' && ch !== '\x7f') state.rawInputDirty = true
  }
}

function writeRawNow(session: PtySession, state: SessionInputState, data: string): void {
  if (session.state !== 'running') return
  cancelMaintenanceTimer(state)
  session.pty.write(data)
  state.lastRawInputAt = Date.now()
  updateRawInputState(state, data)
  schedulePendingMaintenance(session, state)
}

function disposeActiveTimers(active: ActiveSubmission): void {
  if (active.clearTimer) clearTimeout(active.clearTimer)
  if (active.quietTimer) clearTimeout(active.quietTimer)
  if (active.hardTimer) clearTimeout(active.hardTimer)
  if (active.postEnterTimer) clearTimeout(active.postEnterTimer)
  try { active.outputSubscription?.dispose() } catch { /* noop */ }
  active.clearTimer = null
  active.quietTimer = null
  active.hardTimer = null
  active.postEnterTimer = null
  active.outputSubscription = null
}

function finishSubmission(
  session: PtySession,
  state: SessionInputState,
  active: ActiveSubmission,
  submitted: boolean,
): void {
  if (state.active !== active) return
  disposeActiveTimers(active)
  state.active = null

  if (submitted && active.kind === 'maintenance' && active.maintenanceKey) {
    const pending = state.pendingMaintenance.get(active.maintenanceKey)
    // A newer sync may have arrived while the old reload was being typed. Only
    // acknowledge the exact revision we actually submitted.
    if (pending?.revision === active.maintenanceRevision) {
      state.pendingMaintenance.delete(active.maintenanceKey)
    }
  }

  const buffered = state.bufferedRawInput.splice(0)
  for (const data of buffered) writeRawNow(session, state, data)

  const next = state.queue.shift()
  if (next && session.state === 'running') startSubmission(session, state, next)
  else schedulePendingMaintenance(session, state)
}

function startSubmission(session: PtySession, state: SessionInputState, queued: QueuedSubmission): void {
  if (session.state !== 'running') return
  cancelMaintenanceTimer(state)

  const active: ActiveSubmission = {
    ...queued,
    clearTimer: null,
    quietTimer: null,
    hardTimer: null,
    postEnterTimer: null,
    outputSubscription: null,
    fired: false,
  }
  state.active = active
  state.rawInputDirty = false

  // Clear + paste + Enter is one coordinator-owned transaction. No other raw
  // or programmatic input can interleave between these writes.
  try {
    session.pty.write('\x15')
  } catch {
    finishSubmission(session, state, active, false)
    return
  }

  active.clearTimer = setTimeout(() => {
    active.clearTimer = null
    if (state.active !== active || session.state !== 'running') {
      finishSubmission(session, state, active, false)
      return
    }

    const body = queued.text.replace(/\r\n?/g, '\n')
    const fireEnter = () => {
      if (active.fired || state.active !== active) return
      active.fired = true
      disposeActiveTimers(active)
      let submitted = false
      if (session.state === 'running') {
        try {
          session.pty.write('\r')
          state.promptReady = false
          submitted = true
        } catch { /* PTY exited between the state check and write */ }
      }
      if (!submitted) finishSubmission(session, state, active, false)
      else {
        // Keep the transaction reserved briefly after Enter. Starting the next
        // Ctrl+U in the same JS tick can clear the command before Ink consumes
        // Enter, which recreates the exact concatenation/lost-submit race this
        // coordinator is meant to remove.
        active.postEnterTimer = setTimeout(
          () => finishSubmission(session, state, active, true),
          INPUT_POST_ENTER_SETTLE_MS,
        )
      }
    }

    active.outputSubscription = session.pty.onData(() => {
      if (active.fired || state.active !== active) return
      if (active.quietTimer) clearTimeout(active.quietTimer)
      active.quietTimer = setTimeout(fireEnter, INPUT_ECHO_QUIET_MS)
    })
    active.hardTimer = setTimeout(fireEnter, INPUT_HARD_MAX_MS)
    try {
      session.pty.write(`\x1b[200~${body}\x1b[201~`)
    } catch {
      finishSubmission(session, state, active, false)
    }
  }, INPUT_CLEAR_SETTLE_MS)
}

function cancelActiveSubmission(state: SessionInputState): void {
  const active = state.active
  if (active) disposeActiveTimers(active)
  state.active = null
  state.queue = []
  state.bufferedRawInput = []
}

/** Update the server-authoritative prompt state from deterministic CLI hooks. */
export function setSessionPromptReady(session: PtySession, promptReady: boolean): void {
  const state = inputState(session)
  state.promptReady = promptReady
  schedulePendingMaintenance(session, state)
}

/** Re-evaluate deferred maintenance after detach or reattach. */
export function notifySessionAttachmentChanged(session: PtySession): void {
  schedulePendingMaintenance(session)
}

/** Coalesce a maintenance command by key and run it only at a safe prompt. */
export function requestMaintenanceSubmission(session: PtySession, key: string, text: string): void {
  const state = inputState(session)
  state.maintenanceRevision++
  state.pendingMaintenance.set(key, { text, revision: state.maintenanceRevision })
  schedulePendingMaintenance(session, state)
}

/** Read-only diagnostics used by regression tests and debug tooling. */
export function getSessionInputSnapshot(session: PtySession): {
  promptReady: boolean
  rawInputDirty: boolean
  activeKind: 'user' | 'maintenance' | null
  queuedSubmissions: number
  pendingMaintenance: string[]
} {
  const state = inputState(session)
  return {
    promptReady: state.promptReady,
    rawInputDirty: state.rawInputDirty,
    activeKind: state.active?.kind ?? null,
    queuedSubmissions: state.queue.length,
    pendingMaintenance: [...state.pendingMaintenance.keys()],
  }
}

/** Drop all coordinator timers/listeners for a destroyed session. */
export function clearSessionInputState(session: PtySession): void {
  const state = sessionInputStates.get(session)
  if (!state) return
  cancelMaintenanceTimer(state)
  if (state.active) disposeActiveTimers(state.active)
  sessionInputStates.delete(session)
}

/**
 * Write PTY stdin data.
 */
export function writeInputToSession(session: PtySession, data: string): void {
  if (session.state !== 'running') return
  session.lastActivityAt = new Date()
  const state = inputState(session)
  if (data.includes('\x03')) {
    // Interrupt must bypass an in-flight delayed Enter and cancel queued user
    // submissions; otherwise Stop can be followed by a surprise send.
    cancelActiveSubmission(state)
    writeRawNow(session, state, data)
  } else if (state.active) {
    state.bufferedRawInput.push(data)
  } else {
    writeRawNow(session, state, data)
  }

  // Ctrl+C (\x03) is the user's interrupt (Stop button / Esc → sendInput). The
  // CLI aborts the turn but fires NO Stop hook (it didn't "finish"), so the
  // deterministic hook-driven working state — now authoritative over the OSC
  // idle fallback on the client — would stay stuck "working" (spinner + Stop
  // button never clear). Push an authoritative idle so the chat activity
  // indicator AND the native session-row dot both clear immediately on interrupt.
  if (data.includes('\x03')) {
    setSessionPromptReady(session, true)
    sendToClient(session.ws, {
      type: 'session:activity',
      sessionId: session.id,
      rawTitle: '',
      isWorking: false,
      hookDriven: true,
      promptReady: true,
      waiting: false,
    })
  }

  // Throttled emit to update dashboard idle timer
  const now = Date.now()
  const last = lastInputEmit.get(session.id) || 0
  if (now - last > INPUT_EMIT_INTERVAL) {
    lastInputEmit.set(session.id, now)
    eventBus.emit('session:updated', {
      sessionId: session.id,
      mcpCallCount: session.mcpCallCount,
      inputCount: session.inputCount,
      mcpInitialized: session.mcpInitialized,
      mcpLastError: session.mcpLastError,
      lastActivityAt: session.lastActivityAt.toISOString(),
    })
  }
}

/**
 * Submit a full user message deterministically using a quiet-window gate:
 *   1. Send the bracketed-paste envelope with the message body (with any
 *      bare CR normalized to LF so it never commits mid-paste).
 *   2. Subscribe to PTY output. Every chunk pushes a 80ms "quiet" timer
 *      forward. The first 80ms of silence after the paste write means the
 *      CLI has finished rendering the paste echo / input-buffer redraw →
 *      send exactly one CR.
 *   3. Hard ceiling 2000ms: if the session keeps streaming (e.g. assistant
 *      response still coming in) we still fire a CR so we never hang
 *      forever. User Enter lands on the next prompt in that pathological
 *      case, same as before.
 */
export function submitTextToSession(session: PtySession, text: string): void {
  if (session.state !== 'running') return
  session.lastActivityAt = new Date()
  const state = inputState(session)
  cancelMaintenanceTimer(state)
  const queued: QueuedSubmission = { kind: 'user', text }
  if (state.active) state.queue.push(queued)
  else startSubmission(session, state, queued)
}

/**
 * Resize PTY terminal.
 */
export function resizeSessionTerminal(session: PtySession, cols: number, rows: number): void {
  if (session.state !== 'running') return
  session.pty.resize(Math.max(1, cols), Math.max(1, rows))
  // A real client resize = a console is visibly attached (the idle-console
  // shrink uses this heartbeat). Record it and undo any prior shrink; the CLI
  // full-repaints at the restored size on the SIGWINCH this resize just fired.
  session.lastResizeAt = Date.now()
  session.consoleShrunk = false
}

// ---------------------------------------------------------------------------
// Startup auto-responder
// ---------------------------------------------------------------------------

/**
 * Auto-respond to interactive startup prompts.
 * Watches PTY output for known prompts and sends Enter/selection.
 * Self-disables after startup is complete (main prompt appears or timeout).
 */
export function attachStartupAutoResponder(session: PtySession): void {
  let accumulated = ''
  let disposed = false
  let respondCount = 0

  // Auto-disable after 30 seconds (startup should be done by then)
  const timeout = setTimeout(() => { disposed = true }, 30_000)

  const disposable = session.pty.onData((data: string) => {
    if (disposed) return
    accumulated += data

    // "Enter to confirm" → trust folder prompt, send Enter
    if (accumulated.includes('Enter to confirm')) {
      respondCount++
      debugLog('Auto-responding to trust prompt', { sessionId: session.id })
      setTimeout(() => {
        if (session.state === 'running') {
          writeInputToSession(session, '\r')
        }
      }, 200)
      accumulated = ''
    }

    // Effort level prompt — "Use high effort" — send Enter (option 1 selected)
    if (accumulated.includes('Use high effort') || accumulated.includes('Use medium effort')) {
      respondCount++
      debugLog('Auto-responding to effort prompt', { sessionId: session.id })
      setTimeout(() => {
        if (session.state === 'running') {
          writeInputToSession(session, '\r')
        }
      }, 200)
      accumulated = ''
    }

    // Main prompt appeared (> or ❯) — startup is done, stop watching
    // Also stop after 3 auto-responses to prevent infinite loops
    if (respondCount >= 3 || accumulated.includes('\n> ') || accumulated.includes('\n❯ ')) {
      disposed = true
      clearTimeout(timeout)
      disposable.dispose()
    }

    // Keep buffer reasonable
    if (accumulated.length > 4096) {
      accumulated = accumulated.slice(-2048)
    }
  })
}

// ---------------------------------------------------------------------------
// Output debounce + OSC title / activity parsing
// ---------------------------------------------------------------------------

export function attachOutputDebounce(session: PtySession): void {
  let outputBuffer = ''
  let trailTimer: ReturnType<typeof setTimeout> | null = null
  let maxTimer: ReturnType<typeof setTimeout> | null = null

  function flush() {
    if (trailTimer) { clearTimeout(trailTimer); trailTimer = null }
    if (maxTimer) { clearTimeout(maxTimer); maxTimer = null }
    if (outputBuffer && session.ws.readyState === 1 /* WS.OPEN */) {
      sendToClient(session.ws, { type: 'session:output', data: outputBuffer })
      outputBuffer = ''
    }
  }

  session.pty.onData((data: string) => {
    outputBuffer += data

    // Capture Anthropic API retry errors from the output stream — they
    // surface in CLI's terminal output well before any JSONL entry, so
    // intercepting here gives near-realtime error counts. We strip ANSI
    // and run the regex only when the chunk actually mentions
    // "anthropic" — typical chat streams skip the work entirely.
    if (data.includes('anthropic.com')) {
      const plain = data.replace(/\x1b\[[0-9;?]*[A-Za-z]/g, '').replace(/\x1b\][^\x07\x1b]*(?:\x07|\x1b\\)/g, '')

      const recordError = (
        errorType: string, url: string, retryMs: number,
        attempt: number, maxAttempts: number, raw: string,
      ): void => {
        const dedupeKey = `${errorType}|${url}|${attempt}|${session.id}`
        const now = Date.now()
        const last = recentErrorKey.get(dedupeKey) ?? 0
        if (now - last < ERROR_DEDUPE_MS) return
        recentErrorKey.set(dedupeKey, now)
        if (recentErrorKey.size > 1000) {
          const oldest = recentErrorKey.keys().next().value
          if (oldest) recentErrorKey.delete(oldest)
        }
        const ts = new Date().toISOString()
        const userName = session.userName
        const sessionId = session.cliConversationId
        void (async () => {
          try {
            const db = await (await import('../stats/database/lifecycle')).getDb()
            await db.run(
              `INSERT INTO api_errors (timestamp, session_id, user_name, error_type, url, attempt, max_attempts, retry_ms, raw)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)`,
              [ts, sessionId, userName, errorType, url, attempt, maxAttempts, retryMs, raw],
            )
          } catch { /* best-effort */ }
        })()
      }

      let m: RegExpExecArray | null
      const re = new RegExp(API_ERROR_RE.source, 'g')
      while ((m = re.exec(plain)) !== null) {
        recordError(
          m[1] ?? 'Unknown', m[2] ?? '',
          Number(m[3] ?? 0), Number(m[4] ?? 0), Number(m[5] ?? 0),
          m[0] ?? '',
        )
      }
      // Fatal (non-retryable) errors — single occurrence, no retry counter.
      const reFatal = new RegExp(API_ERROR_FATAL_RE.source, 'g')
      while ((m = reFatal.exec(plain)) !== null) {
        recordError(m[1] ?? 'Unknown', m[2] ?? '', 0, 0, 0, m[0] ?? '')
      }
    }

    // Fast path: chunks with no ESC byte (most token-by-token streams) skip
    // the strip+regex passes entirely. Even the cheapest regex sets up
    // capture state per chunk, and CLI emits 50+ chunks/s during a stream.
    const hasEsc = data.indexOf('\x1b') !== -1
    let statusText: string | undefined
    let oscMatch: RegExpMatchArray | null = null

    if (hasEsc) {
      // Localized status scan. The CLI status line ("* Shenaniganing… (58s · ↓
      // 219 tokens)") lives in the terminal body (not the OSC title) and carries
      // a spinner glyph. Instead of ANSI-stripping the whole ~5KB chunk on every
      // frame, find spinner glyphs in the RAW chunk (a cheap char scan) and
      // strip+match only a small window around each — from the LAST (footer)
      // backward, since the status is the last/only spinner. Same STATUS_RE, same
      // result, a fraction of the strip work.
      const spinIdx: number[] = []
      let sp: RegExpExecArray | null
      SPIN_SCAN.lastIndex = 0
      while ((sp = SPIN_SCAN.exec(data)) !== null) spinIdx.push(sp.index)
      for (let k = spinIdx.length - 1; k >= 0; k--) {
        const plain = data.slice(spinIdx[k]!, spinIdx[k]! + STATUS_WINDOW)
          .replace(ANSI_STRIP_CSI, '').replace(ANSI_STRIP_OSC, '')
        const statusMatch = plain.match(STATUS_RE)
        if (statusMatch) {
          statusText = `${statusMatch[1]!.trim()} (${statusMatch[2]!.trim()})`
          break
        }
      }
      if (statusText && statusText !== session.lastStatusText) {
        session.lastStatusText = statusText
        // Emit a standalone activity event so the UI stays in sync even when the
        // OSC title hasn't changed (CLI updates OSC less often than the status
        // line, so without this the spinner text freezes on old data).
        sendToClient(session.ws, {
          type: 'session:activity',
          sessionId: session.id,
          rawTitle: statusText,
          isWorking: true,
        })
      }

      // OSC title — only run the regex when the chunk actually contains an
      // OSC introducer byte. Skips the whole branch on plain text streams.
      if (data.indexOf('\x1b]') !== -1) {
        oscMatch = data.match(OSC_TITLE_RE)
      }
    }
    if (oscMatch) {
      const rawOscTitle = (oscMatch[1] ?? '').trim()

      // Send activity status to the client on every OSC (spinner detection)
      // Spinner chars: Braille dots (⠂⠒⠴⠦ etc.), ✳ = idle, › = prompt ready
      const isWorking = !!rawOscTitle && !rawOscTitle.startsWith('✳') && !rawOscTitle.startsWith('›')
      // Prefer the live CLI status line ("Shenaniganing… (58s · ↓ 219 tokens)")
      // when present — it's what the user sees in-terminal. Fall back to OSC title.
      const reportedTitle = isWorking ? (statusText ?? session.lastStatusText ?? rawOscTitle) : rawOscTitle
      sendToClient(session.ws, {
        type: 'session:activity',
        sessionId: session.id,
        rawTitle: reportedTitle,
        isWorking,
      })

      let title = rawOscTitle
      // Filter out default/status titles: anything that is just "Claude Code" with
      // non-alphanumeric decorations (spinners, icons, Braille dots, etc.)
      // Real topic titles have alphanumeric text BESIDES "Claude Code".
      const strippedOfClaudeCode = title.replace(/Claude Code/g, '').replace(/[^a-zA-Z0-9]/g, '')
      const isDefaultTitle = title.includes('Claude Code') && strippedOfClaudeCode.length === 0
      // Strip leading icon/spinner prefix (✳, ⠂, ⠐, etc.) from CLI title
      title = title.replace(/^[^\w\d]+\s*/, '').trim()
      if (title && !isDefaultTitle && title !== session.sessionTitle) {
        session.sessionTitle = title
        debugLog('Session title detected', { sessionId: session.id, title })
        // Persist to session_tokens so the dashboard's Active PTY
        // Sessions table can show a human-readable label even before
        // CLI writes ai-title/custom-title to the JSONL. DuckDB is async —
        // fire-and-forget IIFE so the OSC parse path stays sync.
        if (session.cliConversationId) {
          const convId = session.cliConversationId
          const titleSnapshot = title
          void (async () => {
            try {
              const { getDb } = await import('../stats/database/lifecycle')
              const db = await getDb()
              await db.run(`UPDATE session_tokens SET title = ? WHERE session_id = ?`, [titleSnapshot, convId])
            } catch { /* best-effort */ }
          })()
        }
        // Notify the client of the title change.
        sendToClient(session.ws, { type: 'session:title', sessionId: session.id, title })
        // Emit event for dashboard / tree update
        eventBus.emit('session:updated', {
          sessionId: session.id,
          sessionTitle: title,
          lastActivityAt: session.lastActivityAt.toISOString(),
        })
      }
    }

    // Also push to session outputBuffer for snapshot on user input. Byte total
    // is tracked incrementally — the old code re-summed the whole ≤5000-line
    // buffer on EVERY chunk, and the CLI TUI repaints many chunks/sec, so at N
    // sessions that alone burned a core (25× worse since the 200→5000 bump).
    const lines = data.split('\n')
    // Push line-by-line — NEVER `push(...lines)`: spreading a big chunk's lines
    // (a `cat` of a large file, a base64 blob) into call args throws
    // `RangeError: Maximum call stack size exceeded` past ~100k args, which would
    // escape the pty.onData callback to uncaughtException.
    for (const line of lines) { session.outputBuffer.push(line); session.outputBufferBytes += line.length }
    // Evict oldest only past cap+slack → the O(n) splice runs once per ~slack of
    // output, not every chunk (active sessions sit AT the cap). One pass trims
    // back to both caps, updating the running byte counter as it drops lines.
    if (
      session.outputBuffer.length > OUTPUT_BUFFER_MAX_LINES + OUTPUT_BUFFER_SLACK_LINES ||
      session.outputBufferBytes > OUTPUT_BUFFER_MAX_BYTES + OUTPUT_BUFFER_SLACK_BYTES
    ) {
      let drop = 0
      const n = session.outputBuffer.length
      while (
        drop < n - 1 &&
        (n - drop > OUTPUT_BUFFER_MAX_LINES || session.outputBufferBytes > OUTPUT_BUFFER_MAX_BYTES)
      ) {
        session.outputBufferBytes -= session.outputBuffer[drop]!.length
        drop++
      }
      if (drop > 0) session.outputBuffer.splice(0, drop)
    }

    if (trailTimer) clearTimeout(trailTimer)
    trailTimer = setTimeout(flush, OUTPUT_DEBOUNCE_MS)
    if (!maxTimer) {
      maxTimer = setTimeout(flush, OUTPUT_MAX_WAIT_MS)
    }
  })
}

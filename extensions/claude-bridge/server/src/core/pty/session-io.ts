// ============================================================================
// PTY Session I/O — input/output buffering, debounce, startup auto-responder
// ============================================================================

import type { WebSocket as WS } from 'ws'
import type { PtySession } from './types'
import { eventBus } from '../events/bus'
import { debugLog } from '../logging'
import {
  setSessionPromptReady,
  submitCoordinatedText,
  writeCoordinatedInput,
} from './session-input-coordinator'

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

/**
 * Write PTY stdin data.
 */
export function writeInputToSession(session: PtySession, data: string): void {
  if (session.state !== 'running') return
  session.lastActivityAt = new Date()
  writeCoordinatedInput(session, data)

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
 * Submit a full user message through the per-session input coordinator:
 *   1. Clear the current line, then send the bracketed-paste envelope (with any
 *      bare CR normalized to LF so it never commits mid-paste).
 *   2. Subscribe to PTY output. Every chunk pushes a 80ms "quiet" timer
 *      forward. The first 80ms of silence after the paste write means the
 *      CLI has finished rendering the paste echo / input-buffer redraw →
 *      send exactly one CR.
 *   3. A hard ceiling of 2000ms guarantees an eventual Enter if no echo arrives.
 *      Raw input and additional submissions stay serialized around this unit.
 */
export function submitTextToSession(session: PtySession, text: string): void {
  if (session.state !== 'running') return
  session.lastActivityAt = new Date()
  submitCoordinatedText(session, text)
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

      // Send activity status to Electron on every OSC (spinner detection)
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
        // Notify Electron client of title change
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

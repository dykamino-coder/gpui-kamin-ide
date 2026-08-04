// ============================================================================
// Idle-console PTY shrink — OPT-IN via BRIDGE_IDLE_CONSOLE_SHRINK=1.
//
// A visible KaminIDE console heartbeats a `session:resize` every ~5s (the TUI-
// corruption SIGWINCH). A hidden console (inactive tab / closed console tool)
// sends nothing. So "no client resize for > SHRINK_AFTER_MS" means nobody is
// watching this session's console right now. We then shrink its PTY to a tiny
// grid: the Ink TUI's per-frame repaint — and thus the byte-diff volume our
// server output hot-path stringifies/regexes on every chunk — scales with the
// viewport area. Live frame-timing A/B (claude 2.1.202, animated turn) measured
// 40x12 at roughly HALF the per-frame terminal patches of 120x40.
//
// Purely a display-size change: the chat, session status (spinner line + OSC
// title, both single-line), streaming (MITM/API), resume and JSONL are all
// independent of PTY cols/rows. When the console is shown again the client's
// fit() sends the real size via resizeSessionTerminal(), which restores the
// viewport and fires a full repaint. Off by default so it only takes effect on
// deployments where the KaminIDE 5s heartbeat is known to be present.
// ============================================================================

import { sessions } from './session-core'
import { debugLog } from '../logging'

export const CONSOLE_SHRINK_COLS = 40
export const CONSOLE_SHRINK_ROWS = 12
export const CONSOLE_SHRINK_AFTER_MS = 12_000  // > 2× the client's 5s heartbeat
export const CONSOLE_SHRINK_SWEEP_MS = 15_000

export function isConsoleShrinkEnabled(): boolean {
  const v = process.env.BRIDGE_IDLE_CONSOLE_SHRINK
  return v === '1' || v === 'true'
}

/** Minimal shape decideConsoleShrink needs — lets unit tests drive every
 *  branch without a real PTY. */
export interface ShrinkableSession {
  state: string
  consoleShrunk: boolean
  lastResizeAt: number
  cols: number
}

/** Pure decision: should this session's PTY be shrunk now? */
export function decideConsoleShrink(s: ShrinkableSession, now: number): boolean {
  if (s.state !== 'running') return false
  if (s.consoleShrunk) return false              // already small — don't churn
  if (s.cols <= CONSOLE_SHRINK_COLS) return false // client is already tiny
  return now - s.lastResizeAt > CONSOLE_SHRINK_AFTER_MS
}

let timer: ReturnType<typeof setInterval> | null = null

/** Start the shared shrink sweep (one timer for all sessions). No-op unless
 *  BRIDGE_IDLE_CONSOLE_SHRINK is set. */
export function startConsoleShrinkSweep(): void {
  if (timer || !isConsoleShrinkEnabled()) return
  timer = setInterval(sweep, CONSOLE_SHRINK_SWEEP_MS)
  timer.unref?.()
  debugLog('Idle-console PTY shrink enabled', {
    afterMs: CONSOLE_SHRINK_AFTER_MS, size: `${CONSOLE_SHRINK_COLS}x${CONSOLE_SHRINK_ROWS}`,
  })
}

export function stopConsoleShrinkSweep(): void {
  if (timer) { clearInterval(timer); timer = null }
}

function sweep(): void {
  const now = Date.now()
  for (const session of sessions.values()) {
    // Whole per-session body guarded: reading `session.pty.cols` on an exited
    // pty can throw, and this runs on a bare setInterval — an escaping throw
    // would reach the process handler. Isolate so one bad session can't abort
    // the sweep or crash the server.
    try {
      const cols = session.pty.cols
      if (!decideConsoleShrink({ state: session.state, consoleShrunk: session.consoleShrunk, lastResizeAt: session.lastResizeAt, cols }, now)) continue
      // NB: resize the pty DIRECTLY (not resizeSessionTerminal) so we don't stamp
      // lastResizeAt — that field must reflect real CLIENT resizes only.
      session.pty.resize(CONSOLE_SHRINK_COLS, CONSOLE_SHRINK_ROWS)
      session.consoleShrunk = true
    } catch { /* pty exited / cols getter threw — skip this session */ }
  }
}

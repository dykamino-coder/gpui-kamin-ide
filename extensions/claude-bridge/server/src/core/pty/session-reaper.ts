// ============================================================================
// PTY Session Reaper — cleans up idle/dead sessions
// ============================================================================

import { eventBus } from '../events/bus'
import { debugLog, warnLog } from '../logging'
import { sessions, destroySession } from './session-core'
import { hasInflightMcpCall } from './session-mcp-call'

// ---------------------------------------------------------------------------
// Constants (exported so unit tests can assert against them without
// duplicating magic numbers).
// ---------------------------------------------------------------------------

export const REAPER_INTERVAL_MS = 60_000          // check every 60s
export const STARTUP_TIMEOUT_MS = 60_000          // kill if stuck starting for 60s
export const DEFAULT_IDLE_TIMEOUT_MIN = 30        // 30 min default idle timeout
export const DEFAULT_MAX_LIFETIME_MIN = 1440      // 24h max lifetime
// Sub-agent idle window. Used to be 2 min, which silently killed sub-
// agents in the middle of long Bash/Grep tool calls — `lastActivityAt`
// only updates on MCP send/receive, so a single `npm install` inside a
// sub-agent looked like dead-air to the reaper. The inflight-MCP guard
// in `decideReap()` is the primary defence, this window is the upper
// bound when no MCP call is pending (waiting on the model itself).
export const SUB_AGENT_IDLE_TIMEOUT_MS = 10 * 60_000  // 10 min

// ---------------------------------------------------------------------------
// Reaper state
// ---------------------------------------------------------------------------

let reaperTimer: ReturnType<typeof setInterval> | null = null

/**
 * Start the session reaper (call once at server startup).
 * Checks every 60s for sessions that should be cleaned up.
 */
export function startSessionReaper(): void {
  if (reaperTimer) return
  reaperTimer = setInterval(reapSessions, REAPER_INTERVAL_MS)
  debugLog('Session reaper started', { intervalMs: REAPER_INTERVAL_MS })
}

/**
 * Stop the session reaper (call on shutdown).
 */
export function stopSessionReaper(): void {
  if (reaperTimer) {
    clearInterval(reaperTimer)
    reaperTimer = null
  }
}

/**
 * Pure decision function. Returns the reap reason (or `null` to keep)
 * for a single session given the current clock. Extracted so unit
 * tests can drive every branch without spinning up a real PTY.
 */
export type ReapReason = 'startup_timeout' | 'idle_timeout' | 'max_lifetime' | null

export interface ReapableSession {
  id: string
  state: string
  isSubAgent?: boolean
  createdAt: Date
  lastActivityAt: Date
}

export function decideReap(
  session: ReapableSession,
  now: number,
  hasInflightCall: (sessionId: string) => boolean = hasInflightMcpCall,
): ReapReason {
  const ageMs = now - session.createdAt.getTime()
  const idleMs = now - session.lastActivityAt.getTime()

  if (session.state === 'starting' && ageMs > STARTUP_TIMEOUT_MS) return 'startup_timeout'
  if (session.state === 'exited' || session.state === 'exiting') return null

  // max_lifetime is an absolute upper bound — even an inflight tool
  // can't keep a session alive past 24h. Anything else is preempted
  // by the inflight-MCP guard below.
  const maxLifetimeMs = DEFAULT_MAX_LIFETIME_MIN * 60_000
  if (ageMs > maxLifetimeMs) return 'max_lifetime'

  // A pending MCP call means the session is actively running a tool
  // (Bash/Grep/etc.) — `lastActivityAt` doesn't tick during execution,
  // so without this guard a 5-min build inside a sub-agent would be
  // killed at the 10-min sub-agent ceiling, or a 35-min `podman build`
  // in the main session would die at the 30-min idle limit.
  if (hasInflightCall(session.id)) return null

  const idleTimeoutMs = DEFAULT_IDLE_TIMEOUT_MIN * 60_000
  const effectiveIdleMs = session.isSubAgent ? SUB_AGENT_IDLE_TIMEOUT_MS : idleTimeoutMs
  if (idleMs > effectiveIdleMs) return 'idle_timeout'

  return null
}

function reapSessions(): void {
  const now = Date.now()

  for (const [, session] of sessions) {
    // Per-session isolation: destroySession() runs a cleanup chain that could
    // throw for ONE session (a bad watcher/hook teardown). Without this, the throw
    // would abort the whole sweep (every remaining session skipped that tick) — so
    // catch + continue so one bad session can't stall reaping the rest.
    try {
      const reason = decideReap(session, now)
      if (!reason) continue
      debugLog('Reaping session', {
        sessionId: session.id,
        reason,
        ageMs: now - session.createdAt.getTime(),
        idleMs: now - session.lastActivityAt.getTime(),
        isSubAgent: session.isSubAgent,
      })
      eventBus.emit('session:reaped', { sessionId: session.id, reason })
      destroySession(session.id)
    } catch (err) {
      warnLog('Reap failed for session', { sessionId: session.id, error: err instanceof Error ? err.message : String(err) })
    }
  }
}

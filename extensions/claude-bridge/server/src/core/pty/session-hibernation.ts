// ============================================================================
// Idle hibernation of Claude CLI processes.
//
// One CLI per open chat costs ~170 MB even at rest. A chat nobody used for a
// while gets its CLI stopped (the ordinary teardown: transcript archived, MCP
// endpoint released); the chat stays open in the client. The next message
// respawns the CLI with `--resume` from the same settings directory and launch
// config (session-ws.ts owns the per-socket half). The decision here only
// says *whether* a session may sleep; anything that still runs inside the
// process — a turn, a tool, an MCP task, a question to the user, a scheduled
// wakeup — keeps it awake.
// ============================================================================

import { debugLog, infoLog, warnLog } from '../logging'
import { runningShellCount } from './commands'
import { mcpTasks } from './mcp-tasks'
import { hasInflightMcpCall } from './session-mcp-call'
import { sessionCliControl } from './session-cli-control'
import { sessions } from './session-core'
import type { PtySession } from './types'

export interface HibernationConfig {
  /** Idle time before a chat's CLI is stopped; 0 turns hibernation off. */
  idleMs: number
  /** Upper bound on running Claude CLIs; 0 = no bound. When a wake would
   *  exceed it, the least recently used idle chat goes to sleep first. */
  maxAwake: number
  /** CLIs respawned at once; further wakes wait their turn (FIFO). */
  wakeConcurrency: number
  /** Shortest idle time for a chat evicted to honour maxAwake. */
  evictIdleMs: number
}

function envNumber(value: string | undefined, fallback: number): number {
  if (value === undefined || value.trim() === '') return fallback
  const n = Number(value)
  return Number.isFinite(n) && n >= 0 ? n : fallback
}

export function hibernationConfig(env: NodeJS.ProcessEnv = process.env): HibernationConfig {
  return {
    idleMs: envNumber(env.BRIDGE_HIBERNATE_IDLE_MIN, 15) * 60_000,
    maxAwake: Math.floor(envNumber(env.BRIDGE_MAX_AWAKE_SESSIONS, 0)),
    wakeConcurrency: Math.max(1, Math.floor(envNumber(env.BRIDGE_WAKE_CONCURRENCY, 4))),
    evictIdleMs: envNumber(env.BRIDGE_HIBERNATE_EVICT_IDLE_S, 60) * 1000,
  }
}

/** A ScheduleWakeup keeps the chat awake until it fired and its turn began. */
export const SCHEDULED_WAKEUP_MARGIN_MS = 60_000

export type HibernationBlocker =
  | 'not-running'
  | 'detached'
  | 'codex'
  | 'subagent'
  | 'children'
  | 'mcp-call'
  | 'mcp-task'
  | 'background-shell'
  | 'elicitation'
  | 'turn'
  | 'submission'
  | 'status-unknown'
  | 'scheduled-wakeup'
  | 'recent'

export interface BlockerDeps {
  inflightCall(sessionId: string): boolean
  workingTask(owner: string): boolean
  runningShells(session: PtySession): number
  liveWork(session: PtySession): ReturnType<NonNullable<ReturnType<typeof sessionCliControl>>['liveWork']>
  childAlive(childId: string): boolean
}

const defaultDeps: BlockerDeps = {
  inflightCall: hasInflightMcpCall,
  workingTask: (owner) => mcpTasks.hasWorkingOwner(owner),
  runningShells: runningShellCount,
  // null from liveWork() means idle; only a missing control is unknown.
  liveWork: (session) => {
    const control = sessionCliControl(session)
    return control ? control.liveWork() : 'status-unknown'
  },
  childAlive: (id) => {
    const child = sessions.get(id)
    return !!child && child.state !== 'exited'
  },
}

/** Why `session` must stay awake now, or null when it may hibernate. */
export function hibernationBlocker(
  session: PtySession,
  now: number,
  minIdleMs: number,
  deps: BlockerDeps = defaultDeps,
): HibernationBlocker | null {
  if (session.state !== 'running' || session.teardown || session.isRestarting || session.hibernating)
    return 'not-running'
  if (session.detachGraceTimer || session.detachedAt) return 'detached'
  if (session.engine === 'codex') return 'codex'
  if (session.isSubAgent) return 'subagent'
  if (session.childSessions.some((id) => deps.childAlive(id))) return 'children'
  if (deps.inflightCall(session.id)) return 'mcp-call'
  if (session.mcpEndpoint && deps.workingTask(session.mcpEndpoint.id)) return 'mcp-task'
  if (deps.runningShells(session) > 0) return 'background-shell'
  if (session.pendingElicitations.size > 0) return 'elicitation'
  const work = deps.liveWork(session)
  if (work) return work
  if (session.scheduledWakeupAt && now < session.scheduledWakeupAt + SCHEDULED_WAKEUP_MARGIN_MS)
    return 'scheduled-wakeup'
  if (now - session.lastActivityAt.getTime() < minIdleMs) return 'recent'
  return null
}

// ── Per-socket handles ──────────────────────────────────────────────────────
// session-ws registers one handle per client socket. The sweeper asks it to
// put its chat to sleep; only the socket knows how to wake it again.

export interface HibernationHandle {
  /** The awake session bound to this socket, if any. */
  awakeSessionId(): string | null
  /** Stop the CLI; false when the socket is busy (startup, switch) or gone. */
  hibernate(): boolean
}

const handles = new Set<HibernationHandle>()
let sweepTimer: ReturnType<typeof setInterval> | null = null
const SWEEP_MS = 30_000
let lastBlockedPicture = ''

export function registerHibernationHandle(handle: HibernationHandle): () => void {
  handles.add(handle)
  if (!sweepTimer && hibernationConfig().idleMs > 0) {
    sweepTimer = setInterval(() => sweepIdleSessions(), SWEEP_MS)
    sweepTimer.unref?.()
  }
  return () => {
    handles.delete(handle)
    if (handles.size === 0 && sweepTimer) {
      clearInterval(sweepTimer)
      sweepTimer = null
    }
  }
}

interface Candidate {
  handle: HibernationHandle
  session: PtySession
}

function candidates(now: number, minIdleMs: number, deps: BlockerDeps, blocked?: Map<string, number>): Candidate[] {
  const out: Candidate[] = []
  for (const handle of handles) {
    const id = handle.awakeSessionId()
    const session = id ? sessions.get(id) : undefined
    if (!session) continue
    const blocker = hibernationBlocker(session, now, minIdleMs, deps)
    if (blocker) {
      if (blocker !== 'recent') blocked?.set(blocker, (blocked.get(blocker) ?? 0) + 1)
      continue
    }
    out.push({ handle, session })
  }
  // Least recently used first.
  return out.sort((a, b) => a.session.lastActivityAt.getTime() - b.session.lastActivityAt.getTime())
}

function sleep(candidate: Candidate): boolean {
  try {
    return candidate.handle.hibernate()
  } catch (err) {
    warnLog('Hibernation failed', { sessionId: candidate.session.id, error: String(err) })
    return false
  }
}

/** One sweep: idle chats past the threshold sleep; then, if more CLIs run
 *  than allowed, the least recently used idle ones follow. */
export function sweepIdleSessions(
  now = Date.now(),
  config = hibernationConfig(),
  deps: BlockerDeps = defaultDeps,
): number {
  if (config.idleMs <= 0) return 0
  let slept = 0
  const blocked = new Map<string, number>()
  for (const candidate of candidates(now, config.idleMs, deps, blocked)) if (sleep(candidate)) slept++
  // Idle chats kept awake by live work, logged when the picture changes: the
  // answer to "why does this chat not sleep" without a debug build.
  const picture = [...blocked]
    .sort()
    .map(([k, n]) => `${k}=${n}`)
    .join(' ')
  if (picture !== lastBlockedPicture) {
    lastBlockedPicture = picture
    if (picture) infoLog('Hibernation: idle chats kept awake', { blockers: picture })
  }
  if (config.maxAwake > 0) slept += evict(awakeClaudeCount() - config.maxAwake, now, config, deps)
  if (slept) debugLog('Hibernation sweep', { slept, awake: awakeClaudeCount() })
  return slept
}

function awakeClaudeCount(): number {
  let n = 0
  for (const s of sessions.values()) if (s.engine !== 'codex' && !s.teardown && !s.hibernating) n++
  return n
}

function evict(excess: number, now: number, config: HibernationConfig, deps: BlockerDeps): number {
  let slept = 0
  if (excess <= 0) return 0
  for (const candidate of candidates(now, config.evictIdleMs, deps)) {
    if (slept >= excess) break
    if (sleep(candidate)) slept++
  }
  return slept
}

/** Before a wake or a new chat: keep the number of running CLIs within
 *  maxAwake by putting the least recently used idle chat to sleep. Never
 *  refuses — if nothing may sleep, the bound is exceeded rather than a user
 *  being turned away. */
export function makeRoomForOneMore(now = Date.now(), config = hibernationConfig()): void {
  if (config.idleMs <= 0 || config.maxAwake <= 0) return
  evict(awakeClaudeCount() + 1 - config.maxAwake, now, config, defaultDeps)
}

// ── Wake pool ────────────────────────────────────────────────────────────────
// A CLI start costs about a second of CPU. Twenty chats woken at once would
// all take several seconds; a bounded pool lets the first ones through fast.

let wakeActive = 0
const wakeQueue: Array<() => void> = []

export function acquireWakeSlot(limit = hibernationConfig().wakeConcurrency): Promise<() => void> {
  return new Promise((resolve) => {
    const grant = () => {
      wakeActive++
      let released = false
      resolve(() => {
        if (released) return
        released = true
        wakeActive--
        wakeQueue.shift()?.()
      })
    }
    if (wakeActive < limit) grant()
    else wakeQueue.push(grant)
  })
}

/** Test hook: the pool's current occupancy. */
export function wakePoolState(): { active: number; queued: number } {
  return { active: wakeActive, queued: wakeQueue.length }
}

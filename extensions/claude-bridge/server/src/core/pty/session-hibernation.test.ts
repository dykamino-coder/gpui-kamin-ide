import { afterEach, describe, expect, it, vi } from 'vitest'

const h = vi.hoisted(() => ({ sessions: new Map<string, any>(), control: undefined as any }))
vi.mock('./session-core', () => ({ sessions: h.sessions }))
vi.mock('./mcp-tasks', () => ({ mcpTasks: { hasWorkingOwner: () => false } }))
vi.mock('./session-mcp-call', () => ({ hasInflightMcpCall: () => false }))
vi.mock('./commands', () => ({ runningShellCount: () => 0 }))
vi.mock('./session-cli-control', () => ({ sessionCliControl: () => h.control }))
vi.mock('../logging', () => ({ debugLog: vi.fn(), infoLog: vi.fn(), warnLog: vi.fn() }))

import {
  acquireWakeSlot,
  hibernationBlocker,
  hibernationConfig,
  makeRoomForOneMore,
  registerHibernationHandle,
  SCHEDULED_WAKEUP_MARGIN_MS,
  sweepIdleSessions,
  wakePoolState,
  type BlockerDeps,
} from './session-hibernation'
import { noteScheduledWakeups } from './scheduled-wakeup'

const MIN = 60_000
const NOW = 100 * MIN

const idle = (): BlockerDeps => ({
  inflightCall: () => false,
  workingTask: () => false,
  runningShells: () => 0,
  liveWork: () => null,
  childAlive: () => false,
})

const session = (id: string, idleMin: number, extra: Record<string, unknown> = {}) => ({
  id,
  state: 'running',
  isRestarting: false,
  isSubAgent: false,
  childSessions: [] as string[],
  pendingElicitations: new Map(),
  mcpEndpoint: { id: `ep-${id}` },
  lastActivityAt: new Date(NOW - idleMin * MIN),
  ...extra,
})

const config = { idleMs: 15 * MIN, maxAwake: 0, wakeConcurrency: 4, evictIdleMs: MIN }

afterEach(() => h.sessions.clear())

describe('hibernationBlocker: live work keeps the CLI', () => {
  const s = () => session('a', 30) as any
  it('lets an idle chat sleep', () => {
    expect(hibernationBlocker(s(), NOW, 15 * MIN, idle())).toBeNull()
  })
  it('keeps a chat used recently', () => {
    expect(hibernationBlocker(session('a', 5) as any, NOW, 15 * MIN, idle())).toBe('recent')
  })
  it.each([
    ['a running model turn', { liveWork: () => 'turn' as const }, 'turn'],
    ['a submission the CLI has not read', { liveWork: () => 'submission' as const }, 'submission'],
    ['an unknown turn state', { liveWork: () => 'status-unknown' as const }, 'status-unknown'],
    ['an MCP tool call in flight', { inflightCall: () => true }, 'mcp-call'],
    ['a running background shell', { runningShells: () => 1 }, 'background-shell'],
    ['a backgrounded MCP task', { workingTask: (owner: string) => owner === 'ep-a' }, 'mcp-task'],
  ])('keeps %s', (_label, override, blocker) => {
    expect(hibernationBlocker(s(), NOW, 15 * MIN, { ...idle(), ...override })).toBe(blocker)
  })
  it('keeps a chat that waits for the user to answer a question', () => {
    const waiting = s()
    waiting.pendingElicitations.set('q', {})
    expect(hibernationBlocker(waiting, NOW, 15 * MIN, idle())).toBe('elicitation')
  })
  it('keeps a chat with a live sub-agent or teammate session', () => {
    const parent = { ...s(), childSessions: ['child'] }
    expect(hibernationBlocker(parent, NOW, 15 * MIN, { ...idle(), childAlive: () => true })).toBe('children')
    expect(hibernationBlocker(parent, NOW, 15 * MIN, idle())).toBeNull()
  })
  it('keeps a chat until its scheduled wakeup fired', () => {
    const due = { ...s(), scheduledWakeupAt: NOW + 10 * MIN }
    expect(hibernationBlocker(due, NOW, 15 * MIN, idle())).toBe('scheduled-wakeup')
    const later = NOW + 10 * MIN + SCHEDULED_WAKEUP_MARGIN_MS
    due.lastActivityAt = new Date(later - 20 * MIN)
    expect(hibernationBlocker(due, later, 15 * MIN, idle())).toBeNull()
  })
  it.each([
    ['codex', { engine: 'codex' }],
    ['subagent', { isSubAgent: true }],
    ['detached', { detachGraceTimer: {} }],
    ['not-running', { state: 'starting' }],
    ['not-running', { teardown: {} }],
    ['not-running', { isRestarting: true }],
    ['not-running', { hibernating: true }],
  ])('never hibernates a %s session', (blocker, extra) => {
    expect(hibernationBlocker({ ...s(), ...extra } as any, NOW, 15 * MIN, idle())).toBe(blocker)
  })
})

describe('sweep and the awake bound', () => {
  const handleFor = (id: string, slept: string[]) => ({
    awakeSessionId: () => (slept.includes(id) ? null : id),
    hibernate: () => {
      slept.push(id)
      h.sessions.get(id).hibernating = true
      return true
    },
  })

  it('puts only chats past the idle threshold to sleep', () => {
    const slept: string[] = []
    const offs = ['old', 'fresh'].map((id) => registerHibernationHandle(handleFor(id, slept)))
    h.sessions.set('old', session('old', 20))
    h.sessions.set('fresh', session('fresh', 2))
    expect(sweepIdleSessions(NOW, config, idle())).toBe(1)
    expect(slept).toEqual(['old'])
    offs.forEach((off) => off())
  })

  it('evicts least recently used idle chats down to maxAwake', () => {
    const slept: string[] = []
    const ids = ['a', 'b', 'c', 'd']
    const offs = ids.map((id) => registerHibernationHandle(handleFor(id, slept)))
    ids.forEach((id, i) => h.sessions.set(id, session(id, 5 - i)))
    // a: 5 min idle, b: 4, c: 3, d: 2 — none past 15 min; bound 2 → a, b sleep.
    sweepIdleSessions(NOW, { ...config, maxAwake: 2 }, idle())
    expect(slept).toEqual(['a', 'b'])
    offs.forEach((off) => off())
  })

  it('never evicts a chat that was active within the eviction floor', () => {
    const slept: string[] = []
    const off = registerHibernationHandle(handleFor('busy', slept))
    h.sessions.set('busy', session('busy', 0.5))
    sweepIdleSessions(NOW, { ...config, maxAwake: 0.5 as any }, idle())
    expect(slept).toEqual([])
    off()
  })

  it('makes room before one more CLI starts', () => {
    const slept: string[] = []
    const offs = ['a', 'b'].map((id) => registerHibernationHandle(handleFor(id, slept)))
    h.sessions.set('a', session('a', 3))
    h.sessions.set('b', session('b', 2))
    vi.stubEnv('BRIDGE_MAX_AWAKE_SESSIONS', '2')
    // Real deps: no CLI control is registered in this test, so a session whose
    // turn state is unknown is never evicted.
    makeRoomForOneMore(NOW)
    expect(slept).toEqual([])
    vi.unstubAllEnvs()
    offs.forEach((off) => off())
  })

  it('treats an idle CLI (liveWork null) as sleepable with the real dependencies', () => {
    const slept: string[] = []
    const off = registerHibernationHandle(handleFor('a', slept))
    h.sessions.set('a', session('a', 20))
    h.control = { liveWork: () => null }
    try {
      expect(sweepIdleSessions(NOW, config)).toBe(1)
      expect(slept).toEqual(['a'])
    } finally {
      h.control = undefined
      off()
    }
  })
  it('is off when the idle threshold is 0', () => {
    const slept: string[] = []
    const off = registerHibernationHandle(handleFor('a', slept))
    h.sessions.set('a', session('a', 600))
    expect(sweepIdleSessions(NOW, { ...config, idleMs: 0 }, idle())).toBe(0)
    off()
  })
})

describe('wake pool', () => {
  it('admits a bounded number of wakes and hands slots on in arrival order', async () => {
    const releases = await Promise.all([acquireWakeSlot(2), acquireWakeSlot(2)])
    const order: number[] = []
    const third = acquireWakeSlot(2).then((r) => (order.push(3), r))
    const fourth = acquireWakeSlot(2).then((r) => (order.push(4), r))
    expect(wakePoolState()).toEqual({ active: 2, queued: 2 })
    releases[0]!()
    releases[0]!() // idempotent
    const r3 = await third
    expect(order).toEqual([3])
    releases[1]!()
    const r4 = await fourth
    expect(order).toEqual([3, 4])
    r3()
    r4()
    expect(wakePoolState()).toEqual({ active: 0, queued: 0 })
  })
})

describe('configuration', () => {
  it('has defaults for a single user and reads overrides', () => {
    expect(hibernationConfig({})).toEqual({ idleMs: 15 * MIN, maxAwake: 0, wakeConcurrency: 4, evictIdleMs: MIN })
    expect(
      hibernationConfig({
        BRIDGE_HIBERNATE_IDLE_MIN: '0',
        BRIDGE_MAX_AWAKE_SESSIONS: '20',
        BRIDGE_WAKE_CONCURRENCY: '0',
        BRIDGE_HIBERNATE_EVICT_IDLE_S: 'junk',
      }),
    ).toEqual({ idleMs: 0, maxAwake: 20, wakeConcurrency: 1, evictIdleMs: MIN })
  })
})

describe('noteScheduledWakeups', () => {
  const call = (delaySeconds: unknown) => ({
    type: 'assistant',
    message: { content: [{ type: 'tool_use', name: 'ScheduleWakeup', input: { delaySeconds } }] },
  })
  it('records the latest due time of a scheduled wakeup', () => {
    const s: { scheduledWakeupAt?: number } = {}
    noteScheduledWakeups(s, [{ type: 'user', message: { content: 'hi' } }], NOW)
    expect(s.scheduledWakeupAt).toBeUndefined()
    noteScheduledWakeups(s, [call(1200), call(60)], NOW)
    expect(s.scheduledWakeupAt).toBe(NOW + 1_200_000)
  })
  it('treats a malformed delay as the longest one', () => {
    const s: { scheduledWakeupAt?: number } = {}
    noteScheduledWakeups(s, [call('soon')], NOW)
    expect(s.scheduledWakeupAt).toBe(NOW + 3_600_000)
  })
})

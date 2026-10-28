import { describe, expect, it, vi } from 'vitest'

const state = vi.hoisted(() => ({ working: false }))
vi.mock('./session-core', () => ({ sessions: new Map(), destroySession: vi.fn() }))
vi.mock('./mcp-tasks', () => ({ mcpTasks: { hasWorkingOwner: () => state.working } }))
vi.mock('./session-mcp-call', () => ({ hasInflightMcpCall: () => false }))
vi.mock('../events/bus', () => ({ eventBus: { emit: vi.fn() } }))
vi.mock('../logging/lifecycle', () => ({ lifecycleLog: vi.fn() }))
vi.mock('../logging', () => ({ debugLog: vi.fn(), warnLog: vi.fn() }))

import { decideReap, type ReapableSession } from './session-reaper'

describe('background task reaper ownership', () => {
  const now = 100_000_000
  const session: ReapableSession = {
    id: 'pty',
    state: 'running',
    mcpEndpoint: { id: 'owner' },
    createdAt: new Date(now - 60 * 60_000),
    lastActivityAt: new Date(now - 40 * 60_000),
  }
  it('keeps an idle-looking session while its task executes, then releases it', () => {
    state.working = true
    expect(decideReap(session, now)).toBeNull()
    state.working = false
    expect(decideReap(session, now)).toBe('idle_timeout')
  })
  it('preserves the absolute lifetime and startup bounds during a task', () => {
    state.working = true
    expect(decideReap({ ...session, createdAt: new Date(0) }, now)).toBe('max_lifetime')
    expect(decideReap({ ...session, state: 'starting' }, now)).toBe('startup_timeout')
    state.working = false
  })
})

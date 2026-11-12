import { expect, it } from 'vitest'
import { engineBreakdown, groupPtySessions } from './pty-groups'
import type { PtySessionInfo } from '../../signals/server'

const sessions: PtySessionInfo[] = Array.from({ length: 48 }, (_, i) => ({
  id: `s${i}`,
  userName: `user-${i % 17}`,
  cwd: '/synthetic',
  state: 'running',
  createdAt: '2026-11-13T00:00:00Z',
  lastActivityAt: new Date(i * 1000).toISOString(),
  mcpCallCount: 2,
  inputCount: 0,
  userMessages: 10,
  assistantMessages: 8,
  contextTokens: 8000,
  totalTokens: 12000,
  cpuPercent: 0.5,
  memBytes: 64 * 1024 ** 2,
  engine: i < 4 ? 'codex' : 'claude',
}))
it('groups 48 sessions into 17 users, sums counters/resources and uses the freshest activity', () => {
  const groups = groupPtySessions(sessions, 'activity')
  expect(groups).toHaveLength(17)
  const group = groups.find((g) => g.name === 'user-0')!
  expect(group).toMatchObject({
    userMessages: 30,
    assistantMessages: 24,
    contextTokens: 24000,
    totalTokens: 36000,
    mcpCallCount: 6,
    cpuPercent: 1.5,
    memBytes: 192 * 1024 ** 2,
    lastActivity: 34000,
  })
  expect(groups[0].sessions[0].id).toBe('s47')
  expect(groupPtySessions(sessions, 'count')[0].sessions).toHaveLength(3)
  expect(sessions[0].id).toBe('s0')
})
it('counts mixed-engine users once in the total and once in each engine they use', () => {
  expect(engineBreakdown(sessions)).toBe('48 (Claude 44 · Codex 4)')
  expect(engineBreakdown(sessions, true)).toBe('17 (Claude 17 · Codex 4)')
  expect(engineBreakdown([{ ...sessions[0], engine: undefined }])).toBe('1 (Claude 1 · Codex 0)')
})
it('keeps unavailable resources distinct from measured zero', () => {
  const [unknown] = groupPtySessions([{ ...sessions[0], cpuPercent: null, memBytes: undefined }], 'activity')
  expect(unknown.cpuPercent).toBeNull()
  expect(unknown.memBytes).toBeNull()
  expect(groupPtySessions([{ ...sessions[0], cpuPercent: 0 }], 'activity')[0].cpuPercent).toBe(0)
})

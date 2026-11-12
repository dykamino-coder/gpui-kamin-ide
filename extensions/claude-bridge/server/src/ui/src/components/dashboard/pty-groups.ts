import type { PtySessionInfo } from '../../signals/server'

export type SessionSort = 'activity' | 'count'
export interface PtyGroup {
  key: string
  name: string
  sessions: PtySessionInfo[]
  lastActivity: number
  userMessages: number
  assistantMessages: number
  mcpCallCount: number
  contextTokens: number
  totalTokens: number
  cpuPercent: number | null
  memBytes: number | null
}

export const ptyUserKey = (s: PtySessionInfo): string =>
  s.tokenId ? `token:${s.tokenId}` : `name:${s.userName || 'Unknown user'}`

/** One pass over the live snapshot; no per-row requests or history queries. */
export function groupPtySessions(sessions: readonly PtySessionInfo[], sort: SessionSort): PtyGroup[] {
  const groups = new Map<string, PtyGroup>()
  for (const s of sessions) {
    const name = s.userName || 'Unknown user'
    const key = ptyUserKey(s)
    let group = groups.get(key)
    if (!group) {
      group = {
        key,
        name,
        sessions: [],
        lastActivity: 0,
        userMessages: 0,
        assistantMessages: 0,
        mcpCallCount: 0,
        contextTokens: 0,
        totalTokens: 0,
        cpuPercent: null,
        memBytes: null,
      }
      groups.set(key, group)
    }
    group.sessions.push(s)
    const activity = Date.parse(s.lastActivityAt) || 0
    if (activity >= group.lastActivity) group.name = name
    group.lastActivity = Math.max(group.lastActivity, activity)
    group.userMessages += s.userMessages ?? 0
    group.assistantMessages += s.assistantMessages ?? 0
    group.mcpCallCount += s.mcpCallCount ?? 0
    group.contextTokens += s.contextTokens ?? 0
    group.totalTokens += s.totalTokens ?? 0
    if (typeof s.cpuPercent === 'number') group.cpuPercent = (group.cpuPercent ?? 0) + s.cpuPercent
    if (typeof s.memBytes === 'number') group.memBytes = (group.memBytes ?? 0) + s.memBytes
  }
  for (const g of groups.values()) {
    g.sessions.sort((a, b) => Date.parse(b.lastActivityAt) - Date.parse(a.lastActivityAt) || a.id.localeCompare(b.id))
  }
  return [...groups.values()].sort(
    (a, b) =>
      (sort === 'count' ? b.sessions.length - a.sessions.length : 0) ||
      b.lastActivity - a.lastActivity ||
      a.name.localeCompare(b.name) ||
      a.key.localeCompare(b.key),
  )
}

export function engineBreakdown(sessions: readonly PtySessionInfo[], users = false): string {
  const count = (list: readonly PtySessionInfo[]) => (users ? new Set(list.map(ptyUserKey)).size : list.length)
  const claude = count(sessions.filter((s) => s.engine !== 'codex'))
  const codex = count(sessions.filter((s) => s.engine === 'codex'))
  return `${count(sessions)} (Claude ${claude} · Codex ${codex})`
}

import type { AgentInfo, AgentTreeState } from '../signals/agents'

interface Cycle { idleAt: number; acknowledgedAt: number }
interface FollowUp { recipient?: string; acknowledgedAt?: number; failed?: boolean }
// Weak ownership: closing a parent or replacing replay staging releases its ledger.
// Only IDs, recipient identity and timestamps are retained; never message bodies.
const cycles = new WeakMap<AgentInfo, Cycle>()
const ledgers = new WeakMap<AgentTreeState, Map<string, FollowUp>>()
const MAX_FOLLOW_UPS = 256
const time = (value?: string): number => {
  const parsed = value ? Date.parse(value) : NaN
  return Number.isFinite(parsed) ? parsed : 0
}
function ledger(tree: AgentTreeState): Map<string, FollowUp> {
  let map = ledgers.get(tree)
  if (!map) { map = new Map(); ledgers.set(tree, map) }
  return map
}
function remember(tree: AgentTreeState, id: string, update: FollowUp): void {
  const map = ledger(tree)
  map.set(id, { ...map.get(id), ...update })
  while (map.size > MAX_FOLLOW_UPS) map.delete(map.keys().next().value!)
}
/** Never guess between same-name members; mailbox identity takes precedence. */
export function findTeammate(tree: AgentTreeState, recipient: string): AgentInfo | undefined {
  const exact = tree.agentIdToAgent.get(recipient)
  if (exact) return exact
  const candidates = new Set<AgentInfo>(tree.agentIdToAgent.values())
  for (const agent of tree.standaloneAgents.values()) candidates.add(agent)
  for (const team of tree.teams.values()) for (const agent of team.agents.values()) candidates.add(agent)
  const matches = [...candidates].filter(a => a.name === recipient || a.inputName === recipient)
  return matches.length === 1 ? matches[0] : undefined
}
export function inheritTeammateCycle(prior: AgentInfo, next: AgentInfo): void {
  const cycle = cycles.get(prior)
  if (cycle) cycles.set(next, { ...cycle })
}
export function recordTeammateIdle(agent: AgentInfo, emitted?: string, received?: string): void {
  const at = time(emitted) || time(received)
  const cycle = cycles.get(agent) ?? { idleAt: 0, acknowledgedAt: 0 }
  cycle.idleAt = Math.max(cycle.idleAt, at)
  cycles.set(agent, cycle)
  // An older event's arrival time cannot finish newer acknowledged work.
  if (agent.status === 'running' && at >= cycle.acknowledgedAt) agent.status = 'done'
}
export function applyFollowUps(tree: AgentTreeState): boolean {
  let changed = false
  for (const follow of ledger(tree).values()) {
    if (!follow.recipient || !follow.acknowledgedAt || follow.failed) continue
    const agent = findTeammate(tree, follow.recipient)
    if (!agent || agent.status === 'terminated' || agent.status === 'error') continue
    if (agent.teamName && tree.teams.get(agent.teamName)?.status === 'disbanded') continue
    const cycle = cycles.get(agent) ?? { idleAt: 0, acknowledgedAt: 0 }
    if (follow.acknowledgedAt <= Math.max(cycle.idleAt, cycle.acknowledgedAt)) continue
    cycle.acknowledgedAt = follow.acknowledgedAt
    cycles.set(agent, cycle)
    agent.status = 'running'
    agent.lastSeenAt = new Date(follow.acknowledgedAt).toISOString()
    // Cleanup archives the same object; restore that identity, not a second row.
    if (agent.teamName) {
      let team = tree.teams.get(agent.teamName)
      if (!team) { team = { name: agent.teamName, description: '', agents: new Map() }; tree.teams.set(agent.teamName, team) }
      team.agents.set(agent.name, agent)
    } else tree.standaloneAgents.set(agent.name, agent)
    changed = true
  }
  return changed
}
export function rememberFollowUp(tree: AgentTreeState, id: string, recipient: string): boolean {
  if (!id || id.length > 256 || !recipient || recipient.length > 128) return false
  remember(tree, id, { recipient })
  return applyFollowUps(tree)
}
export function acknowledgeFollowUp(tree: AgentTreeState, id: string, timestamp?: string, failed = false): boolean {
  if (!id || id.length > 256) return false
  remember(tree, id, { acknowledgedAt: time(timestamp), failed })
  return applyFollowUps(tree)
}

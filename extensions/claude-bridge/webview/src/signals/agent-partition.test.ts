// The Agents panel derives `Active` and `Completed` from ONE partition: every
// agent is on exactly one side, a terminal status moves it at once, and a badge
// always equals the rows its tab renders. Cleanup (tree → history) is storage
// only and must not change what the panel shows.
import { describe, it, expect } from 'vitest'
import { partitionAgents, isActiveAgent, type AgentPartition } from './agent-partition'
import type { AgentInfo, AgentTreeState, TeamInfo } from './agents'

function agent(name: string, status: AgentInfo['status'] = 'running', teamName?: string): AgentInfo {
  return { id: `id-${name}`, name, inputName: name, description: '', teamName, status, messages: [] }
}

function tree(teams: TeamInfo[] = [], solo: AgentInfo[] = []): AgentTreeState {
  return {
    teams: new Map(teams.map((t) => [t.name, t])),
    standaloneAgents: new Map(solo.map((a) => [a.name, a])),
    pendingAgentCalls: new Map(),
    teamNameAliases: new Map(),
    taskIdToAgent: new Map(),
    agentIdToAgent: new Map(),
  }
}

function team(name: string, members: AgentInfo[], status: TeamInfo['status'] = 'active'): TeamInfo {
  return { name, description: '', status, agents: new Map(members.map((a) => [a.name, a])) }
}

/** Names rendered on a side, in render order. */
function rows(side: AgentPartition['active']): string[] {
  return [...side.teams.flatMap((t) => t.members.map((m) => m.name)), ...side.solo.map((a) => a.name)]
}

/** The invariant every update must keep: badge === rendered rows, sides disjoint. */
function checkInvariant(p: AgentPartition): void {
  const a = rows(p.active); const c = rows(p.completed)
  expect(p.active.count).toBe(a.length)
  expect(p.completed.count).toBe(c.length)
  expect(a.filter((n) => c.includes(n))).toEqual([])
  expect(new Set([...a, ...c]).size).toBe(a.length + c.length)
}

describe('partitionAgents', () => {
  it('shows nothing for an empty session', () => {
    const p = partitionAgents(undefined, [])
    expect(p.active.count).toBe(0)
    expect(p.completed.count).toBe(0)
    checkInvariant(p)
  })

  it('keeps running teammates active and moves a terminal one at once', () => {
    const alpha = agent('alpha', 'running', 'br25'); const beta = agent('beta', 'running', 'br25')
    const t = tree([team('br25', [alpha, beta])])
    let p = partitionAgents(t, [])
    expect(rows(p.active)).toEqual(['alpha', 'beta'])
    expect(p.completed.count).toBe(0)
    checkInvariant(p)

    alpha.status = 'done' // the parser flips the status in place; no cleanup yet
    p = partitionAgents(t, [])
    expect(rows(p.active)).toEqual(['beta'])
    expect(rows(p.completed)).toEqual(['alpha'])
    expect(p.active.teams[0].members.map((m) => m.status)).toEqual(['running'])
    expect(p.completed.teams[0].name).toBe('br25')
    checkInvariant(p)
  })

  it('labels done, error and kicked distinctly on the completed side', () => {
    const t = tree([team('br25', [agent('a', 'done', 'br25'), agent('b', 'error', 'br25'), agent('c', 'terminated', 'br25')])])
    const p = partitionAgents(t, [])
    expect(p.active.count).toBe(0)
    expect(p.completed.teams[0].members.map((m) => m.status)).toEqual(['done', 'error', 'terminated'])
    checkInvariant(p)
  })

  it('puts a disbanded team whole into completed, even a member still marked running', () => {
    const t = tree([team('br25', [agent('a', 'terminated', 'br25'), agent('b', 'running', 'br25')], 'disbanded')])
    const p = partitionAgents(t, [])
    expect(p.active.count).toBe(0)
    expect(p.completed.teams[0].status).toBe('disbanded')
    expect(rows(p.completed)).toEqual(['a', 'b'])
    expect(isActiveAgent(agent('b'), { status: 'disbanded' })).toBe(false)
    checkInvariant(p)
  })

  it('cleanup moving a finished agent from the tree to history changes nothing visible', () => {
    const alpha = agent('alpha', 'done', 'br25'); const beta = agent('beta', 'running', 'br25')
    const t = tree([team('br25', [alpha, beta])])
    const before = partitionAgents(t, [])
    // scheduleCleanup(): record into history, drop from the tree
    t.teams.get('br25')!.agents.delete('alpha')
    const after = partitionAgents(t, [alpha])
    expect(rows(after.active)).toEqual(rows(before.active))
    expect(rows(after.completed)).toEqual(rows(before.completed))
    expect(after.completed.count).toBe(before.completed.count)
    expect(after.completed.teams[0].name).toBe('br25') // history keeps its team grouping
    checkInvariant(after)
  })

  it('counts a name once: the tree record wins over a stale history record', () => {
    const live = agent('alpha', 'running', 'br25')
    const t = tree([team('br25', [live])])
    const p = partitionAgents(t, [agent('alpha', 'done', 'br25')]) // re-run of the same name
    expect(rows(p.active)).toEqual(['alpha'])
    expect(p.completed.count).toBe(0)
    checkInvariant(p)
  })

  it('ignores a history record that still claims to be running', () => {
    const p = partitionAgents(tree(), [agent('ghost', 'running')])
    expect(p.active.count).toBe(0)
    expect(p.completed.count).toBe(0)
  })

  it('splits standalone agents by status too', () => {
    const t = tree([], [agent('solo-live'), agent('solo-done', 'done')])
    const p = partitionAgents(t, [agent('old', 'error')])
    expect(rows(p.active)).toEqual(['solo-live'])
    expect(rows(p.completed)).toEqual(['solo-done', 'old'])
    checkInvariant(p)
  })
})

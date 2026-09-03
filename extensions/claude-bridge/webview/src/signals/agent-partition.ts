import type { AgentInfo, AgentTreeState, TeamInfo } from './agents'

/** One team as the Agents panel shows it on ONE side of the partition: only the
 *  members that belong to that side. */
export interface TeamView {
  name: string
  description: string
  status: 'active' | 'disbanded'
  members: AgentInfo[]
}

export interface PartitionSide {
  teams: TeamView[]
  solo: AgentInfo[]
  /** Rows rendered on this side — the badge value, by construction. */
  count: number
}

/** The single lifecycle partition behind the `Active` / `Completed` tabs. Every
 *  agent of the session sits in exactly one side; the badge of a side equals
 *  the number of rows it renders. */
export interface AgentPartition {
  active: PartitionSide
  completed: PartitionSide
}

/** An agent is active only while it is `running` inside a team that has not
 *  been disbanded (or standalone). Everything else — done, error, kicked, a
 *  member of a dissolved team, a pruned history record — is completed. */
export function isActiveAgent(agent: AgentInfo, team?: Pick<TeamInfo, 'status'>): boolean {
  return agent.status === 'running' && team?.status !== 'disbanded'
}

/** Derive the partition from one snapshot: the live tree first, then the
 *  history of agents the tree already pruned. A name is counted once — the
 *  tree record wins over history, so cleanup (which only moves a finished
 *  agent from the tree to history) never changes what the panel shows. */
export function partitionAgents(tree: AgentTreeState | undefined, history: readonly AgentInfo[]): AgentPartition {
  const active: PartitionSide = { teams: [], solo: [], count: 0 }
  const completed: PartitionSide = { teams: [], solo: [], count: 0 }
  const seen = new Set<string>()
  const teamViews = new Map<string, { active?: TeamView; completed?: TeamView }>()

  const teamSide = (team: TeamInfo, side: PartitionSide, key: 'active' | 'completed'): TeamView => {
    const slot = teamViews.get(team.name) ?? {}
    let view = slot[key]
    if (!view) {
      view = { name: team.name, description: team.description, status: team.status ?? 'active', members: [] }
      slot[key] = view
      teamViews.set(team.name, slot)
      side.teams.push(view)
    }
    return view
  }

  if (tree) {
    for (const team of tree.teams.values()) {
      for (const agent of team.agents.values()) {
        if (seen.has(agent.name)) continue
        seen.add(agent.name)
        const side = isActiveAgent(agent, team) ? active : completed
        teamSide(team, side, side === active ? 'active' : 'completed').members.push(agent)
        side.count += 1
      }
    }
    for (const agent of tree.standaloneAgents.values()) {
      if (seen.has(agent.name)) continue
      seen.add(agent.name)
      const side = isActiveAgent(agent) ? active : completed
      side.solo.push(agent)
      side.count += 1
    }
  }
  for (const agent of history) {
    if (seen.has(agent.name)) continue
    seen.add(agent.name)
    // History only ever holds finished agents; a stale `running` record would
    // otherwise resurrect a row the tree already closed.
    if (agent.status === 'running') continue
    const team = agent.teamName ? tree?.teams.get(agent.teamName) : undefined
    if (agent.teamName) {
      const stub: TeamInfo = team ?? { name: agent.teamName, description: '', status: 'active', agents: new Map() }
      teamSide(stub, completed, 'completed').members.push(agent)
    } else {
      completed.solo.push(agent)
    }
    completed.count += 1
  }
  return { active, completed }
}

import type { JSX } from 'preact'
import { useState, useEffect, useRef } from 'preact/hooks'
import { tabAgentTrees, tabAgentHistory, findAgentByName, agentEntriesWithLive, type AgentInfo, type TeamInfo } from '../../signals/agents'
import { jsonlEntriesByTab } from '../../signals/jsonl'
import { activeTabId } from '../../signals/tabs'
import { JsonlEntry } from '../jsonl-viewer/JsonlEntry'
import { PanelTabs } from '../ui/PanelTabs'
import type { JsonlEntryData } from '../../types/jsonl'
import styles from './AgentsToolPanel.module.css'

/** Human label for an agent status — `terminated` reads as "kicked" (the lead
 *  shut it down) to distinguish it from a natural `done`. */
const STATUS_LABEL: Record<AgentInfo['status'], string> = {
  running: 'running', done: 'done', error: 'error', terminated: 'kicked',
}

/** The Agents tool panel (claudeBridgeAgentsView). A browser for the session's
 *  teammates/subagents rendered as a TREE: each TEAM is a group (with an
 *  active/disbanded badge) over its member rows, plus any standalone agents and
 *  a History section of ones the tree already pruned. Clicking a member reads
 *  its chat inline (Back returns). Reads the agent tree directly so it populates
 *  wherever `agentData` is on for this view. */
export function AgentsToolPanel(): JSX.Element {
  const tabId = activeTabId.value
  const tree = tabId ? tabAgentTrees.value.get(tabId) : undefined
  const teams = tree ? [...tree.teams.values()] : []
  const standalone = tree ? [...tree.standaloneAgents.values()] : []
  const liveNames = new Set([...teams.flatMap((t) => [...t.agents.values()]), ...standalone].map((a) => a.name))
  const history = ((tabId ? tabAgentHistory.value.get(tabId) : undefined) ?? []).filter((a) => !liveNames.has(a.name))

  const [tab, setTab] = useState<'active' | 'completed'>('active')
  const [selected, setSelected] = useState<string | null>(null)
  const known = new Set<string>([...liveNames, ...history.map((a) => a.name)])
  const sel = selected && known.has(selected) ? selected : null
  if (sel) return <AgentReader name={sel} onBack={() => { setSelected(null) }} />

  if (teams.length === 0 && standalone.length === 0 && history.length === 0) {
    return (
      <div class={styles.empty}>
        <i class="fas fa-users" style="font-size:26px;color:var(--text-disabled)" />
        <span class={styles.emptyTitle}>Agents</span>
        <span class={styles.emptyHint}>Teammates spawned in this session appear here — click one to read its chat.</span>
      </div>
    )
  }

  const open = (name: string): void => { setSelected(name) }
  // Active = the live structure (teams still going + standalone). Completed =
  // dissolved teams + the pruned-agent history.
  const activeTeams = teams.filter((t) => t.status !== 'disbanded')
  const disbandedTeams = teams.filter((t) => t.status === 'disbanded')
  const activeCount = activeTeams.flatMap((t) => [...t.agents.values()]).filter((a) => a.status === 'running').length
    + standalone.filter((a) => a.status === 'running').length
  const completedCount = history.length + disbandedTeams.reduce((n, t) => n + t.agents.size, 0)

  return (
    <div style="display:flex;flex-direction:column;height:100%">
      <PanelTabs
        active={tab}
        onChange={setTab}
        tabs={[
          { key: 'active', label: 'Active', count: activeCount },
          { key: 'completed', label: 'Completed', count: completedCount },
        ]}
      />
      <div class={styles.tree} style="flex:1;overflow-y:auto">
        {tab === 'active' ? (
          activeTeams.length === 0 && standalone.length === 0
            ? <div class={styles.tabEmpty}>No active agents.</div>
            : <>
                {activeTeams.map((team) => <TeamGroup key={`team:${team.name}`} team={team} onOpen={open} />)}
                {standalone.length > 0 && (
                  <div class={styles.section}>
                    {standalone.map((a) => <AgentRow key={a.name} agent={a} onClick={() => { open(a.name) }} />)}
                  </div>
                )}
              </>
        ) : (
          disbandedTeams.length === 0 && history.length === 0
            ? <div class={styles.tabEmpty}>Nothing completed yet.</div>
            : <>
                {disbandedTeams.map((team) => <TeamGroup key={`team:${team.name}`} team={team} onOpen={open} />)}
                {history.length > 0 && (
                  <div class={styles.section}>
                    {history.map((a) => <AgentRow key={`h:${a.name}`} agent={a} onClick={() => { open(a.name) }} />)}
                  </div>
                )}
              </>
        )}
      </div>
    </div>
  )
}

function TeamGroup({ team, onOpen }: { team: TeamInfo; onOpen: (name: string) => void }): JSX.Element {
  const members = [...team.agents.values()]
  const running = members.filter((m) => m.status === 'running').length
  return (
    <div class={styles.teamGroup}>
      <div class={styles.teamHeader} title={team.description || team.name}>
        <i class="fas fa-users" style="font-size:11px;flex-shrink:0" aria-hidden="true" />
        <span class={styles.teamName}>{team.name}</span>
        {team.status === 'disbanded'
          ? <span class={`${styles.badge} ${styles.disbanded}`}>disbanded</span>
          : <span class={styles.badge}>{running}/{members.length} live</span>}
      </div>
      {members.length === 0
        ? <div class={styles.teamEmpty}>no members</div>
        : members.map((a) => <AgentRow key={a.name} agent={a} nested onClick={() => { onOpen(a.name) }} />)}
    </div>
  )
}

function AgentRow({ agent, nested, onClick }: { agent: AgentInfo; nested?: boolean; onClick: () => void }): JSX.Element {
  return (
    <button
      type="button"
      class={`${styles.row} ${nested ? styles.nested : ''}`}
      onClick={onClick}
      title={`${agent.name}${agent.agentType ? ` · ${agent.agentType}` : ''} — ${STATUS_LABEL[agent.status]}`}
    >
      <span class={`${styles.dot} ${styles[agent.status]}`} aria-hidden="true" />
      <span class={styles.name}>{agent.name}</span>
      {agent.agentType && agent.agentType !== agent.name && <span class={styles.type}>{agent.agentType}</span>}
      <span class={`${styles.status} ${styles[agent.status]}`}>{STATUS_LABEL[agent.status]}</span>
    </button>
  )
}

function AgentReader({ name, onBack }: { name: string; onBack: () => void }): JSX.Element {
  const tabId = activeTabId.value
  const tree = tabId ? tabAgentTrees.value.get(tabId) : undefined
  // Live tree first, else the history record (a finished agent left the tree but
  // its transcript survives in subagentTileState).
  const info = findAgentByName(tree, name)
    ?? (tabId ? tabAgentHistory.value.get(tabId) : undefined)?.find((a) => a.name === name)
  // Subscribe to the main store so the live streaming stub repaints as it flushes.
  void (tabId ? jsonlEntriesByTab.value.get(tabId)?.length : 0)
  const entries = agentEntriesWithLive(tabId, name, info?.agentType) as JsonlEntryData[]

  const bodyRef = useRef<HTMLDivElement | null>(null)
  const prevLenRef = useRef(0)
  useEffect(() => {
    const el = bodyRef.current
    if (!el) return
    if (entries.length > prevLenRef.current) {
      const atBottom = el.scrollHeight - el.scrollTop - el.clientHeight < 60
      if (atBottom) el.scrollTop = el.scrollHeight
    }
    prevLenRef.current = entries.length
  }, [entries.length])

  return (
    <div class={styles.reader}>
      <div class={styles.readerHeader}>
        <button type="button" class={styles.back} onClick={onBack}>
          <i class="fas fa-arrow-left" aria-hidden="true" />
          <span>Agents</span>
        </button>
        <span class={styles.readerTitle}>{name}</span>
        {info?.status && <span class={`${styles.status} ${styles[info.status]}`}>{STATUS_LABEL[info.status]}</span>}
      </div>
      <div class={styles.readerBody} ref={bodyRef}>
        {entries.length === 0
          ? <div class={styles.empty}><span class={styles.emptyHint}>No messages from this agent yet.</span></div>
          : entries.map((entry, i) => <JsonlEntry key={i} entry={entry} />)}
      </div>
    </div>
  )
}

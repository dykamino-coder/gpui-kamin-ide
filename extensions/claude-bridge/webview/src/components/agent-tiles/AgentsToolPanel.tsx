import type { JSX } from 'preact'
import { useState, useEffect, useRef } from 'preact/hooks'
import { tabAgentTrees, tabAgentHistory, findAgentByName, agentEntriesWithLive, type AgentInfo } from '../../signals/agents'
import { partitionAgents, type PartitionSide, type TeamView } from '../../signals/agent-partition'
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
 *  teammates/subagents rendered as a TREE: each TEAM is a group over its member
 *  rows, plus standalone agents. `Active` and `Completed` are the two sides of
 *  ONE lifecycle partition (`partitionAgents`): every agent is on exactly one
 *  side, a terminal agent moves to `Completed` the moment its status changes,
 *  and each badge equals the rows its tab renders. The 5 s cleanup that moves
 *  finished agents from the tree into history changes storage only — the
 *  partition reads both. Clicking a row reads its chat inline (Back returns). */
export function AgentsToolPanel(): JSX.Element {
  const tabId = activeTabId.value
  const tree = tabId ? tabAgentTrees.value.get(tabId) : undefined
  const history = (tabId ? tabAgentHistory.value.get(tabId) : undefined) ?? []
  const partition = partitionAgents(tree, history)

  const [tab, setTab] = useState<'active' | 'completed'>('active')
  const [selected, setSelected] = useState<string | null>(null)
  const known = new Set<string>()
  for (const side of [partition.active, partition.completed]) {
    for (const t of side.teams) for (const m of t.members) known.add(m.name)
    for (const a of side.solo) known.add(a.name)
  }
  const sel = selected && known.has(selected) ? selected : null
  if (sel) return <AgentReader name={sel} onBack={() => { setSelected(null) }} />

  if (partition.active.count === 0 && partition.completed.count === 0) {
    return (
      <div class={styles.empty}>
        <i class="fas fa-users" style="font-size:26px;color:var(--text-disabled)" />
        <span class={styles.emptyTitle}>Agents</span>
        <span class={styles.emptyHint}>Teammates spawned in this session appear here — click one to read its chat.</span>
      </div>
    )
  }

  const open = (name: string): void => { setSelected(name) }
  const side = tab === 'active' ? partition.active : partition.completed

  return (
    <div style="display:flex;flex-direction:column;height:100%">
      <PanelTabs
        active={tab}
        onChange={setTab}
        tabs={[
          { key: 'active', label: 'Active', count: partition.active.count },
          { key: 'completed', label: 'Completed', count: partition.completed.count },
        ]}
      />
      <div class={styles.tree} style="flex:1;overflow-y:auto">
        {side.count === 0
          ? <div class={styles.tabEmpty}>{tab === 'active' ? 'No active agents.' : 'Nothing completed yet.'}</div>
          : <SideRows side={side} onOpen={open} />}
      </div>
    </div>
  )
}

function SideRows({ side, onOpen }: { side: PartitionSide; onOpen: (name: string) => void }): JSX.Element {
  return (
    <>
      {side.teams.map((team) => <TeamGroup key={`team:${team.name}`} team={team} onOpen={onOpen} />)}
      {side.solo.length > 0 && (
        <div class={styles.section}>
          {side.solo.map((a) => <AgentRow key={a.name} agent={a} onClick={() => { onOpen(a.name) }} />)}
        </div>
      )}
    </>
  )
}

function TeamGroup({ team, onOpen }: { team: TeamView; onOpen: (name: string) => void }): JSX.Element {
  const running = team.members.filter((m) => m.status === 'running').length
  const badge = team.status === 'disbanded'
    ? 'disbanded'
    : running > 0 ? `${running} live` : `${team.members.length} finished`
  return (
    <div class={styles.teamGroup}>
      <div class={styles.teamHeader} title={team.description || team.name}>
        <i class="fas fa-users" style="font-size:11px;flex-shrink:0" aria-hidden="true" />
        <span class={styles.teamName}>{team.name}</span>
        <span class={`${styles.badge} ${team.status === 'disbanded' ? styles.disbanded : ''}`}>{badge}</span>
      </div>
      {team.members.length === 0
        ? <div class={styles.teamEmpty}>no members</div>
        : team.members.map((a) => <AgentRow key={a.name} agent={a} nested onClick={() => { onOpen(a.name) }} />)}
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

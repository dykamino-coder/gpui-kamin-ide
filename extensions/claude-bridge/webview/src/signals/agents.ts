import { signal } from '@preact/signals'
import { boundAgentTranscript, boundAgentSlots } from './agent-retention'
import { jsonlEntriesByTab } from './jsonl'

export interface AgentInfo {
  id: string
  name: string
  inputName: string
  description: string
  teamName?: string
  /** running = working; done = finished its task (idle_notification / completed);
   *  error = failed; terminated = kicked/shut down by the lead (not a natural
   *  finish). */
  status: 'running' | 'done' | 'error' | 'terminated'
  agentType?: string
  taskId?: string
  agentId?: string
  messages: Array<{ from: string; text: string; ts: number }>
  lastSeenAt?: string
  // Completion summary, read from the Agent tool_result's structured
  // `toolUseResult` (not the free-text): richer + more reliable than regex.
  totalTokens?: number
  totalToolUseCount?: number
  durationMs?: number
}

export interface TeamInfo {
  name: string
  description: string
  /** active = live; disbanded = TeamDelete seen (kept so the panel can show the
   *  team was dissolved rather than silently dropping it). */
  status?: 'active' | 'disbanded'
  agents: Map<string, AgentInfo>
}

export interface AgentTreeState {
  teams: Map<string, TeamInfo>
  standaloneAgents: Map<string, AgentInfo>
  pendingAgentCalls: Map<string, AgentInfo>
  teamNameAliases: Map<string, string>
  taskIdToAgent: Map<string, AgentInfo>
  agentIdToAgent: Map<string, AgentInfo>
}

export const tabAgentTrees = signal<Map<string, AgentTreeState>>(new Map())
export const tabJsonlLive = signal<Set<string>>(new Set())

/** Finished agents kept per tab so the Agents panel can show HISTORY, not only
 *  the currently-live tree (which prunes done/error agents after 5s). Captured
 *  at prune time. Their transcripts survive in `subagentTileState` (keyed by
 *  tab and agent file identity), so a history row can still open the full chat. */
export const tabAgentHistory = signal<Map<string, AgentInfo[]>>(new Map())

/** Record a finished agent into a tab's history (newest first, de-duped by spawn identity
 *  so another same-name run retains its own history). */
export function recordAgentHistory(tabId: string, agent: AgentInfo): void {
  const cur = tabAgentHistory.value
  const list = cur.get(tabId) ?? []
  const next = [agent, ...list.filter((a) => agentIdentity(a) !== agentIdentity(agent))]
  const m = new Map(cur); m.set(tabId, next); tabAgentHistory.value = m
}

export interface SubagentTileState {
  tabId?: string
  agentName?: string
  agentId?: string
  retainedBytes?: number
  touchedAt?: number
  tileKey: string
  entries: any[]
  /** uuid-дедуп: реплей приходит повторно на каждый resync/реаттач, и без
   *  этого транскрипт агента множился (54 записи из 9 реальных). */
  seen?: Set<string>
}
export const subagentTileState = signal<Map<string, SubagentTileState>>(new Map())

/** Which subagent's chat is expanded to fill the whole iframe (null = none).
 *  Keyed by tab and stable spawn identity, so
 *  the fullscreen view and the buttons row resolve the same per-agent entries.
 *  Cleared on tab close and whenever the named agent leaves the active tree. */
export const fullscreenAgentId = signal<string | null>(null)

/** Release a closed tab's per-tab agent state so the memory of a closed session
 *  is freed (onTabClosed used to drop only `tabs`, leaking the agent tree + the
 *  live-flag entry forever → cumulative growth toward the shared WebView2 OOM).
 *  Transcript slots carry the owning tab and are released with it. */
export function clearAgentTabState(tabId: string): void {
  if (tabAgentTrees.value.has(tabId)) {
    const m = new Map(tabAgentTrees.value); m.delete(tabId); tabAgentTrees.value = m
  }
  if (tabJsonlLive.value.has(tabId)) {
    const s = new Set(tabJsonlLive.value); s.delete(tabId); tabJsonlLive.value = s
  }
  if (tabAgentHistory.value.has(tabId)) {
    const m = new Map(tabAgentHistory.value); m.delete(tabId); tabAgentHistory.value = m
  }
  subagentTileState.value = new Map([...subagentTileState.value].filter(([, state]) => state.tabId !== tabId))
  if (fullscreenAgentId.value) {
    try { if (JSON.parse(fullscreenAgentId.value)[0] === tabId) fullscreenAgentId.value = null }
    catch { fullscreenAgentId.value = null }
  }
}

/** Look up an agent by name across the tab's teams + standalone agents. Shared
 *  by the buttons row and the fullscreen view so both resolve identically. */
export function findAgentByName(tree: AgentTreeState | undefined, name: string): AgentInfo | undefined {
  if (!tree) return undefined
  const solo = tree.standaloneAgents.get(name)
  if (solo) return solo
  for (const team of tree.teams.values()) {
    const a = team.agents.get(name)
    if (a) return a
  }
  return undefined
}

/** All agents in a tab's tree (team members first, then standalone), de-duped by
 *  spawn identity so an agent briefly in two buckets renders one chip. */
export function listAgents(tree: AgentTreeState | undefined): AgentInfo[] {
  if (!tree) return []
  const out: AgentInfo[] = []
  const seen = new Set<string>()
  const add = (a: AgentInfo): void => { if (!seen.has(agentIdentity(a))) { seen.add(agentIdentity(a)); out.push(a) } }
  for (const team of tree.teams.values()) for (const a of team.agents.values()) add(a)
  for (const a of tree.standaloneAgents.values()) add(a)
  return out
}

/** Stable tree identity includes team and spawn ID, never display name alone. */
export function agentIdentity(agent: AgentInfo): string {
  return JSON.stringify([agent.teamName ?? '', agent.id])
}
export function agentSelectionKey(tabId: string, agent: AgentInfo): string {
  return JSON.stringify([tabId, agentIdentity(agent)])
}
function transcriptId(id: string): string { return id.replace(/^agent-/, '') }
export function transcriptKey(tabId: string, name: string, agentId?: string): string {
  return JSON.stringify([tabId, agentId ? 'id' : 'name', agentId ? transcriptId(agentId) : name])
}

/** ID lookup is authoritative. Legacy name/type fallback must be unambiguous
 * within this tab and cannot substitute a different known ID. */
export function agentEntries(tabId: string | null, name: string, agentType?: string, agentId?: string): unknown[] {
  if (!tabId) return []
  const m = subagentTileState.value
  if (agentId) {
    const exact = m.get(transcriptKey(tabId, name, agentId))
    if (exact) return exact.entries
  }
  const candidates = [...m.values()].filter(state => state.tabId === tabId
    && (state.agentName === name || (agentType && state.agentName === agentType)))
  if (candidates.length !== 1) return []
  const state = candidates[0]
  // Team mailbox IDs (name@team/session) are not transcript-file hashes.
  // They may use a unique local name/type alias; comparable file IDs may not.
  if (agentId && !agentId.includes('@') && state.agentId) return []
  return state.entries
}

/** Listener-owned canonical ingest. Each stable file ID owns one slot in one
 * tab; a same-name file never adopts another ID's transcript. */
export function ingestAgentEntries(tabId: string, agentName: string, entries: any[], agentId?: string): void {
  const key = transcriptKey(tabId, agentName, agentId)
  const map = subagentTileState.value
  let state = map.get(key)
  if (!state) {
    const seen = new Set<string>()
    for (const entry of entries) if (entry.uuid) seen.add(entry.uuid)
    state = { tabId, agentName, agentId, tileKey: '', entries: [...entries], seen }
  } else {
    const seen = state.seen ?? (state.seen = new Set())
    const fresh = entries.filter(entry => { if (!entry.uuid) return true; if (seen.has(entry.uuid)) return false; seen.add(entry.uuid); return true })
    state.entries.push(...fresh)
  }
  boundAgentTranscript(state)
  const next = new Map(map); next.set(key, state); subagentTileState.value = boundAgentSlots(next)
}

/** `<name>-<n>@<team>` → `<name>`: the base agent name a subagentId belongs to,
 *  matching how the canonical tile stream keys agents (by agentType). */
function subagentBaseName(subagentId: string): string {
  return subagentId.split('@')[0].replace(/-\d+$/, '')
}

/** Canonical per-agent entries PLUS the LIVE sidechain streaming stub, if that
 *  agent is mid-turn. The stub lives in the main store (hidden from the chat)
 *  with `subagentId` + live deltas already applied — so pulling it here shows an
 *  agent's tokens as they stream, before the (seconds-later) canonical JSONL
 *  lands. De-duped by message.id so a settled turn isn't shown twice. */
export function agentEntriesWithLive(tabId: string | null, name: string, agentType?: string, agentId?: string, teamName?: string): unknown[] {
  const canonical = agentEntries(tabId, name, agentType, agentId) as Array<{ message?: { id?: string } }>
  if (!tabId) return canonical
  const store = (jsonlEntriesByTab.value.get(tabId) ?? []) as Array<{
    __streaming?: boolean; isSidechain?: boolean; subagentId?: string; message?: { id?: string }
  }>
  const targets = new Set([name, agentType].filter((v): v is string => !!v))
  const seen = new Set(canonical.map((e) => e.message?.id).filter(Boolean))
  const live = store.filter((e) => {
    if (e.__streaming === undefined || !e.isSidechain || !e.subagentId) return false
    if (agentId && !agentId.includes('@')) {
      if (transcriptId(e.subagentId) !== transcriptId(agentId)) return false
    } else {
      if (!targets.has(subagentBaseName(e.subagentId)) && !targets.has(e.subagentId)) return false
      if (teamName && e.subagentId.split('@')[1] !== teamName) return false
    }
    const mid = e.message?.id
    return !mid || !seen.has(mid)
  })
  return (live.length ? [...canonical, ...live] : canonical).slice(-600)
}


/** A full export uses a file ID from an unambiguous cache slot, never a name as
 * a filesystem path. Distinct mailbox IDs need a unique name/type match. */
export function agentTranscriptDownloadId(tabId: string | null, name: string, agentType?: string, agentId?: string): string | undefined {
  if (!tabId) return undefined
  const states = [...new Set(subagentTileState.value.values())].filter(state => state.tabId === tabId)
  if (agentId && !agentId.includes('@')) return agentId.replace(/^agent-/, '')
  const candidates = states.filter(state => state.agentName === name || (agentType && state.agentName === agentType))
  return candidates.length === 1 ? candidates[0].agentId?.replace(/^agent-/, '') : undefined
}

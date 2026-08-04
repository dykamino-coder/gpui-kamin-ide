import { signal } from '@preact/signals'
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
 *  agent, never tab-pruned), so a history row can still open the full chat. */
export const tabAgentHistory = signal<Map<string, AgentInfo[]>>(new Map())

/** Record a finished agent into a tab's history (newest first, de-duped by name
 *  so a re-run replaces the stale record). */
export function recordAgentHistory(tabId: string, agent: AgentInfo): void {
  const cur = tabAgentHistory.value
  const list = cur.get(tabId) ?? []
  const next = [agent, ...list.filter((a) => a.name !== agent.name)]
  const m = new Map(cur); m.set(tabId, next); tabAgentHistory.value = m
}

export interface SubagentTileState {
  tileKey: string
  entries: any[]
  /** uuid-дедуп: реплей приходит повторно на каждый resync/реаттач, и без
   *  этого транскрипт агента множился (54 записи из 9 реальных). */
  seen?: Set<string>
}
export const subagentTileState = signal<Map<string, SubagentTileState>>(new Map())

/** Which subagent's chat is expanded to fill the whole iframe (null = none).
 *  Keyed by agent NAME — the same key as `subagentTileState` / `openTiles`, so
 *  the fullscreen view and the buttons row resolve the same per-agent entries.
 *  Cleared on tab close and whenever the named agent leaves the active tree. */
export const fullscreenAgentId = signal<string | null>(null)

/** Release a closed tab's per-tab agent state so the memory of a closed session
 *  is freed (onTabClosed used to drop only `tabs`, leaking the agent tree + the
 *  live-flag entry forever → cumulative growth toward the shared WebView2 OOM).
 *  NB: `subagentTileState` is keyed by agent, not tab, so it isn't pruned here —
 *  its growth is bounded instead at the source, by a per-agent entry cap in the
 *  onJsonlSubagentEntries handler (SUBAGENT_TILE_MAX_ENTRIES), and it is only
 *  accumulated in panels that actually render tiles (chat + Agents section). */
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
  fullscreenAgentId.value = null // a closed tab must not leave its subagent overlay up
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
 *  name so an agent that briefly appears in two buckets renders one chip. */
export function listAgents(tree: AgentTreeState | undefined): AgentInfo[] {
  if (!tree) return []
  const out: AgentInfo[] = []
  const seen = new Set<string>()
  const add = (a: AgentInfo): void => { if (!seen.has(a.name)) { seen.add(a.name); out.push(a) } }
  for (const team of tree.teams.values()) for (const a of team.agents.values()) add(a)
  for (const a of tree.standaloneAgents.values()) add(a)
  return out
}

/** Entries collected for an agent — by name, falling back to its agentType key
 *  (the tile stream is stored under whichever the source used). */
export function agentEntries(name: string, agentType?: string): unknown[] {
  const m = subagentTileState.value
  const st = m.get(name) ?? (agentType ? m.get(agentType) : undefined)
  return st?.entries ?? []
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
export function agentEntriesWithLive(tabId: string | null, name: string, agentType?: string): unknown[] {
  const canonical = agentEntries(name, agentType) as Array<{ message?: { id?: string } }>
  if (!tabId) return canonical
  const store = (jsonlEntriesByTab.value.get(tabId) ?? []) as Array<{
    __streaming?: boolean; isSidechain?: boolean; subagentId?: string; message?: { id?: string }
  }>
  const targets = new Set([name, agentType].filter((v): v is string => !!v))
  const seen = new Set(canonical.map((e) => e.message?.id).filter(Boolean))
  const live = store.filter((e) => {
    if (e.__streaming === undefined || !e.isSidechain || !e.subagentId) return false
    if (!targets.has(subagentBaseName(e.subagentId)) && !targets.has(e.subagentId)) return false
    const mid = e.message?.id
    return !mid || !seen.has(mid)
  })
  return live.length ? [...canonical, ...live] : canonical
}

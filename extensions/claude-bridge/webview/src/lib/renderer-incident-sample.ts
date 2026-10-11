import { SCROLL_UP_MAX, STORE_WINDOW } from '../signals/jsonl'
import { readSharedHeapMB } from './host-ready'

export type IncidentSampleRole = 'chat' | 'tools' | 'customize'

export interface RendererIncidentSample {
  role: IncidentSampleRole
  heapMB?: number
  retainedTabs: number
  retainedEntries: number
  activeEntries: number
  storeWindow: number
  scrollUpMax: number
  windowState: 'within-configured-window' | 'over-configured-window'
  /** Subagent transcript retention (INC-2026-0008). Always counted here; the
   *  extension host forwards it to the incident log only when the debug
   *  acceptance switch KAMIN_DEBUG_AGENT_RETENTION=1 is set. Counts only. */
  agentRetention?: AgentRetentionCounters
}

export interface AgentRetentionCounters {
  slots: number
  storedEntries: number
  uuidIndex: number
  closedTabSlots: number
}

interface RetentionSlot { tabId?: string; entries: readonly unknown[]; seen?: { size: number } }

/** Counters over the subagent transcript map; aliases of one slot count once. */
export function countAgentRetention(
  slots: ReadonlyMap<string, RetentionSlot>,
  openTabIds: ReadonlySet<string>,
): AgentRetentionCounters {
  const unique = new Set(slots.values())
  let storedEntries = 0, uuidIndex = 0, closedTabSlots = 0
  for (const slot of unique) {
    storedEntries += slot.entries.length
    uuidIndex += slot.seen?.size ?? 0
    if (slot.tabId !== undefined && !openTabIds.has(slot.tabId)) closedTabSlots += 1
  }
  return { slots: unique.size, storedEntries, uuidIndex, closedTabSlots }
}

export function buildRendererIncidentSample(
  role: IncidentSampleRole,
  store: ReadonlyMap<string, readonly unknown[]>,
  activeTabId: string | null,
  heapMB = readSharedHeapMB(),
  agentRetention?: AgentRetentionCounters,
): RendererIncidentSample {
  let retainedEntries = 0
  for (const entries of store.values()) retainedEntries += entries.length
  const activeEntries = activeTabId ? (store.get(activeTabId)?.length ?? 0) : 0
  return {
    role,
    ...(heapMB === undefined ? {} : { heapMB }),
    retainedTabs: store.size,
    retainedEntries,
    activeEntries,
    storeWindow: STORE_WINDOW,
    scrollUpMax: SCROLL_UP_MAX,
    windowState: activeEntries > SCROLL_UP_MAX ? 'over-configured-window' : 'within-configured-window',
    ...(agentRetention ? { agentRetention } : {}),
  }
}

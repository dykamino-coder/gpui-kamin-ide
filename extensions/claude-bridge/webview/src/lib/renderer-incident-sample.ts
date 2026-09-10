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
}

export function buildRendererIncidentSample(
  role: IncidentSampleRole,
  store: ReadonlyMap<string, readonly unknown[]>,
  activeTabId: string | null,
  heapMB = readSharedHeapMB(),
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
  }
}

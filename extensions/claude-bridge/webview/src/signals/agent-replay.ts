import { tabAgentTrees, type AgentTreeState } from './agents'
import { orderEntries } from './order-entries'
import { createAgentTree, parseAgentEntriesInto } from '../hooks/useAgentTree'

/** One replay generation being collected for a tab. Nothing here is visible:
 *  the panel keeps reading the last published snapshot in `tabAgentTrees`
 *  until `publishAgentReplay()` swaps it atomically. */
interface Staging {
  generation: number
  entries: unknown[]
}

const staging = new Map<string, Staging>()
let nextGeneration = 0

/** A replay started (`jsonl-status { replayComplete:false }`). Opens a new
 *  generation; an older unfinished one for the same tab is abandoned. Returns
 *  the generation the caller must hand back to `publishAgentReplay()`. */
export function beginAgentReplay(tabId: string): number {
  nextGeneration += 1
  staging.set(tabId, { generation: nextGeneration, entries: [] })
  return nextGeneration
}

export function isAgentReplayStaging(tabId: string): boolean {
  return staging.has(tabId)
}

/** Collect a replay batch (or a live entry that lands mid-replay — it belongs
 *  to the same transcript and is ordered with the rest at publish time).
 *  Returns false when no replay is open, so the caller parses live instead. */
export function stageAgentEntries(tabId: string, entries: unknown[]): boolean {
  const st = staging.get(tabId)
  if (!st) return false
  st.entries.push(...entries)
  return true
}

/** Build the snapshot from everything staged for `generation` and publish it in
 *  ONE signal update. A stale generation (a newer replay has started since) is
 *  dropped without touching the published tree. Returns true when published. */
export function publishAgentReplay(tabId: string, generation: number): boolean {
  const st = staging.get(tabId)
  if (!st || st.generation !== generation) return false
  staging.delete(tabId)
  // Replay chunks arrive newest-batch-first; the lifecycle parser is
  // order-sensitive (a notification before its Agent tool_use only leaves a
  // synthetic record), so order once by the transcript's own keys.
  const ordered = orderEntries(st.entries as Parameters<typeof orderEntries>[0])
  const tree: AgentTreeState = createAgentTree()
  parseAgentEntriesInto(tree, ordered)
  const next = new Map(tabAgentTrees.value)
  next.set(tabId, tree)
  tabAgentTrees.value = next
  return true
}

/** Forget an open replay (tab closed / state reset). */
export function abandonAgentReplay(tabId: string): void {
  staging.delete(tabId)
}

/** Test/diagnostic accessor: how many entries a tab has staged. */
export function stagedAgentEntryCount(tabId: string): number {
  return staging.get(tabId)?.entries.length ?? 0
}

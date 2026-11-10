// "Compacting conversation…" state, the way the CLI TUI shows a compaction.
// The server marks the stream of the CLI's compaction request (`__compaction`,
// no content: see server/src/core/proxy/compaction-request.ts), so the summary
// text never becomes an assistant answer or a ghost stub in the chat. The state
// opens with the first marker and closes with its final snapshot, or with the
// compact boundary / summary row when the final snapshot is lost (abort,
// reconnect).
import { signal } from '@preact/signals'
import type { JsonlEntryData } from '../types/jsonl'

export const compactingTabs = signal<ReadonlySet<string>>(new Set())

function set(tabId: string, on: boolean): void {
  if (compactingTabs.peek().has(tabId) === on) return
  const next = new Set(compactingTabs.peek())
  if (on) next.add(tabId)
  else next.delete(tabId)
  compactingTabs.value = next
}

/** Consume a compaction marker; false for every other streaming entry. */
export function noteCompactionStream(tabId: string, entry: JsonlEntryData): boolean {
  if (!entry.__compaction) return false
  set(tabId, entry.__streaming === true)
  return true
}

/** Canonical rows: the compaction result ends the state. */
export function noteCompactionRows(tabId: string, entries: readonly JsonlEntryData[]): void {
  if (!compactingTabs.peek().has(tabId)) return
  if (entries.some(row => (row.type === 'system' && row.subtype === 'compact_boundary') || row.isCompactSummary)) set(tabId, false)
}

export function forgetCompaction(tabId: string): void {
  set(tabId, false)
}

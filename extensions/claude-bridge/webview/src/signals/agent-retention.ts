import type { SubagentTileState } from './agents'

// Recent reader cache only. Full JSONL stays authoritative on the server and
// can be downloaded on demand; eviction must not imply transcript deletion.
export const AGENT_RETENTION = { entries: 600, slots: 64, slotBytes: 4 * 1024 * 1024, totalBytes: 32 * 1024 * 1024 }
const sizes = new WeakMap<object, number>()
const encoder = new TextEncoder()
function size(entry: unknown): number {
  if (entry && typeof entry === 'object') {
    const cached = sizes.get(entry)
    if (cached !== undefined) return cached
    const bytes = encoder.encode(JSON.stringify(entry)).length
    sizes.set(entry, bytes)
    return bytes
  }
  return encoder.encode(JSON.stringify(entry)).length
}

export function boundAgentTranscript(state: SubagentTileState): void {
  // File offsets when available, otherwise canonical timestamps. Sorting the
  // bounded merge prevents a replay of evicted older UUIDs displacing live tail.
  state.entries.sort((a, b) => {
    if (typeof a._pos === 'number' && typeof b._pos === 'number') return a._pos - b._pos
    if (a.timestamp && b.timestamp) return String(a.timestamp).localeCompare(String(b.timestamp))
    return 0
  })
  const unique = new Set<string>()
  const retained: any[] = []
  let bytes = 0
  for (let i = state.entries.length - 1; i >= 0; i--) {
    const entry = state.entries[i]
    if (entry.uuid && unique.has(entry.uuid)) continue
    const entryBytes = size(entry)
    if (entryBytes > AGENT_RETENTION.slotBytes) continue // available in full export
    if (retained.length >= AGENT_RETENTION.entries || bytes + entryBytes > AGENT_RETENTION.slotBytes) break
    retained.push(entry)
    bytes += entryBytes
    if (entry.uuid) unique.add(entry.uuid)
  }
  state.entries = retained.reverse()
  state.seen = unique
  state.retainedBytes = bytes
  state.touchedAt = ++touchSequence
}
let touchSequence = 0

/** Alias keys refer to the same slot; evict the entire slot, not one alias. */
export function boundAgentSlots(map: Map<string, SubagentTileState>): Map<string, SubagentTileState> {
  const states = [...new Set(map.values())].sort((a, b) => (b.touchedAt ?? 0) - (a.touchedAt ?? 0))
  const keep = new Set<SubagentTileState>()
  let bytes = 0
  for (const state of states) {
    const next = bytes + (state.retainedBytes ?? 0)
    if (keep.size >= AGENT_RETENTION.slots || next > AGENT_RETENTION.totalBytes) continue
    bytes = next
    keep.add(state)
  }
  return new Map([...map].filter(([, state]) => keep.has(state)))
}

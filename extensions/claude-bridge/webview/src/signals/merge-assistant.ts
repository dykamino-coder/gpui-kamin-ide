import type { JsonlEntryData } from '../types/jsonl'
import { orderEntries } from './order-entries'

/** A CLI message is persisted as one UUID-bearing row per content block.
 * Keep those source rows: reverse replay and older pages can deliver the
 * beginning AFTER the ending. Concatenating an already folded message loses
 * the block positions and makes that delivery order permanent. */
export function mergeAssistantEntry(target: JsonlEntryData, incoming: JsonlEntryData): JsonlEntryData {
  const parts = orderEntries([
    ...(target.__assistantParts ?? (target.__streaming === undefined ? [target] : [])),
    ...(incoming.__assistantParts ?? [incoming]),
  ])
  const first = parts[0]!
  const last = parts[parts.length - 1]!
  return {
    ...last,
    // The bubble starts at the FIRST block and ends at the LAST UUID. Keeping
    // the last block's internal parent would sever the conversation chain.
    parentUuid: first.parentUuid,
    _pos: first._pos,
    _ord: first._ord,
    timestamp: first.timestamp,
    message: {
      ...last.message!,
      content: parts.flatMap(part => Array.isArray(part.message?.content) ? part.message.content : []),
    },
    __streaming: undefined,
    __assistantParts: parts,
    __uuids: [...new Set([...(target.__uuids ?? (target.uuid ? [target.uuid] : [])),
      ...parts.flatMap(part => part.__uuids ?? (part.uuid ? [part.uuid] : []))])],
  }
}

/** Snapshot/segment loads need the same canonical folding as append/pages. */
export function foldAssistantEntries(entries: JsonlEntryData[]): JsonlEntryData[] {
  const out: JsonlEntryData[] = []
  const indices = new Map<string, number>()
  for (const entry of entries) {
    const id = entry.type === 'assistant' ? entry.message?.id : undefined
    const index = id ? indices.get(id) : undefined
    if (index !== undefined) out[index] = mergeAssistantEntry(out[index]!, entry)
    else {
      if (id) indices.set(id, out.length)
      out.push(entry)
    }
  }
  return out
}

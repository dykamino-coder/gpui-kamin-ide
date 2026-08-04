import type { JsonlEntryData, ContentBlock } from '../types/jsonl'

/** tool_use_id → the result that came back for it. */
export type ToolResultMap = Map<string, { content: string; isError: boolean }>

/** Fold a RANGE of entries into an existing map.
 *
 *  Rebuilding the whole map is O(history) AND re-creates the text of every tool
 *  result — on a 23k-entry session a heap sampler measured 200MB allocated in a
 *  single window doing exactly this, because a structure bump (every arriving
 *  entry) triggered a full rebuild. Appends can extend the map in place instead,
 *  which is what makes the cost proportional to what actually arrived. */
export function foldToolResultsInto(map: ToolResultMap, entries: JsonlEntryData[], from = 0): boolean {
  let mutated = false
  for (let i = from; i < entries.length; i++) {
    const entry = entries[i]
    if (!entry || entry.type !== 'user') continue
    const content = entry.message?.content
    if (!Array.isArray(content)) continue
    for (const block of content as ContentBlock[]) {
      if (block.type !== 'tool_result' || !block.tool_use_id) continue
      const text = typeof block.content === 'string'
        ? block.content
        : Array.isArray(block.content)
          ? (block.content as any[]).map((c: any) => c.text || JSON.stringify(c)).join('\n')
          : block.content != null ? JSON.stringify(block.content) : ''
      // Report whether anything actually changed — an OVERWRITE of an existing
      // id with different text leaves `map.size` unchanged, so a size diff can't
      // be the invalidation signal (it would leave the render's `sig`/`ver` stale
      // and keep serving the old tool-result bubble).
      const prev = map.get(block.tool_use_id)
      if (!prev || prev.content !== text || prev.isError !== (block.is_error === true)) {
        map.set(block.tool_use_id, { content: text, isError: block.is_error === true })
        mutated = true
      }
    }
  }
  return mutated
}

export function buildToolResultMap(entries: JsonlEntryData[]): ToolResultMap {
  const map: ToolResultMap = new Map()
  foldToolResultsInto(map, entries, 0)
  return map
}

/** The tool-result map spans the FULL history and holds the text of every tool
 *  result — on a session full of file reads and greps that is the single largest
 *  structure the chat keeps. It used to be built inside each segment model, so a
 *  session with several compact segments kept up to MAX_ENTRIES copies of the
 *  SAME data, and every structure bump rebuilt it from scratch. A heap sampler
 *  run on a real session showed it as the top named allocation site, tens of MB
 *  per window, with the shared renderer climbing past 4GB and the panels dying
 *  with OUT_OF_MEMORY in a loop.
 *
 *  One map per tab, rebuilt only when that tab's structure actually changes. */
interface ToolResultsEntry {
  structureVersion: number
  map: ToolResultMap | undefined
  /** Σ content length — the vnode-cache invalidation signal. Summed once here
   *  rather than walked per segment model. */
  sig: number
  /** How many entries are already folded in, and the identity of the last one.
   *  A new entries array whose prefix ends in that SAME object is an append, so
   *  only the tail needs folding. Sharing the map across segments removed the
   *  duplicate copies, but the rebuild remained O(history) per structure bump —
   *  which on a 23k-entry session was still 200MB of allocation per window. */
  builtUpTo: number
  lastEntry: JsonlEntryData | undefined
}

const toolResultsByTab = new Map<string, ToolResultsEntry>()
/** A couple of live tabs' worth; the rest rebuild on demand. Bounded because
 *  each entry can be tens of MB. */
const MAX_TOOL_RESULT_TABS = 3

/** Hard ceiling on the total tool-result TEXT held for one tab, in UTF-16 chars
 *  (~2 bytes each, so ~48MB). This map holds the full body of every tool result
 *  over the WHOLE history — on a marathon session full of file reads and greps
 *  it is the single largest structure the chat keeps, and with the active tab
 *  never evicted it grew until the shared renderer hit its ~4GB heap ceiling and
 *  died with OUT_OF_MEMORY. The chat only ever RENDERS the recent tail, so a
 *  scrolled-off old result is not on screen; evicting oldest-first keeps every
 *  visible result full and drops only history that isn't being shown. */
const MAX_TOOL_RESULT_CHARS = 24_000_000

/** Evict oldest map entries (Map preserves insertion = chronological order)
 *  until the total content length is within the cap. Returns the new total. */
function capToolResultMap(map: ToolResultMap, total: number): number {
  if (total <= MAX_TOOL_RESULT_CHARS) return total
  for (const [id, v] of map) {
    if (total <= MAX_TOOL_RESULT_CHARS) break
    total -= v.content.length
    map.delete(id)
  }
  return total
}

/** Index just past the last COMMITTED entry — i.e. length minus the trailing
 *  streaming stub(s). The tool-result map is a fold of USER tool_result entries;
 *  a live assistant stub (which carries no tool_result and is REPLACED by a fresh
 *  object on every content_block boundary) never contributes to it. Anchoring the
 *  append fast-path on the last COMMITTED entry instead of the literal last one is
 *  the whole fix: with the old anchor, every boundary swapped the tail object, the
 *  identity check failed, and the WHOLE map was rebuilt — on a tool-heavy session
 *  that is hundreds of MB of string re-allocation per boundary, i.e. the freeze. */
function committedCount(entries: JsonlEntryData[]): number {
  let n = entries.length
  while (n > 0 && entries[n - 1]!.__streaming !== undefined) n--
  return n
}

export function getToolResults(
  tabId: string,
  currentEntries: JsonlEntryData[],
  structureVersion: number,
): { map: ToolResultMap | undefined; sig: number } {
  const hit = toolResultsByTab.get(tabId)
  if (hit && hit.structureVersion === structureVersion) {
    toolResultsByTab.delete(tabId); toolResultsByTab.set(tabId, hit) // refresh LRU
    return hit
  }
  // Append fast path: committed entries are immutable, so if the array still
  // holds the SAME COMMITTED anchor at `builtUpTo - 1`, everything up to it is
  // unchanged and only the tail (new committed entries + skipped stubs) needs
  // folding. Anchoring on the last committed entry — not the literal last one —
  // is what makes a streaming boundary (which only swaps the trailing stub) reuse
  // the map instead of rebuilding it. A reorder/replay/prepend moves the anchor
  // object and correctly falls back to a full rebuild.
  if (hit?.map && hit.builtUpTo > 0 && currentEntries.length >= hit.builtUpTo
    && currentEntries[hit.builtUpTo - 1] === hit.lastEntry) {
    // Recompute the vnode-cache signal on ANY change, not just a size change:
    // an overwrite of an existing tool_use_id with different text (a retried
    // call, or a duplicate record with a fresh uuid) leaves the size the same
    // but changes the rendered bubble, so a size diff would leave `sig`/`ver`
    // stale and keep painting the old result.
    const changed = foldToolResultsInto(hit.map, currentEntries, hit.builtUpTo)
    if (changed) {
      let sig = 0
      hit.map.forEach((v) => { sig += v.content.length })
      hit.sig = capToolResultMap(hit.map, sig)
    }
    hit.structureVersion = structureVersion
    const committed = committedCount(currentEntries)
    hit.builtUpTo = committed
    hit.lastEntry = committed > 0 ? currentEntries[committed - 1] : undefined
    toolResultsByTab.delete(tabId); toolResultsByTab.set(tabId, hit)
    return hit
  }
  const map = currentEntries.length > 0 ? buildToolResultMap(currentEntries) : undefined
  let sig = 0
  map?.forEach((v) => { sig += v.content.length })
  if (map) sig = capToolResultMap(map, sig)
  const committed = committedCount(currentEntries)
  const entry: ToolResultsEntry = {
    structureVersion,
    map,
    sig,
    builtUpTo: committed,
    lastEntry: committed > 0 ? currentEntries[committed - 1] : undefined,
  }
  toolResultsByTab.delete(tabId)
  toolResultsByTab.set(tabId, entry)
  while (toolResultsByTab.size > MAX_TOOL_RESULT_TABS) {
    const oldest = toolResultsByTab.keys().next().value
    if (oldest === undefined) break
    toolResultsByTab.delete(oldest)
  }
  return entry
}

export function dropToolResults(tabId: string): void {
  toolResultsByTab.delete(tabId)
}

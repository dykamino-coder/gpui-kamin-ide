import { signal, computed } from '@preact/signals'
import { activeTabId } from './tabs'
import { jsonlEntriesByTab, getJsonlStructureVersion } from './jsonl'
import { segmentIndexByTab } from './all-segments'
import {
  isInvisibleAssistantEntry,
  isCaveatOnlyEntry,
  isToolResultEntry,
  mergeConsecutiveCliEntries,
  groupConsecutiveToolCalls,
  NON_RENDERING_ENTRY_TYPES,
  isVisibleAttachment,
} from '../components/jsonl-viewer/utils'
import type { JsonlEntryData, ContentBlock } from '../types/jsonl'

// Compact-segment tab state. Lifted out of JsonlViewer so the
// "Conversations" tab strip can be rendered up next to ChatHeader
// instead of inside the scrolling chat panel — keeps the active
// segment indicator always visible without `position: sticky`
// inside an overflow:auto container.

export interface CompactSegment {
  from: number
  to: number
  ts?: string
  /** Count of entries that actually render in the chat panel — excludes
   *  meta/caveat/snapshot/attachment/tool-result rows that the viewer
   *  hides. The raw `to - from` overcounts by ~10× because most JSONL
   *  rows are protocol noise. Used by the segment-tab pill so the
   *  badge matches what the user sees.*/
  visibleCount: number
}

function textOfUserEntry(e: JsonlEntryData): string {
  if (e.type !== 'user') return ''
  const c = e.message?.content
  if (typeof c === 'string') return c.trim()
  if (Array.isArray(c)) {
    for (const b of c as ContentBlock[]) {
      if (b.type === 'text' && typeof b.text === 'string') return b.text.trim()
    }
  }
  return ''
}

function isVisibleEntry(e: JsonlEntryData): boolean {
  // Bookkeeping the CLI emits per turn/tick. This used to be a hardcoded list
  // right here — a SECOND copy that had drifted from the canonical one in
  // utils.ts (it knew todo-snapshot/compact-summary/tool-call-error, but not
  // ai-title/summary), so the segment-tab count and the viewer disagreed about
  // what's visible. One set, imported.
  if (NON_RENDERING_ENTRY_TYPES.has(e.type)) return false
  if (e.type === 'attachment') return isVisibleAttachment(e)
  if (e.type === 'assistant' && isInvisibleAssistantEntry(e)) return false
  if (e.type === 'user') {
    if ((e as any).isMeta) return false
    if (isCaveatOnlyEntry(e)) return false
    if (isToolResultEntry(e)) return false
    const t = textOfUserEntry(e)
    if (t === '[Request interrupted by user]'
      || t === '[Request interrupted by user for tool use]'
      || t === '[Skipped by user]') return false
  }
  if (e.type === 'system') {
    const sub = (e as any).subtype
    // Compact_boundary renders as a divider — count it. api_error retries
    // and bare system entries without messages don't render.
    if (sub === 'api_error') return false
    if (!(e as any).message && sub !== 'compact_boundary') return false
  }
  return true
}

/** Count entries that would render as visible bubbles after the same
 *  merge + group pipeline JsonlViewer applies. Without grouping, a tool
 *  burst of 30 tool_use assistants would inflate the count to 30 even
 *  though the viewer collapses them to ONE bubble. */
function countVisibleBubbles(slice: JsonlEntryData[]): number {
  const processed = groupConsecutiveToolCalls(mergeConsecutiveCliEntries(slice))
  let n = 0
  for (const e of processed) {
    // tool_group is the synthetic wrapper from groupConsecutiveToolCalls
    // — always counts as one visible bubble regardless of how many
    // tool_use entries it wraps.
    if ((e as any).type === 'tool_group') { n++; continue }
    if (isVisibleEntry(e)) n++
  }
  return n
}

// Memo of the last computed segment list per tab, keyed on the SAME per-tab
// `structureVersion` the render-model cache uses (see derived-cache.ts).
//
// WHY: this computed reads `activeTabId.value` AND `jsonlEntriesByTab.value`, so
// it was invalidated by BOTH a session switch and every streaming flush — and it
// rebuilds `visibleCount` by running merge+group over EVERY segment, i.e. the
// WHOLE session, purely to number the segment tab pills. Measured on a real
// 90,345-entry session: ~120ms warm / 210-922ms cold per switch (the active
// segment is ~2% of it — 98% of the work was thrown away), and ~187ms per
// stream flush at up to 60 flushes/sec on a compacted session.
//
// That is exactly the reported asymmetry: `compact-segments` is imported ONLY by
// the chat (JsonlViewer / ConversationSegmentTabs / SessionStats) — the Bridge
// Console never touches it, which is why the console switched instantly while
// the chat hung for seconds. The 0.2.42 render-model cache memoized
// `computeModel` but left THIS uncovered, so the freeze fix was incomplete.
//
// Invariant (same as derived-cache): committed entries are immutable and every
// structural mutation bumps `structureVersion`; a streaming TEXT delta does not,
// and cannot change segment boundaries or visible counts.
// Self-bounded (insertion-order LRU) rather than evicted from clearJsonlEntries:
// exporting a drop() for jsonl.ts to call would make jsonl ↔ compact-segments a
// module CYCLE, and this file's `computed` runs at module scope — a TDZ waiting
// to happen. A segment list is tiny (a few {from,to,ts,count} per session), so a
// handful of stale ones cost nothing; the whole point of the eviction next door
// (derived-cache) is freeing MULTI-MB render models, which this isn't.
const SEG_CACHE_MAX = 8
interface SegCacheEntry {
  version: number
  segs: CompactSegment[]
  /** Incremental-extend proof (audit #70 B1): entries.length at compute time,
   *  plus OBJECT IDENTITIES of the first entry and every boundary entry at
   *  their cached indices. Committed entries are immutable, so if all of these
   *  still match, the prefix layout is untouched and only the tail (after the
   *  last boundary) needs recounting. Any prepend / trim / mid-replace breaks
   *  an identity or shifts an index → full recompute. */
  lenAt: number
  firstRef: JsonlEntryData | undefined
  boundaryIdx: number[]
  boundaryRefs: JsonlEntryData[]
}
const segCache = new Map<string, SegCacheEntry>()

function rememberSegs(id: string, entry: SegCacheEntry): CompactSegment[] {
  segCache.delete(id) // re-insert at the newest position (Map keeps insertion order)
  segCache.set(id, entry)
  while (segCache.size > SEG_CACHE_MAX) {
    const oldest = segCache.keys().next().value
    if (oldest === undefined) break
    segCache.delete(oldest)
  }
  return entry.segs
}

/** Prefix provably unchanged since `hit` was computed? */
function canExtend(hit: SegCacheEntry, entries: JsonlEntryData[]): boolean {
  if (hit.boundaryIdx.length === 0) return false
  if (entries.length < hit.lenAt) return false
  if (entries[0] !== hit.firstRef) return false
  for (let i = 0; i < hit.boundaryIdx.length; i++) {
    if (entries[hit.boundaryIdx[i]] !== hit.boundaryRefs[i]) return false
  }
  return true
}

/** All compact-boundary-delimited segments for the active tab. Empty
 *  array when the session has no compactions yet. */
export const compactSegments = computed<CompactSegment[]>(() => {
  const id = activeTabId.value
  if (!id) return []
  // Read the signal to stay subscribed (O(1) Map.get) — the memo below is what
  // makes the flush/switch cheap, not skipping the subscription.
  const entries = jsonlEntriesByTab.value.get(id) ?? []
  if (entries.length === 0) return []
  const version = getJsonlStructureVersion(id)
  const hit = segCache.get(id)
  if (hit && hit.version === version) return hit.segs
  const isBoundary = (e: JsonlEntryData | undefined): boolean =>
    e?.type === 'system' && (e as any).subtype === 'compact_boundary'
  function countVisible(from: number, to: number): number {
    return countVisibleBubbles(entries.slice(from, to))
  }

  // Incremental extend (B1): a typical version bump is a tail append. If the
  // prefix layout is provably unchanged (identity checks in canExtend), reuse
  // every segment BEFORE the last boundary and recount only from it — the
  // post-compact tail is ~2% of a long session, so a batch costs O(tail)
  // instead of merge+group over the WHOLE store.
  if (hit && canExtend(hit, entries)) {
    const lastB = hit.boundaryIdx[hit.boundaryIdx.length - 1]!
    const newBoundaries: number[] = []
    for (let i = lastB + 1; i < entries.length; i++) {
      if (isBoundary(entries[i])) newBoundaries.push(i)
    }
    const out = hit.segs.slice(0, hit.segs.length - 1)
    let prev = lastB
    for (const b of newBoundaries) {
      out.push({ from: prev, to: b, ts: entries[b]?.timestamp, visibleCount: countVisible(prev, b) })
      prev = b
    }
    out.push({ from: prev, to: entries.length, visibleCount: countVisible(prev, entries.length) })
    const idx = hit.boundaryIdx.concat(newBoundaries)
    return rememberSegs(id, {
      version,
      segs: out,
      lenAt: entries.length,
      firstRef: entries[0],
      boundaryIdx: idx,
      boundaryRefs: idx.map((i) => entries[i]!),
    })
  }

  const boundaries: number[] = []
  for (let i = 0; i < entries.length; i++) {
    if (isBoundary(entries[i])) boundaries.push(i)
  }
  // Cache the no-compaction answer too: an uncompacted session pays the O(n)
  // boundary scan on EVERY stream flush otherwise, and that's the common case.
  if (boundaries.length === 0) {
    return rememberSegs(id, {
      version,
      segs: [],
      lenAt: entries.length,
      firstRef: entries[0],
      boundaryIdx: [],
      boundaryRefs: [],
    })
  }
  const out: CompactSegment[] = []
  let prev = 0
  for (const b of boundaries) {
    out.push({ from: prev, to: b, ts: entries[b]?.timestamp, visibleCount: countVisible(prev, b) })
    prev = b
  }
  out.push({ from: prev, to: entries.length, visibleCount: countVisible(prev, entries.length) })
  return rememberSegs(id, {
    version,
    segs: out,
    lenAt: entries.length,
    firstRef: entries[0],
    boundaryIdx: boundaries,
    boundaryRefs: boundaries.map((i) => entries[i]!),
  })
})

// The user's EXPLICIT segment pick, per tab, keyed by the segment's boundary
// timestamp — NOT by its index.
//
// WHY not the index: history reverse-loads (the current segment first, older
// ones streamed in behind it), so older segments are PREPENDED and every index
// shifts under us. A stored "2" silently came to mean a different conversation
// once a fourth segment arrived — the user was reading segment 2 of 3 and got
// dropped into segment 2 of 4. A boundary ts identifies the same conversation
// no matter how much history lands later.
//
// `undefined` = follow the latest. That is the DEFAULT and it is computed at
// render time, so a still-loading session always shows the END of the chat and
// cannot be yanked backwards by history arriving behind it.
export const activeSegmentTsByTab = signal<Map<string, string>>(new Map())

export const activeSegmentIdx = computed<number>(() => {
  const id = activeTabId.value
  const segs = compactSegments.value
  if (!id || segs.length === 0) return 0
  const ts = activeSegmentTsByTab.value.get(id)
  if (ts !== undefined) {
    const i = segs.findIndex((s) => s.ts === ts)
    if (i >= 0) return i // still present → honour the user's pick
    // Its boundary is gone (compaction rewrote the file) — fall through to the
    // latest rather than showing an arbitrary neighbour.
  }
  return segs.length - 1 // default: the newest conversation, always
})

// ── Full segment index (mirror index when loaded, else resident window) ─────
// The strip renders THIS. `fromTs`/`toTs` are the TIMESTAMP range for loadSegment
// (empty toTs = to the newest); `residentIdx` maps back into `compactSegments`
// for the in-window pick path. `count` is the segment's record count.
export interface DisplaySegment {
  key: string
  ts?: string
  fromTs: string
  toTs: string
  count?: number
  resident: boolean
  isLatest: boolean
  residentIdx: number
}

export const displaySegments = computed<DisplaySegment[]>(() => {
  const id = activeTabId.value
  const resident = compactSegments.value
  const index = id ? segmentIndexByTab.value.get(id) : undefined

  // No mirror index yet (first moment before it loads) → resident view. Its
  // `visibleCount` is bubbles, not raw records, but it's all we have until the
  // index lands, and it's correct for the live window.
  if (!index || index.boundaries.length === 0) {
    return resident.map((s, i) => ({
      key: s.ts ?? `r${i}`, ts: s.ts, fromTs: '', toTs: '',
      count: s.visibleCount, resident: true,
      isLatest: i === resident.length - 1, residentIdx: i,
    }))
  }

  // The mirror index — CHRONOLOGICAL by timestamp (the only reliable axis; the
  // mirror's byte offsets reset per file across resumes) with per-segment record
  // counts. counts[0] = the original pre-compaction conversation, counts[j] = the
  // segment starting at boundaries[j-1]. Each segment spans [its start ts, the
  // next boundary ts). Every non-latest segment loads from the mirror by ts (its
  // `fromTs`/`toTs`) — NOT matched back to a resident index. The resident segment
  // ts marks the boundary that ENDS a segment, while here it marks the one that
  // STARTS it, so matching by ts is off by one and would show the wrong dialog.
  const b = [...index.boundaries].sort((x, y) => x.ts.localeCompare(y.ts))
  const counts = index.counts
  const n = b.length // n >= 1 here (empty index took the resident branch above)
  const out: DisplaySegment[] = []
  // seg 0 — the original conversation before the first compaction (no boundary).
  out.push({ key: 'seg0', ts: undefined, fromTs: '', toTs: b[0]!.ts, count: counts[0], resident: false, isLatest: false, residentIdx: -1 })
  for (let k = 0; k < n; k++) {
    const ts = b[k]!.ts
    out.push({
      key: ts, ts,
      fromTs: ts, toTs: k + 1 < n ? b[k + 1]!.ts : '', // empty = to the newest
      count: counts[k + 1],
      resident: false, isLatest: k === n - 1, residentIdx: -1,
    })
  }
  return out
})

/** User clicked a segment pill. Picking the LAST one means "follow the latest"
 *  — stored as a clear, so a later compaction keeps them on Current instead of
 *  pinning them to what just became an old conversation. */
export function setActiveSegment(idx: number): void {
  const id = activeTabId.value
  if (!id) return
  const segs = compactSegments.value
  const next = new Map(activeSegmentTsByTab.value)
  const ts = idx >= 0 && idx < segs.length - 1 ? segs[idx]?.ts : undefined
  if (ts === undefined) next.delete(id)
  else next.set(id, ts)
  activeSegmentTsByTab.value = next
}


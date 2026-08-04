// Per-tab cache of the WHOLE chat render model — the heavy merge/group/chain +
// tool-result map PLUS the visibility pass (uuids-in-range, interrupt/skip flags,
// chain-membership filter). WHY a module cache keyed on a STRUCTURE version:
//
// A live session fires ~30 streaming text deltas/sec. Each used to reassign the
// entries signal with a fresh array ref, and this whole pipeline re-ran over the
// ENTIRE active compact segment every time — on a 13k-entry session that O(segment)
// recompute per frame stalled the shared renderer thread for minutes while the
// underlying JSONL kept advancing (the huge-session "chat froze mid-answer" bug).
//
// Key insight: a streaming TEXT delta changes NOTHING structural — same entries,
// same order, same message.ids, same tool results, same chain. Only a streaming
// stub's text grows. So the entire model is INVARIANT across a text delta; only
// the live stub OBJECTS need re-pointing (the per-rAF flush clones them). We key
// the cache on a per-tab `structureVersion` (bumped by signals/jsonl.ts only on a
// real shape change — add/remove/reorder/merge). While it's unchanged, a stream
// flush reuses the cached model and patches the tail stub objects in O(tail).
// A structural change (a few times/sec at most — batch append) recomputes fully.
//
// Correctness rests on: (1) committed entries are immutable (only stubs mutate,
// in place, text-only); (2) every structural mutation bumps the version (audited
// in signals/jsonl.ts); (3) the stub patch is provably identical to a full
// recompute when structure is unchanged (see derived-cache.test.ts).

import type { JsonlEntryData } from '../../types/jsonl'
import {
  mergeAssistantEntriesByMessageId,
  mergeConsecutiveCliEntries,
  groupConsecutiveToolCalls,
  NON_RENDERING_ENTRY_TYPES,
} from './utils'
import { getToolResults, dropToolResults, type ToolResultMap } from '../../lib/tool-result-cache'
import { computeChainUuids } from './compute-chain-uuids'
import {
  buildUuidsInRange,
  buildInterruptSkipFlags,
  entryIsVisible,
  isInterruptUserEntry,
  isSkippedUserEntry,
  type CompactSegmentRange,
} from './prepare-jsonl-entries'

export interface RenderModel {
  /** Full merged/grouped active-segment list (pre-visibility). */
  merged: JsonlEntryData[]
  /** tool_use_id → result, over full history. */
  toolResults: ToolResultMap | undefined
  /** Merged list after the visibility + interrupt/skip pass — what the viewer
   *  windows (renderCap) and mounts. */
  visibleMerged: JsonlEntryData[]
  /** vnode-cache invalidation key: tool-result text sum + interrupt/skip counts. */
  ver: string
}

interface CacheEntry {
  structureVersion: number
  segFrom: number | undefined
  segTo: number | undefined
  model: RenderModel
}

// Keyed by `${tabId}:${segIdx}` so flipping between a session's compact-segment
// tabs is ALSO cached (each segment's model is structurally stable once past) —
// not just alternating whole sessions. Map insertion order → cheap LRU (delete+
// set on hit moves to newest). Bounded across a few tabs × a few segments each.
const MAX_ENTRIES = 8
const cache = new Map<string, CacheEntry>()
const keyOf = (tabId: string, segIdx: number): string => `${tabId}:${String(segIdx)}`

/** Evict every cached segment-model for a tab — call when its JSONL is cleared
 *  so a closed/compacted huge session's multi-MB model isn't retained (mirrors
 *  the window.__jsonlEntries eviction in signals/jsonl.ts). */
export function dropRenderModelCache(tabId: string): void {
  const prefix = `${tabId}:`
  for (const k of [...cache.keys()]) if (k.startsWith(prefix)) cache.delete(k)
  dropToolResults(tabId) // the heaviest part of the model — drop it too
}

function computeModel(
  currentEntries: JsonlEntryData[],
  segs: CompactSegmentRange[],
  segIdx: number,
  tools: { map: ToolResultMap | undefined; sig: number },
): RenderModel {
  const range = segs[segIdx]
  const segFrom = range?.from
  const segTo = range?.to
  const scopedEntries = segFrom != null && segTo != null
    ? currentEntries.slice(segFrom, segTo)
    : currentEntries
  const merged = scopedEntries.length > 0
    ? groupConsecutiveToolCalls(mergeConsecutiveCliEntries(mergeAssistantEntriesByMessageId(scopedEntries)))
    : []
  // toolResults stays over the FULL history — a tool_use in the active segment
  // can reference a tool_result outside it (looked up by tool_use_id). Shared
  // per tab (see getToolResults): every segment model referencing the SAME map
  // instead of building its own is what stopped the duplication.
  const toolResults = tools.map
  const chainUuids = computeChainUuids(scopedEntries)

  const isLatestSegment = segs.length === 0 || segIdx === segs.length - 1
  const uuidsInRange = buildUuidsInRange(currentEntries, segs, segIdx)
  const visCtx = { uuidsInRange, range, entries: currentEntries, toolResults, isLatestSegment, chainUuids }
  const { interruptFlags, skippedFlags } = buildInterruptSkipFlags(merged)

  // Keep the in-flight turn (last assistant → end) always visible so the chain
  // walk can't drop the live answer during the stub→canonical uuid hand-off.
  // recentTip membership BYPASSES entryIsVisible below, so it must never take
  // in bookkeeping rows: the CLI re-emits ai-title/permission-mode after every
  // turn, and a session that ended on a burst of them (~260 in a row) forced all
  // of them "visible", filling the whole render window with rows that render
  // null — a blank chat with the real turns pushed behind "N earlier messages".
  const recentTip = new Set<JsonlEntryData>()
  if (isLatestSegment) {
    let li = -1
    for (let i = merged.length - 1; i >= 0; i--) { if (merged[i]?.type === 'assistant') { li = i; break } }
    for (let i = Math.max(0, li >= 0 ? li : merged.length - 1); i < merged.length; i++) {
      const e = merged[i]!
      if (!NON_RENDERING_ENTRY_TYPES.has(e.type)) recentTip.add(e)
    }
  }
  const visibleMerged: JsonlEntryData[] = merged
    .filter(e => !isInterruptUserEntry(e) && !isSkippedUserEntry(e))
    .filter(e => recentTip.has(e) || entryIsVisible(e, visCtx))
    .map(e => {
      const wantInt = interruptFlags.has(e)
      const wantSkip = skippedFlags.has(e)
      if (!wantInt && !wantSkip) return e
      return { ...e, interruptedByUser: wantInt || undefined, skippedByUser: wantSkip || undefined } as JsonlEntryData
    })

  const ver = `${tools.sig}|${interruptFlags.size}|${skippedFlags.size}`

  return { merged, toolResults, visibleMerged, ver }
}

// Collect the CURRENT live-stub objects (message.id → entry) by scanning the tail
// of the entries array — a streaming stub is always the in-flight answer near the
// end. Bounded so this stays O(tail), never O(history).
// A live stub always sorts to the very end of the entries array (order-entries.ts
// puts entries without an `_ord` last), so it sits within this tail window. The
// bound keeps the scan O(tail); the fail-safe in getRenderModel covers the day
// that invariant ever changes (a buried stub → force a full recompute, never
// silently serve frozen text).
const STUB_SCAN_TAIL = 64
function collectStubs(entries: JsonlEntryData[]): Map<string, JsonlEntryData> {
  const stubs = new Map<string, JsonlEntryData>()
  for (let i = entries.length - 1, seen = 0; i >= 0 && seen < STUB_SCAN_TAIL; i--, seen++) {
    const e = entries[i]
    const id = e?.message?.id
    if (e && e.__streaming !== undefined && id) stubs.set(id, e)
  }
  return stubs
}

// Re-point the cached model's stub entries to the current (grown-text) objects.
// A stream flush clones the stub into a new object each frame, so the cached
// model — captured at the last structural compute — otherwise holds a frozen-text
// stub. Streaming stubs are never merged/grouped (they're pushed standalone), so
// each is a top-level array element; scan the tail (O(tail)) and swap in place.
// Returns false if any CURRENT stub id wasn't found in the model's tail — the
// cache is then structurally stale for it (bound exceeded / merged), so the
// caller must recompute rather than serve possibly-frozen text.
function patchStubs(model: RenderModel, stubs: Map<string, JsonlEntryData>): boolean {
  if (stubs.size === 0) return true
  const patched = new Set<string>()
  const patch = (arr: JsonlEntryData[]): void => {
    for (let i = arr.length - 1, seen = 0; i >= 0 && seen < STUB_SCAN_TAIL; i--, seen++) {
      const e = arr[i]
      const id = e?.message?.id
      if (e && e.__streaming !== undefined && id) {
        const cur = stubs.get(id)
        if (cur) { if (cur !== e) arr[i] = cur; patched.add(id) }
      }
    }
  }
  patch(model.merged)
  patch(model.visibleMerged)
  for (const id of stubs.keys()) if (!patched.has(id)) return false
  return true
}

/** The whole chat render model for a tab, memoized by (tabId, structureVersion,
 *  segment). A stream-only flush (same structureVersion, new entries ref) is an
 *  O(tail) stub re-point; a structural change recomputes. */
export function getRenderModel(
  tabId: string,
  currentEntries: JsonlEntryData[],
  segs: CompactSegmentRange[],
  segIdx: number,
  structureVersion: number,
): RenderModel {
  const range = segs[segIdx]
  const segFrom = range?.from
  const segTo = range?.to
  const key = keyOf(tabId, segIdx)
  const hit = cache.get(key)
  if (hit && hit.structureVersion === structureVersion
    && hit.segFrom === segFrom && hit.segTo === segTo) {
    // Structure unchanged — only live stub text can have grown. Re-point the
    // stub objects and return the cached (heavy) model. patchStubs returns false
    // only if a current stub escaped the tail window (bound/merge invariant broke)
    // — then fall through to a full recompute rather than serve frozen text.
    if (patchStubs(hit.model, collectStubs(currentEntries))) {
      cache.delete(key); cache.set(key, hit) // refresh LRU
      return hit.model
    }
  }
  const model = computeModel(currentEntries, segs, segIdx, getToolResults(tabId, currentEntries, structureVersion))
  cache.delete(key) // re-insert at the newest (last) Map position for LRU
  cache.set(key, { structureVersion, segFrom, segTo, model })
  while (cache.size > MAX_ENTRIES) {
    const oldest = cache.keys().next().value
    if (oldest === undefined) break
    cache.delete(oldest)
  }
  return model
}

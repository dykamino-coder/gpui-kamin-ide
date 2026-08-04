import { signal } from '@preact/signals'

// The FULL compact-segment index for a tab, read from the disk mirror (the
// complete transcript) — every segment INCLUDING ones the renderer trimmed out
// of its resident window, keyed by TIMESTAMP with per-segment record counts.
// `compactSegments` can only ever see the boundaries inside the window; this is
// what lets the strip show them all with counts and load an out-of-window one.
// `counts` has one more entry than `boundaries`: counts[0] = the original
// pre-compaction conversation, counts[j] = the segment starting at boundaries[j-1].
export interface SegmentIndex {
  boundaries: { ts: string }[]
  counts: number[]
}

export const segmentIndexByTab = signal<Map<string, SegmentIndex>>(new Map())

export function setSegmentIndex(tabId: string, index: SegmentIndex): void {
  const next = new Map(segmentIndexByTab.value)
  next.set(tabId, index)
  segmentIndexByTab.value = next
}

export function dropSegmentIndex(tabId: string): void {
  if (!segmentIndexByTab.value.has(tabId)) return
  const next = new Map(segmentIndexByTab.value)
  next.delete(tabId)
  segmentIndexByTab.value = next
}

// Segment selection must survive reverse-load.
//
// THE BUG: history loads newest-first — the current segment arrives, then older
// ones are PREPENDED behind it. Selection was stored as an INDEX, so every
// arriving segment shifted what that index meant: a stored "0" started as
// "current" and silently became "the oldest conversation". Worse, the index was
// pinned by a useEffect AFTER the commit, so the render that already used the
// stale value painted the wrong conversation for a frame — on every batch.
// Symptom: "the chat jumps to the previous conversation while loading".
//
// Run: from the kamin-ide repo root →  npx tsx <abs path to this file>

import { compactSegments, activeSegmentIdx, setActiveSegment, activeSegmentTsByTab } from './compact-segments.ts'
import { activeTabId } from './tabs.ts'
import { appendJsonlEntries, clearJsonlEntries } from './jsonl.ts'
import type { JsonlEntryData } from '../types/jsonl'

let failures = 0
function assert(cond: boolean, msg: string): void {
  if (!cond) { failures++; console.error('  ✗ ' + msg) }
}

let uid = 0
// `_ord` is the server's file-position tag; orderEntries sorts on it, which is
// exactly how a later-arriving OLDER batch gets placed BEFORE what's on screen.
// Without it these builders would just append and the test would prove nothing.
const user = (t: string, ord: number): JsonlEntryData =>
  ({ type: 'user', uuid: `u${uid++}`, _ord: ord, message: { role: 'user', content: [{ type: 'text', text: t }] } }) as never
const asst = (t: string, ord: number): JsonlEntryData =>
  ({ type: 'assistant', uuid: `a${uid++}`, _ord: ord, message: { id: `m${uid}`, role: 'assistant', content: [{ type: 'text', text: t }] } }) as never
const boundary = (ts: string, ord: number): JsonlEntryData =>
  ({ type: 'system', subtype: 'compact_boundary', uuid: `b${uid++}`, _ord: ord, timestamp: ts }) as never

/** One conversation = a boundary + a couple of turns, at file position `ord`. */
const convo = (ts: string, label: string, ord: number): JsonlEntryData[] =>
  [boundary(ts, ord), user(`q ${label}`, ord + 1), asst(`a ${label}`, ord + 2)]

// ── Scenario 1: history landing behind must NOT move the view ───────────────
console.log('Scenario 1: reverse-load keeps you on the newest conversation')
{
  clearJsonlEntries('t1')
  activeTabId.value = 't1'
  // The CURRENT conversation lands first (that's how reverse-load works).
  appendJsonlEntries('t1', [...convo('2026-07-17T03:00:00Z', 'newest', 300)])
  const first = activeSegmentIdx.value
  const segsAfterFirst = compactSegments.value
  assert(first === segsAfterFirst.length - 1, 'S1 opens on the LAST segment')

  // Older history streams in behind it, one batch at a time.
  appendJsonlEntries('t1', [...convo('2026-07-17T01:00:00Z', 'older', 200)])
  appendJsonlEntries('t1', [...convo('2026-07-17T00:00:00Z', 'oldest', 100)])

  const segs = compactSegments.value
  assert(segs.length >= 3, `S1 segments accumulated (got ${String(segs.length)})`)
  // The view must STILL be the newest — not dragged back by the arrivals.
  assert(activeSegmentIdx.value === segs.length - 1,
    `S1 still on the newest after history arrives (idx ${String(activeSegmentIdx.value)} of ${String(segs.length)})`)
}

// ── Scenario 2: an explicit pick follows its conversation, not its index ────
console.log('Scenario 2: a user pick survives a prepend')
{
  clearJsonlEntries('t2')
  activeTabId.value = 't2'
  activeSegmentTsByTab.value = new Map()
  appendJsonlEntries('t2', [...convo('2026-07-17T02:00:00Z', 'B', 200), ...convo('2026-07-17T03:00:00Z', 'C', 300)])
  let segs = compactSegments.value
  // Pick the FIRST (an older one) — index 0 today.
  setActiveSegment(0)
  const pickedTs = segs[0]?.ts
  assert(activeSegmentIdx.value === 0, 'S2 pick honoured')

  // Now older history arrives and shifts every index by one.
  appendJsonlEntries('t2', [...convo('2026-07-17T00:00:00Z', 'A', 100)])
  segs = compactSegments.value
  const idx = activeSegmentIdx.value
  assert(segs[idx]?.ts === pickedTs,
    `S2 the pick still points at the SAME conversation (ts ${String(segs[idx]?.ts)} vs ${String(pickedTs)})`)
  assert(idx !== 0, 'S2 …and its index actually moved (proving index-keying would have broken)')
}

// ── Scenario 3: picking the newest = "follow latest", not a pin ─────────────
console.log('Scenario 3: choosing Current keeps following Current')
{
  clearJsonlEntries('t3')
  activeTabId.value = 't3'
  activeSegmentTsByTab.value = new Map()
  appendJsonlEntries('t3', [...convo('2026-07-17T01:00:00Z', 'A', 100), ...convo('2026-07-17T02:00:00Z', 'B', 200)])
  const segs = compactSegments.value
  setActiveSegment(segs.length - 1) // click "Current"
  assert(!activeSegmentTsByTab.value.has('t3'), 'S3 picking the last stores NO pin (it means "follow")')

  // A new compaction arrives → the user must ride it to the new Current.
  appendJsonlEntries('t3', [...convo('2026-07-17T03:00:00Z', 'C', 300)])
  const after = compactSegments.value
  assert(activeSegmentIdx.value === after.length - 1, 'S3 followed the new compaction to Current')
}

console.log('')
if (failures === 0) console.log('✅ ALL SEGMENT-SELECTION CHECKS PASSED')
else { console.error(`❌ ${failures} check(s) FAILED`); process.exit(1) }

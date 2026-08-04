// Perf + correctness test for the compactSegments memo.
//
// THE BUG IT PINS: `compactSegments` is a `computed` reading BOTH `activeTabId`
// and `jsonlEntriesByTab`, and it rebuilt `visibleCount` by running merge+group
// over EVERY segment — the whole session — just to number the segment tab pills.
// So it re-ran on every session switch AND on every streaming flush (which
// reassigns the entries map ref per rAF). Only the chat imports it, which is why
// the Bridge Console switched instantly while the chat hung for seconds.
//
// Run: from the kamin-ide repo root →  npx tsx <abs path to this file>

import { compactSegments } from './compact-segments.ts'
import { activeTabId } from './tabs.ts'
import { jsonlEntriesByTab, appendJsonlEntries, applyStreamingEntry } from './jsonl.ts'
import type { JsonlEntryData } from '../types/jsonl'

let failures = 0
function assert(cond: boolean, msg: string): void {
  if (!cond) { failures++; console.error('  ✗ ' + msg) }
}

let uid = 0
const user = (t: string): JsonlEntryData =>
  ({ type: 'user', uuid: `u${uid++}`, message: { role: 'user', content: [{ type: 'text', text: t }] } }) as never
const asst = (t: string): JsonlEntryData =>
  ({ type: 'assistant', uuid: `a${uid++}`, message: { id: `m${uid}`, role: 'assistant', content: [{ type: 'text', text: t }] } }) as never
const boundary = (): JsonlEntryData =>
  ({ type: 'system', subtype: 'compact_boundary', uuid: `b${uid++}`, timestamp: '2026-07-17T00:00:00Z' }) as never

/** A session with `turns` turns and a compact boundary every `every` turns. */
function build(turns: number, every: number): JsonlEntryData[] {
  const out: JsonlEntryData[] = []
  for (let i = 0; i < turns; i++) {
    out.push(user(`question ${i} with a realistic amount of prose in it`))
    out.push(asst(`answer ${i} `.repeat(20)))
    if (i > 0 && i % every === 0) out.push(boundary())
  }
  return out
}

// ── Scenario 1: a switch between two sessions must not recompute ────────────
console.log('Scenario 1: session switch is memoized')
{
  const A = build(4000, 400) // ~8k entries, 9 boundaries
  const B = build(4000, 400)
  appendJsonlEntries('tabA', A)
  appendJsonlEntries('tabB', B)

  activeTabId.value = 'tabA'
  const first = compactSegments.value // cold compute for A
  assert(first.length > 1, `S1 A has segments (got ${String(first.length)})`)

  activeTabId.value = 'tabB'
  compactSegments.value // cold compute for B

  const t0 = performance.now()
  activeTabId.value = 'tabA'
  const back = compactSegments.value
  const switchMs = performance.now() - t0

  assert(back === first, 'S1 switch-back returns the SAME cached array (no recompute)')
  console.log(`  switch-back: ${switchMs.toFixed(2)}ms`)
  assert(switchMs < 5, `S1 switch-back must be ~free (got ${switchMs.toFixed(1)}ms)`)
}

// ── Scenario 2: a streaming flush must not recompute ────────────────────────
// This is the one that fires up to 60×/sec on a compacted session.
console.log('Scenario 2: streaming flush is memoized')
{
  activeTabId.value = 'tabA'
  const before = compactSegments.value
  const t0 = performance.now()
  const N = 60
  for (let i = 0; i < N; i++) {
    // What scheduleStreamFlush does: reassign the map ref with grown stub text.
    const cur = jsonlEntriesByTab.value.get('tabA') ?? []
    const next = new Map(jsonlEntriesByTab.value)
    next.set('tabA', [...cur])
    jsonlEntriesByTab.value = next
    compactSegments.value // what JsonlViewer's render reads
  }
  const perFlush = (performance.now() - t0) / N
  console.log(`  ${String(N)} flushes: ${perFlush.toFixed(3)}ms each`)
  assert(compactSegments.value === before, 'S2 flush returns the SAME cached array')
  assert(perFlush < 1, `S2 a flush must be ~free (got ${perFlush.toFixed(2)}ms)`)
}

// ── Scenario 3: a STRUCTURAL change must still recompute (correctness) ──────
console.log('Scenario 3: a real append invalidates')
{
  activeTabId.value = 'tabA'
  const before = compactSegments.value
  const lastBefore = before[before.length - 1]!
  appendJsonlEntries('tabA', [user('a brand new turn'), asst('and its answer')])
  const after = compactSegments.value
  assert(after !== before, 'S3 append produced a FRESH segment list (not the stale cache)')
  assert(after[after.length - 1]!.to > lastBefore.to, 'S3 the active segment grew')
  assert(after[after.length - 1]!.visibleCount > lastBefore.visibleCount, 'S3 visibleCount grew')
}

// ── Scenario 4: a new boundary must appear (correctness) ────────────────────
console.log('Scenario 4: a new compact boundary invalidates')
{
  activeTabId.value = 'tabA'
  const before = compactSegments.value.length
  appendJsonlEntries('tabA', [boundary(), user('post-compact'), asst('reply')])
  assert(compactSegments.value.length === before + 1, `S4 segment count grew ${String(before)} → ${String(compactSegments.value.length)}`)
}

// ── Scenario 5: an unmemoized session pays the real price (the baseline) ────
// Proves the numbers this memo is defending against are real, not theoretical.
console.log('Scenario 5: cold compute cost (the thing being avoided)')
{
  const big = build(12000, 400) // ~24k entries, 29 boundaries
  appendJsonlEntries('tabBig', big)
  activeTabId.value = 'tabBig'
  const t0 = performance.now()
  const segs = compactSegments.value
  const coldMs = performance.now() - t0
  console.log(`  ${String(jsonlEntriesByTab.value.get('tabBig')?.length ?? 0)} entries, ${String(segs.length)} segments → cold ${coldMs.toFixed(0)}ms`)
  assert(segs.length > 1, 'S5 built segments')
  // No assertion on the number — it's machine-dependent; it's printed so a human
  // can see what every switch/flush used to cost.
}

console.log('')
if (failures === 0) console.log('✅ ALL COMPACT-SEGMENT CHECKS PASSED')
else { console.error(`❌ ${failures} check(s) FAILED`); process.exit(1) }

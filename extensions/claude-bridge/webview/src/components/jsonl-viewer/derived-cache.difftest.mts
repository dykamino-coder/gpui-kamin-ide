// Differential test for the structureVersion render-model cache.
//
// PROVES: the stream-only fast path (cache hit + O(tail) stub re-point) produces
// output IDENTICAL to a full recompute of the SAME current entries. If this ever
// diverges, a huge session would render stale/wrong during streaming — the exact
// class of "text vanished / froze" bug this cache must never reintroduce.
//
// Run: from the kamin-ide repo root →  npx tsx <abs path to this file>
// Pure logic only (no preact/DOM); tsx erases the type-only imports.

import { getRenderModel } from './derived-cache.ts'
import type { CompactSegmentRange } from './prepare-jsonl-entries.ts'
import type { JsonlEntryData } from '../../types/jsonl.ts'

let failures = 0
function assert(cond: boolean, msg: string): void {
  if (!cond) { failures++; console.error('  ✗ ' + msg) }
}

function textOf(e: any): string {
  const c = e?.message?.content
  if (typeof c === 'string') return c
  if (Array.isArray(c)) {
    return c.map((b: any) => b?.type === 'text' ? (b.text ?? '') : b?.type === 'thinking' ? (b.thinking ?? '') : b?.type === 'tool_use' ? `tool:${b.name}` : b?.type === 'tool_result' ? `res:${b.tool_use_id}` : '').join('|')
  }
  return ''
}
function project(model: { visibleMerged: JsonlEntryData[]; ver: string }): string {
  const rows = model.visibleMerged.map((e: any) => ({
    t: e.type, u: e.uuid, id: e.message?.id, txt: textOf(e),
    ie: e.interruptedByUser ?? null, sk: e.skippedByUser ?? null,
    grp: Array.isArray(e._groupEntries) ? e._groupEntries.length : null,
  }))
  return JSON.stringify({ rows, ver: model.ver })
}

// Oracle = a forced full recompute (unique structureVersion each call → never a
// cache hit). Compared against the REAL path which reuses the cache on stream-only.
let oracleTick = 1_000_000
function oracle(entries: JsonlEntryData[], segs: CompactSegmentRange[]): string {
  return project(getRenderModel('oracle', entries, segs, segs.length ? segs.length - 1 : 0, ++oracleTick))
}
function live(entries: JsonlEntryData[], segs: CompactSegmentRange[], sv: number): string {
  return project(getRenderModel('live', entries, segs, segs.length ? segs.length - 1 : 0, sv))
}

// Entry builders ------------------------------------------------------------
let uid = 0
const U = () => `u${uid++}`
function userMsg(text: string, parent?: string): JsonlEntryData {
  return { type: 'user', uuid: U(), parentUuid: parent, message: { role: 'user', content: [{ type: 'text', text }] } } as any
}
function stub(msgId: string, text: string): JsonlEntryData {
  return { type: 'assistant', uuid: `stream-${msgId}`, parentUuid: null, __streaming: true, message: { id: msgId, role: 'assistant', content: [{ type: 'text', text }] } } as any
}
function asstText(msgId: string, text: string, parent: string): JsonlEntryData {
  return { type: 'assistant', uuid: U(), parentUuid: parent, message: { id: msgId, role: 'assistant', content: [{ type: 'text', text }] } } as any
}
function asstTool(msgId: string, toolId: string, name: string, parent: string): JsonlEntryData {
  return { type: 'assistant', uuid: U(), parentUuid: parent, message: { id: msgId, role: 'assistant', content: [{ type: 'tool_use', id: toolId, name, input: {} }] } } as any
}
function toolResult(toolId: string, out: string, parent: string): JsonlEntryData {
  return { type: 'user', uuid: U(), parentUuid: parent, message: { role: 'user', content: [{ type: 'tool_result', tool_use_id: toolId, content: out }] } } as any
}
// A stream flush clone: fresh array + fresh stub object with grown text (mirrors
// scheduleStreamFlush) — same structureVersion.
function flushGrow(entries: JsonlEntryData[], msgId: string, newText: string): JsonlEntryData[] {
  return entries.map(e => {
    if ((e as any).__streaming !== undefined && e.message?.id === msgId) {
      return { ...e, message: { ...e.message, content: [{ type: 'text', text: newText }] } } as any
    }
    return e
  })
}

// ── Scenario 1: streaming growth — cache hit must equal full recompute ──────
console.log('Scenario 1: streaming text growth')
{
  let sv = 1
  const u1 = userMsg('do a thing')
  let entries: JsonlEntryData[] = [u1, stub('m1', '')]
  assert(live(entries, [], sv) === oracle(entries, []), 'S1.0 initial stub')
  for (const t of ['H', 'Hello', 'Hello wor', 'Hello world, here is a longer answer']) {
    entries = flushGrow(entries, 'm1', t) // stream flush: new ref, SAME sv
    const got = live(entries, [], sv)
    const want = oracle(entries, [])
    assert(got === want, `S1 grow="${t.slice(0, 12)}" — live cache-hit must equal full recompute`)
    assert(got.includes(t), `S1 grow="${t.slice(0, 12)}" — grown text is actually present`)
  }
}

// ── Scenario 2: tool burst + results, then a streaming answer over it ───────
console.log('Scenario 2: tool burst then streaming')
{
  let sv = 10
  const u1 = userMsg('run tools')
  const a1 = asstTool('mt1', 't1', 'Bash', u1.uuid!)
  const r1 = toolResult('t1', 'ok1', a1.uuid!)
  const a2 = asstTool('mt2', 't2', 'Read', r1.uuid!)
  const r2 = toolResult('t2', 'ok2', a2.uuid!)
  let entries: JsonlEntryData[] = [u1, a1, r1, a2, r2, stub('m9', '')]
  assert(live(entries, [], sv) === oracle(entries, []), 'S2.0 burst + stub')
  for (const t of ['par', 'partial answer', 'partial answer complete now']) {
    entries = flushGrow(entries, 'm9', t)
    assert(live(entries, [], sv) === oracle(entries, []), `S2 grow="${t.slice(0, 10)}"`)
  }
}

// ── Scenario 3: structural change (canonical replaces stub) forces recompute ─
console.log('Scenario 3: stub → canonical (structural)')
{
  let sv = 20
  const u1 = userMsg('answer me')
  let entries: JsonlEntryData[] = [u1, stub('m3', 'streamed text')]
  assert(live(entries, [], sv) === oracle(entries, []), 'S3.0 stub')
  // canonical arrives: structural change → sv bumps, stub replaced by real entry
  sv = 21
  const canonical = asstText('m3', 'streamed text', u1.uuid!)
  entries = [u1, canonical]
  const got = live(entries, [], sv)
  assert(got === oracle(entries, []), 'S3.1 canonical after sv bump equals full recompute')
  assert(got.includes('streamed text'), 'S3.1 canonical text present')
  assert(!got.includes('stream-m3'), 'S3.1 synthetic stub uuid gone')
}

// ── Scenario 4: compaction segment — only active segment rendered ───────────
console.log('Scenario 4: compaction segment scope')
{
  let sv = 30
  // pre-compact turn + boundary + post-compact turn
  const uOld = userMsg('old prompt')
  const aOld = asstText('mo', 'old answer', uOld.uuid!)
  const uNew = userMsg('new prompt', aOld.uuid!)
  let entries: JsonlEntryData[] = [uOld, aOld, uNew, stub('mn', '')]
  const segs: CompactSegmentRange[] = [{ from: 0, to: 2 }, { from: 2, to: 4 }] // active = latest (2..4)
  const got0 = live(entries, segs, sv)
  assert(got0 === oracle(entries, segs), 'S4.0 segmented stub')
  assert(!got0.includes('old answer'), 'S4.0 pre-compact turn hidden')
  for (const t of ['ne', 'new answer streaming']) {
    entries = flushGrow(entries, 'mn', t)
    // segment `to` grows as the array grows? here length stable (flush replaces), so seg fixed
    assert(live(entries, segs, sv) === oracle(entries, segs), `S4 grow="${t}"`)
  }
}

// ── Scenario 5: interrupt marker flags the preceding user prompt ─────────────
console.log('Scenario 5: interrupt flag')
{
  const sv = 40
  const u1 = userMsg('please stop midway')
  const a1 = asstText('mi', 'partial…', u1.uuid!)
  const interrupt = userMsg('[Request interrupted by user]', a1.uuid!)
  const entries: JsonlEntryData[] = [u1, a1, interrupt]
  const got = live(entries, [], sv)
  assert(got === oracle(entries, []), 'S5.0 interrupt equals recompute')
  assert(got.includes('"ie":true'), 'S5.0 preceding user prompt flagged interrupted')
}

// ── Scenario 7: bookkeeping rows must never occupy the render window ─────────
// Regression: the CLI re-emits ai-title/permission-mode between turns — a real
// session had ~260 in a row (dump: storeCount 1059, seenUuids 181). ai-title
// passed entryIsVisible but renders null, so the burst filled the whole
// 150-entry window and the chat came up BLANK with every real message pushed
// behind "N earlier messages".
console.log('Scenario 7: ai-title burst never counts as visible')
{
  const sv = 60
  const u1 = userMsg('real question')
  const a1 = asstText('mr1', 'real answer', u1.uuid!)
  const noise: JsonlEntryData[] = []
  for (let i = 0; i < 200; i++) {
    noise.push({ type: 'ai-title' } as unknown as JsonlEntryData)
    noise.push({ type: 'permission-mode' } as unknown as JsonlEntryData)
  }
  const entries = [u1, a1, ...noise]
  const out = live(entries, [], sv)
  assert(!out.includes('"t":"ai-title"'), 'S7 no ai-title row is visible')
  assert(!out.includes('"t":"permission-mode"'), 'S7 no permission-mode row is visible')
  assert(out.includes('real question'), 'S7 the real question survives the burst')
  assert(out.includes('real answer'), 'S7 the real answer survives the burst')
  assert(out === oracle(entries, []), 'S7 cache path matches full recompute')
}

// ── Scenario 6: PERF — on a huge session the stream path must be O(tail) ─────
// Build a big committed history, then measure N stream flushes on the cache
// (should be O(tail)) vs N forced full recomputes (O(segment)). The freeze is
// only fixed if the cache path is dramatically cheaper.
console.log('Scenario 6: perf — stream flush vs full recompute on a big session')
{
  const sv = 50
  const big: JsonlEntryData[] = []
  let parent = ''
  for (let k = 0; k < 4000; k++) {
    const u = userMsg(`turn ${k} prompt with some length to it`, parent); big.push(u); parent = u.uuid!
    const a = asstTool(`mb${k}`, `tb${k}`, 'Bash', parent); big.push(a); parent = a.uuid!
    const r = toolResult(`tb${k}`, `output of turn ${k} `.repeat(4), parent); big.push(r); parent = r.uuid!
  }
  let entries: JsonlEntryData[] = big.concat(stub('mlive', ''))
  live(entries, [], sv) // prime the cache (one full compute)

  const N = 60
  let tLive = 0, tFull = 0
  for (let i = 0; i < N; i++) {
    entries = flushGrow(entries, 'mlive', 'streaming answer token '.repeat(i + 1))
    let s = performance.now(); live(entries, [], sv); tLive += performance.now() - s   // cache hit
    s = performance.now(); oracle(entries, []); tFull += performance.now() - s          // full recompute
  }
  console.log(`  ${big.length} committed entries · ${N} flushes — cache-path ${tLive.toFixed(1)}ms total, full-recompute ${tFull.toFixed(1)}ms total (${(tFull / Math.max(tLive, 0.01)).toFixed(1)}× )`)
  assert(tLive * 4 < tFull, `perf: stream cache path (${tLive.toFixed(1)}ms) must be far cheaper than full recompute (${tFull.toFixed(1)}ms)`)
}

console.log('')
if (failures === 0) console.log('✅ ALL DIFFERENTIAL CHECKS PASSED — stream-cache output ≡ full recompute')
else { console.error(`❌ ${failures} check(s) FAILED`); process.exit(1) }

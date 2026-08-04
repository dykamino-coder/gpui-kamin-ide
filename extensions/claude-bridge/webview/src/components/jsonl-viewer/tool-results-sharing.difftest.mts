// The tool-result map is the biggest thing the chat keeps: it spans the whole
// history and holds the TEXT of every tool result. It used to be built inside
// each segment model, so a session with several compact segments kept a separate
// full copy per cached segment — and a heap sampler on a real session showed it
// as the top named allocation site while the shared renderer climbed past 4GB
// and the panels died with OUT_OF_MEMORY on repeat.
//
// This proves the copies are gone: every segment model of a tab must reference
// the SAME map object, and flipping segments must not rebuild it.
//
// Run: from the kamin-ide repo root →  npx tsx <path to this file>
import { getRenderModel, dropRenderModelCache } from './derived-cache.ts'
import type { JsonlEntryData } from '../../types/jsonl.ts'

let failures = 0
function check(name: string, ok: boolean, detail = ''): void {
  if (ok) { console.log(`  ok   ${name}`); return }
  failures++
  console.log(`  FAIL ${name} ${detail}`)
}

/** A session shaped like the real thing: tool calls whose results carry bulk. */
function makeEntries(turns: number, idOffset = 0): JsonlEntryData[] {
  const out: JsonlEntryData[] = []
  for (let n = 0; n < turns; n++) {
    const i = n + idOffset
    out.push({
      type: 'assistant', uuid: `a${String(i)}`, _ord: out.length,
      message: { id: `m${String(i)}`, content: [{ type: 'tool_use', id: `t${String(i)}`, name: 'Read', input: {} }] },
    } as unknown as JsonlEntryData)
    out.push({
      type: 'user', uuid: `u${String(i)}`, _ord: out.length,
      message: { content: [{ type: 'tool_result', tool_use_id: `t${String(i)}`, content: 'X'.repeat(4096) }] },
    } as unknown as JsonlEntryData)
  }
  return out
}

const TAB = 'tab-1'
const entries = makeEntries(200)
// Two compact segments over the same history — the shape that used to duplicate.
const segs = [{ from: 0, to: 200 }, { from: 200, to: entries.length }] as never

dropRenderModelCache(TAB)
const seg0 = getRenderModel(TAB, entries, segs, 0, 1)
const seg1 = getRenderModel(TAB, entries, segs, 1, 1)

check('both segments expose a tool-result map', !!seg0.toolResults && !!seg1.toolResults)
check(
  'segments SHARE one map instance (no per-segment copy)',
  seg0.toolResults === seg1.toolResults,
  seg0.toolResults === seg1.toolResults ? '' : '— each segment built its own copy',
)

// Flipping back must not rebuild: same instance again.
const seg0again = getRenderModel(TAB, entries, segs, 0, 1)
check('flipping segments reuses the map', seg0again.toolResults === seg0.toolResults)

// An append must EXTEND the map, not rebuild it: rebuilding is O(history) and
// re-creates the text of every tool result, which a heap sampler measured at
// ~200MB per window on a 23k-entry session. The property that matters is that
// the new result becomes visible — whether the instance is reused is the
// optimisation, so assert both.
const sizeBefore = seg0.toolResults?.size ?? 0
// Unique ids — reusing t0 would overwrite the existing key and prove nothing.
const grown = [...entries, ...makeEntries(1, 1000)]
const afterChange = getRenderModel(TAB, grown, segs, 0, 2)
check('an append extends the SAME map in place', afterChange.toolResults === seg0.toolResults)
check('the appended result is visible', (afterChange.toolResults?.size ?? 0) === sizeBefore + 1)

// A non-append change (history rewritten under us — replay, compaction) must
// fall back to a full rebuild: a stale map would hide a tool result, which is
// worse than paying for the rebuild.
const rewritten = makeEntries(50, 5000)
const afterRewrite = getRenderModel(TAB, rewritten, segs, 0, 3)
check('a rewritten history rebuilds from scratch', afterRewrite.toolResults !== afterChange.toolResults)
check('the rebuilt map drops the old results', (afterRewrite.toolResults?.size ?? 0) === 50)

// Dropping a tab must release it — this is what eviction relies on.
dropRenderModelCache(TAB)
const afterDrop = getRenderModel(TAB, entries, segs, 0, 1)
check('dropping the tab releases the map', afterDrop.toolResults !== afterChange.toolResults)

console.log(failures === 0
  ? '\n✅ tool-result map is shared per tab, not copied per segment'
  : `\n❌ ${String(failures)} check(s) failed`)
process.exit(failures === 0 ? 0 : 1)

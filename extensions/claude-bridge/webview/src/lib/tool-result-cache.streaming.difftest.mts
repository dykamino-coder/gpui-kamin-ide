// Does the shared tool-result cache survive the STREAMING path?
//
// Streaming does not go through appendJsonlEntries. It mutates a block in place
// and, per animation frame, clones ONLY the streaming stub into a fresh array
// (scheduleStreamFlush) WITHOUT bumping structureVersion — the load-bearing
// invariant is that a delta only appends text to an existing block, never
// changes structure. A content_block_start (applyStreamingEntry) DOES bump.
//
// So the cache must behave like this:
//   • delta frame  → same structureVersion → return the SAME map, no rebuild,
//     even though the entries array is a new object each frame;
//   • boundary bump → recompute, and the map must equal a full rebuild.
//
// If the delta path rebuilt the full-history map every frame, that is the
// 33ms/record cost landing ~30×/sec on the live turn — the exact "chat doesn't
// fly" the shared cache exists to remove. This test pins that it does not.
import { getToolResults, buildToolResultMap, dropToolResults } from './tool-result-cache.ts'

type Block = { type: string; text?: string; tool_use_id?: string; content?: unknown; is_error?: boolean }
type Rec = { type: string; message?: { id?: string; content?: Block[] }; __streaming?: true }

let failures = 0
const ok = (cond: boolean, msg: string) => { if (!cond) { failures++; console.log(`  ❌ ${msg}`) } else console.log(`  ✅ ${msg}`) }

const TAB = 'stream-tab'
const userToolResult = (id: string, text: string): Rec =>
  ({ type: 'user', message: { content: [{ type: 'tool_result', tool_use_id: id, content: text }] } })
const assistantStub = (msgId: string, text: string): Rec =>
  ({ type: 'assistant', __streaming: true, message: { id: msgId, content: [{ type: 'text', text }] } })

const mapEqualsRebuild = (entries: Rec[], got: ReturnType<typeof getToolResults>['map']): boolean => {
  const fresh = buildToolResultMap(entries as never)
  if ((got?.size ?? 0) !== fresh.size) return false
  for (const [k, v] of fresh) { const g = got?.get(k); if (!g || g.content !== v.content || g.isError !== v.isError) return false }
  return true
}

// ── Committed history: a few tool results, then a streaming turn begins. ──
dropToolResults(TAB)
const history: Rec[] = [
  userToolResult('t1', 'read fileA'),
  userToolResult('t2', 'grep result'),
]
let version = 5
let r = getToolResults(TAB, history as never, version)
ok(mapEqualsRebuild(history, r.map), 'baseline map equals a full rebuild')
ok(r.map?.size === 2, 'baseline has both tool results')
const baselineMap = r.map

// A streaming stub appears (content_block_start → applyStreamingEntry bumps).
version++ // boundary snapshot bumps structureVersion
let live: Rec[] = [...history, assistantStub('m1', 'Hel')]
r = getToolResults(TAB, live as never, version)
ok(r.map?.size === 2, 'stub does not add a tool result (it is assistant text)')

// ── Delta frames: text grows, stub is CLONED each frame, version UNCHANGED. ──
const mapAcrossDeltas: unknown[] = []
for (const text of ['Hell', 'Hello', 'Hello ', 'Hello w', 'Hello wo']) {
  // scheduleStreamFlush clones ONLY the stub into a fresh array — a new array
  // object and a new tail object every frame, same structureVersion.
  live = [...history, assistantStub('m1', text)]
  r = getToolResults(TAB, live as never, version) // SAME version
  mapAcrossDeltas.push(r.map)
}
ok(mapAcrossDeltas.every((m) => m === baselineMap),
  'every delta frame returns the SAME map object — no rebuild despite a new array each frame')

// ── A real tool result lands mid-turn (a boundary snapshot → version bump). ──
version++
const grown: Rec[] = [...history, userToolResult('t3', 'wrote fileB'), assistantStub('m1', 'Hello wo')]
r = getToolResults(TAB, grown as never, version)
ok(r.map?.size === 3, 'a tool result arriving on a version bump is folded in')
ok(mapEqualsRebuild(grown, r.map), 'post-bump map equals a full rebuild')
// The map is REUSED and extended in place, not rebuilt into a new object — the
// committed-anchor fast path holds across the boundary because the trailing stub
// is excluded from the anchor. (This is the freeze fix: no full re-materialise.)
ok(r.map === baselineMap, 'the boundary extends the SAME map in place — no rebuild')

// ── The stub is canonicalized (streaming ends). Still version-stable frames. ──
const canonical: Rec[] = [...history, userToolResult('t3', 'wrote fileB'),
  { type: 'assistant', message: { id: 'm1', content: [{ type: 'text', text: 'Hello world' }] } }]
r = getToolResults(TAB, canonical as never, version) // same version — a late flush
ok(r.map?.size === 3, 'canonicalization does not disturb the tool-result map')

console.log(failures === 0
  ? '✅ STREAMING-INTERACTION CHECKS PASSED — delta frames reuse the cache, bumps rebuild correctly'
  : `❌ ${failures} FAILURE(S)`)
process.exit(failures === 0 ? 0 : 1)

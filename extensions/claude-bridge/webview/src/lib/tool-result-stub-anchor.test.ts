// The freeze fix: during streaming, the trailing assistant stub is REPLACED with
// a fresh object on every content_block boundary. The tool-result map (a fold of
// USER tool_result entries) doesn't depend on that stub at all — but the old
// fast-path anchored on the literal last entry, so each boundary failed the
// identity check and rebuilt the ENTIRE map (hundreds of MB of tool-output text
// on a real session → the main-thread freeze). Anchoring on the last COMMITTED
// entry makes a stub swap reuse the map.
import { describe, it, expect, beforeEach } from 'vitest'
import { getToolResults, dropToolResults, foldToolResultsInto, buildToolResultMap } from './tool-result-cache'
import type { JsonlEntryData } from '../types/jsonl'

type Rec = JsonlEntryData

const userResult = (id: string, text: string): Rec =>
  ({ type: 'user', message: { content: [{ type: 'tool_result', tool_use_id: id, content: text }] } } as unknown as Rec)
// A fresh stub object each call — this is what a streaming boundary produces.
const stub = (msgId: string, text: string): Rec =>
  ({ type: 'assistant', __streaming: true, message: { id: msgId, content: [{ type: 'text', text }] } } as unknown as Rec)

const TAB = 't'

describe('tool-result map — committed anchor across streaming boundaries', () => {
  beforeEach(() => dropToolResults(TAB))

  it('reuses the SAME map object when only the trailing stub is replaced', () => {
    const committed = [userResult('r1', 'read A'), userResult('r2', 'grep B')]
    let ver = 1
    const first = getToolResults(TAB, [...committed, stub('m1', 'Hel')], ver).map
    expect(first?.size).toBe(2)

    // Three content_block boundaries: each bumps structureVersion AND swaps the
    // stub for a NEW object. The old code rebuilt the map each time; now it must
    // hand back the identical map instance — proof no rebuild happened.
    for (const text of ['Hell', 'Hello', 'Hello world']) {
      const m = getToolResults(TAB, [...committed, stub('m1', text)], ++ver).map
      expect(m).toBe(first) // same object — not rebuilt
    }
  })

  it('still folds a NEW committed tool_result that lands after the stub finalizes', () => {
    const committed = [userResult('r1', 'read A')]
    let ver = 1
    getToolResults(TAB, [...committed, stub('m1', 'working')], ver)
    // The turn finalizes: the stub becomes a committed assistant entry and a new
    // user tool_result arrives; a fresh stub starts the next turn.
    const grown = [
      ...committed,
      { type: 'assistant', message: { id: 'm1', content: [{ type: 'text', text: 'done' }] } } as unknown as Rec,
      userResult('r2', 'wrote C'),
      stub('m2', 'next'),
    ]
    const m = getToolResults(TAB, grown, ++ver).map
    expect(m?.size).toBe(2)
    expect(m?.get('r2')?.content).toBe('wrote C') // the new result was folded in
    expect(m?.get('r1')?.content).toBe('read A') // and the old one survived
  })

  it('re-folds an OVERWRITE of an existing id and refreshes sig via the fast path', () => {
    // A retried tool call (or a duplicate JSONL line with a fresh uuid) can fold a
    // SECOND result under an id already in the map. That leaves map.size unchanged,
    // so a size-diff gate would skip the sig recompute and the render's vnode cache
    // (keyed on sig) would keep painting the OLD result. The fold must report the
    // overwrite so sig is recomputed. `r0` anchors the fast path (a live stub swap)
    // while `r1` is the id that gets overwritten with longer text.
    const anchor = userResult('r0', 'anchor')
    let ver = 1
    const first = getToolResults(TAB, [anchor, userResult('r1', 'short'), stub('m1', 'a')], ver)
    const sigBefore = first.sig
    // A new turn commits a fresh tool_result reusing id r1 with DIFFERENT, longer
    // text. Same anchor object at builtUpTo-1 → fast path; size stays the same.
    const grown = [anchor, userResult('r1', 'short'),
      { type: 'assistant', message: { id: 'm1', content: [{ type: 'text', text: 'a' }] } } as unknown as Rec,
      userResult('r1', 'a much longer replacement result'), stub('m2', 'b')]
    const second = getToolResults(TAB, grown, ++ver)
    expect(second.map?.size).toBe(2) // r0 + r1 — no new key, an overwrite
    expect(second.map?.get('r1')?.content).toBe('a much longer replacement result')
    expect(second.sig).not.toBe(sigBefore) // sig refreshed → vnode cache invalidated
    const expectedSig = [...second.map!.values()].reduce((n, v) => n + v.content.length, 0)
    expect(second.sig).toBe(expectedSig)
  })

  it('foldToolResultsInto reports whether it mutated the map', () => {
    const map = buildToolResultMap([userResult('r1', 'aaa')])
    // Re-folding the SAME entry changes nothing → false (no spurious invalidation).
    expect(foldToolResultsInto(map, [userResult('r1', 'aaa')])).toBe(false)
    // Overwriting with different text → true even though size is unchanged.
    expect(foldToolResultsInto(map, [userResult('r1', 'bbbb')])).toBe(true)
    expect(map.get('r1')?.content).toBe('bbbb')
    // A brand-new id → true.
    expect(foldToolResultsInto(map, [userResult('r2', 'c')])).toBe(true)
  })

  it('a reorder/prepend (committed anchor object moves) still triggers a rebuild', () => {
    const committed = [userResult('r1', 'a'), userResult('r2', 'b')]
    let ver = 1
    const before = getToolResults(TAB, committed, ver).map
    // Prepend an older page — the anchor at builtUpTo-1 is now a different object.
    const prepended = [userResult('r0', 'older'), ...committed]
    const after = getToolResults(TAB, prepended, ++ver).map
    expect(after).not.toBe(before) // rebuilt (correctly — the prefix changed)
    expect(after?.size).toBe(3)
    expect(after?.get('r0')?.content).toBe('older')
  })
})

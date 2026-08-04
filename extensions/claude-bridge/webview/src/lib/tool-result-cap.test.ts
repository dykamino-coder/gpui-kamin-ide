// The tool-result map holds the full text of every tool result over the whole
// history. On a marathon session (the active tab is never evicted) it grew until
// the shared renderer hit its ~4GB heap ceiling. It is now capped by total text
// size, evicting OLDEST first — the chat only renders the recent tail, so a
// scrolled-off old result being dropped is invisible, while every visible
// (recent) result stays intact.
import { describe, it, expect } from 'vitest'
import { getToolResults, dropToolResults } from './tool-result-cache'
import type { JsonlEntryData } from '../types/jsonl'

// A user entry carrying one tool_result of `size` chars for `id`.
const result = (id: string, size: number): JsonlEntryData =>
  ({ type: 'user', message: { content: [{ type: 'tool_result', tool_use_id: id, content: 'x'.repeat(size) }] } } as unknown as JsonlEntryData)

const CAP = 24_000_000

describe('tool-result map cap', () => {
  it('keeps everything while under the cap', () => {
    dropToolResults('t')
    const entries = [result('a', 1000), result('b', 2000)]
    const { map, sig } = getToolResults('t', entries, 1)
    expect(map?.size).toBe(2)
    expect(sig).toBe(3000)
  })

  it('evicts the OLDEST results once the total text exceeds the cap', () => {
    dropToolResults('t2')
    // Five 6M-char results = 30M > 24M cap. Oldest evicted until under.
    const big = 6_000_000
    const entries = [result('r0', big), result('r1', big), result('r2', big), result('r3', big), result('r4', big)]
    const { map, sig } = getToolResults('t2', entries, 1)
    expect(sig).toBeLessThanOrEqual(CAP)
    // The newest results survive; the oldest are gone.
    expect(map?.has('r4')).toBe(true)
    expect(map?.has('r3')).toBe(true)
    expect(map?.has('r0')).toBe(false)
  })

  it('caps on the incremental append path too, not only a full rebuild', () => {
    dropToolResults('t3')
    const big = 6_000_000
    let entries: JsonlEntryData[] = [result('r0', big), result('r1', big)]
    getToolResults('t3', entries, 1) // 12M — under cap, cached
    // Append more via the fast path (same prefix objects), pushing over the cap.
    entries = [...entries, result('r2', big), result('r3', big), result('r4', big)]
    const { map, sig } = getToolResults('t3', entries, 2)
    expect(sig).toBeLessThanOrEqual(CAP)
    expect(map?.has('r4')).toBe(true)
    expect(map?.has('r0')).toBe(false) // oldest dropped on the append path
  })

  it('a single result under the cap is always kept in full', () => {
    dropToolResults('t4')
    const { map } = getToolResults('t4', [result('solo', 10_000_000)], 1)
    expect(map?.get('solo')?.content.length).toBe(10_000_000)
  })
})

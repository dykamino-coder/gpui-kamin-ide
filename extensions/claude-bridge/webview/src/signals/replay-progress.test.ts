import { describe, it, expect, beforeEach } from 'vitest'
import { replayProgressByTab, setReplayProgress } from './replay-progress'

describe('replay progress', () => {
  beforeEach(() => { replayProgressByTab.value = new Map() })

  it('records a percent per tab', () => {
    setReplayProgress('a', 40)
    setReplayProgress('b', 70)
    expect(replayProgressByTab.value.get('a')).toBe(40)
    expect(replayProgressByTab.value.get('b')).toBe(70)
  })

  it('is a no-op when the percent is unchanged — no needless re-render', () => {
    setReplayProgress('a', 40)
    const ref = replayProgressByTab.value
    setReplayProgress('a', 40)
    expect(replayProgressByTab.value).toBe(ref) // same Map object, no signal write
  })

  it('writes a new Map when the percent changes', () => {
    setReplayProgress('a', 40)
    const ref = replayProgressByTab.value
    setReplayProgress('a', 55)
    expect(replayProgressByTab.value).not.toBe(ref)
    expect(replayProgressByTab.value.get('a')).toBe(55)
  })

  it('clears one tab without touching the others', () => {
    setReplayProgress('a', 40)
    setReplayProgress('b', 70)
    setReplayProgress('a', null)
    expect(replayProgressByTab.value.has('a')).toBe(false)
    expect(replayProgressByTab.value.get('b')).toBe(70)
  })

  it('clearing an absent tab is a no-op — does not churn the signal', () => {
    setReplayProgress('a', 40)
    const ref = replayProgressByTab.value
    setReplayProgress('missing', null)
    expect(replayProgressByTab.value).toBe(ref)
  })
})

// The server sums each batch's own byte span; batches arrive newest-first and
// out of order. This proves the coverage that drives the percent is the SAME
// total regardless of delivery order, and reaches the full file size — the whole
// reason the metric is coverage, not max(_posEnd) (which would jump to ~100% on
// the first newest-batch).
describe('byte coverage is order-independent (server metric)', () => {
  type Rec = { _pos: number; _posEnd: number }
  const span = (batch: Rec[]) => batch.reduce((n, r) => n + (r._posEnd - r._pos), 0)

  // A file of 10 contiguous 100-byte records → total 1000.
  const file: Rec[] = Array.from({ length: 10 }, (_, i) => ({ _pos: i * 100, _posEnd: (i + 1) * 100 }))
  const total = 1000

  const coverageOverBatches = (batches: Rec[][]): number =>
    batches.reduce((acc, b) => Math.min(total, acc + span(b)), 0)

  it('forward order sums to the whole file', () => {
    const batches = [file.slice(0, 5), file.slice(5, 10)]
    expect(coverageOverBatches(batches)).toBe(total)
  })

  it('reverse (newest-first) order sums to the SAME whole file', () => {
    const batches = [file.slice(5, 10), file.slice(0, 5)] // as emitRangeFromEnd delivers
    expect(coverageOverBatches(batches)).toBe(total)
  })

  it('is monotonic and never exceeds 100% at any step', () => {
    const batches = [file.slice(8, 10), file.slice(4, 8), file.slice(0, 4)]
    let acc = 0
    const pcts: number[] = []
    for (const b of batches) { acc = Math.min(total, acc + span(b)); pcts.push(Math.floor((acc / total) * 100)) }
    expect(pcts).toEqual([20, 60, 100])
    for (let i = 1; i < pcts.length; i++) expect(pcts[i]!).toBeGreaterThanOrEqual(pcts[i - 1]!)
  })
})

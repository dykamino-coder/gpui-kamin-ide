import { describe, it, expect } from 'vitest'
import { computeContextStats } from './session-cost'

// One assistant turn on the 1M Opus tier. `used` = input + cache_read + cache_creation.
const turn = (msgId: string, inputTok: number, opts: { output?: number; cacheRead?: number } = {}) => ({
  type: 'assistant',
  model: 'claude-opus-5',
  message: {
    id: msgId,
    usage: {
      input_tokens: inputTok,
      output_tokens: opts.output ?? 0,
      cache_read_input_tokens: opts.cacheRead ?? 0,
      cache_creation_input_tokens: 0,
    },
  },
})

const TAB_MODEL = 'claude-opus-5' // 1M нативно (4.8[1m]-вариантов больше нет)

describe('computeContextStats — live segment tracks the LATEST turn (the /compact-freeze fix)', () => {
  it('after a /compact the bar follows the window DOWN, not stuck at the pre-compact peak', () => {
    // context climbed to 994K, /compact collapsed it, post-compact turns are ~64K.
    const entries = [
      turn('m1', 500_000),
      turn('m2', 994_000), // pre-compact high-water mark
      turn('m3', 64_000),  // first post-compact turn
      turn('m4', 66_000),  // latest
    ]
    const s = computeContextStats(entries, /* isLiveSegment */ true, TAB_MODEL)!
    expect(s.used).toBe(66_000)         // latest, NOT the 994K peak
    expect(s.pct).toBe(7)               // 66K / 1M
    expect(s.limit).toBe(1_000_000)
  })

  it('keeps moving as new turns arrive (does not latch)', () => {
    expect(computeContextStats([turn('m1', 994_000), turn('m2', 60_000)], true, TAB_MODEL)!.used).toBe(60_000)
    expect(computeContextStats([turn('m1', 994_000), turn('m2', 70_000)], true, TAB_MODEL)!.used).toBe(70_000)
  })
})

describe('computeContextStats — archived segment keeps the PEAK', () => {
  it('shows how full the sub-conversation got before it was compacted', () => {
    const entries = [turn('m1', 500_000), turn('m2', 994_000), turn('m3', 300_000)]
    const s = computeContextStats(entries, /* isLiveSegment */ false, TAB_MODEL)!
    expect(s.used).toBe(994_000) // peak, not the 300K tail
    expect(s.pct).toBe(99)
  })
})

describe('computeContextStats — general', () => {
  it('returns null when no assistant turn carries usage', () => {
    expect(computeContextStats([{ type: 'user' }, { type: 'assistant' }], true, TAB_MODEL)).toBeNull()
  })

  it('skips <synthetic> rows so the bar does not collapse to 0%', () => {
    const entries = [
      turn('m1', 64_000),
      { type: 'assistant', model: '<synthetic>', message: { id: 's', usage: { input_tokens: 0 } } },
    ]
    expect(computeContextStats(entries, true, TAB_MODEL)!.used).toBe(64_000)
  })

  it('dedups cost by message.id (parallel tool_use blocks share one usage)', () => {
    // Same turn emitted as 3 rows sharing message.id — cost counted once.
    const one = computeContextStats([turn('m1', 1000, { output: 1000 })], true, TAB_MODEL)!.cost
    const three = computeContextStats(
      [turn('m1', 1000, { output: 1000 }), turn('m1', 1000, { output: 1000 }), turn('m1', 1000, { output: 1000 })],
      true, TAB_MODEL)!.cost
    expect(three).toBeCloseTo(one, 10)
  })

  it('Opus 5: 1M window нативно, без [1m]-тега', () => {
    const s = computeContextStats([turn('m1', 500_000)], true, TAB_MODEL)!
    expect(s.limit).toBe(1_000_000)
    expect(s.pct).toBe(50)
  })
})

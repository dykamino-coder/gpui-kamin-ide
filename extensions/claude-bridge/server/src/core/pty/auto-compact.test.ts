import { describe, expect, it } from 'vitest'

import { AUTO_COMPACT_PCT, AUTO_COMPACT_WINDOW_TOKENS, autoCompactEnv, autoCompactThreshold } from './auto-compact'

describe('auto-compact policy', () => {
  it('exports the CLI env from the single policy constants', () => {
    expect(AUTO_COMPACT_WINDOW_TOKENS).toBe(530_000)
    expect(AUTO_COMPACT_PCT).toBe(92)
    expect(autoCompactEnv()).toEqual({ CLAUDE_CODE_AUTO_COMPACT_WINDOW: '530k', CLAUDE_AUTOCOMPACT_PCT_OVERRIDE: '92' })
  })

  it('mirrors the CLI 2.1.322 threshold: clamp, 20k output reserve, min(pct, remainder - 13k)', () => {
    // 1M model: min(530k, 1M) - 20k = 510k; min(floor(510k * 0.92), 510k - 13k).
    expect(autoCompactThreshold(1_000_000)).toBe(469_200)
    // Unknown window: the override is not clamped.
    expect(autoCompactThreshold(null)).toBe(469_200)
    expect(autoCompactThreshold(undefined)).toBe(469_200)
    expect(autoCompactThreshold(0)).toBe(469_200)
    // A smaller model window clamps the override first.
    expect(autoCompactThreshold(200_000)).toBe(165_600)
    // The 13k summary buffer wins over the percentage on tiny windows.
    expect(autoCompactThreshold(150_000)).toBe(117_000)
  })
})

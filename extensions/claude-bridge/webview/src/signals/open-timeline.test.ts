import { describe, it, expect, beforeEach, vi, afterEach } from 'vitest'
import {
  markOpen,
  getOpenTimeline,
  resetOpenTimeline,
  formatOpenTimeline,
  stalledForMs,
  lastOpenPhase,
} from './open-timeline'

describe('open-timeline', () => {
  beforeEach(() => {
    vi.useFakeTimers()
    vi.setSystemTime(new Date('2026-07-23T00:00:00Z'))
    resetOpenTimeline('t1')
  })
  afterEach(() => vi.useRealTimers())

  it('records the first occurrence of each phase, at ms from the first mark', () => {
    markOpen('t1', 'activate')
    vi.advanceTimersByTime(1500)
    markOpen('t1', 'mcp-ready')
    expect(getOpenTimeline('t1')).toEqual([
      { phase: 'activate', at: 0 },
      { phase: 'mcp-ready', at: 1500 },
    ])
  })

  it('ignores repeats so a chatty phase cannot bury the shape of the open', () => {
    markOpen('t1', 'replay-progress')
    vi.advanceTimersByTime(50)
    markOpen('t1', 'replay-progress')
    expect(getOpenTimeline('t1')).toHaveLength(1)
  })

  it('keeps tabs independent', () => {
    resetOpenTimeline('t2')
    markOpen('t1', 'activate')
    markOpen('t2', 'activate')
    markOpen('t2', 'bound')
    expect(getOpenTimeline('t1')).toHaveLength(1)
    expect(getOpenTimeline('t2')).toHaveLength(2)
    resetOpenTimeline('t2')
  })

  it('reports the phase we are still waiting to leave, and for how long', () => {
    markOpen('t1', 'activate')
    vi.advanceTimersByTime(1000)
    markOpen('t1', 'replay-status')
    vi.advanceTimersByTime(12_000)
    expect(lastOpenPhase('t1')).toBe('replay-status')
    expect(stalledForMs('t1')).toBe(12_000)
  })

  it('formats sub-second marks in ms and longer ones in seconds', () => {
    markOpen('t1', 'activate')
    vi.advanceTimersByTime(400)
    markOpen('t1', 'mcp-loading')
    vi.advanceTimersByTime(41_000)
    markOpen('t1', 'mcp-ready')
    expect(formatOpenTimeline('t1')).toBe('activate 0ms · mcp-loading 400ms · mcp-ready 41.4s')
  })

  it('is a no-op for a missing tab id', () => {
    markOpen(null, 'activate')
    markOpen(undefined, 'activate')
    expect(getOpenTimeline(null)).toEqual([])
    expect(lastOpenPhase(null)).toBe('(nothing yet)')
    expect(stalledForMs(null)).toBe(0)
  })
})

import { describe, expect, it } from 'vitest'
import { formatPercent, formatReset, reportedNote, usageItems, usageLevel } from './usage-format'
import { setTabUsage, usageByTab } from '../signals/session-usage'
import type { SessionUsage } from '../../shared/types'

const NOW = Date.parse('2026-10-13T12:00:00Z')
const iso = (ms: number) => new Date(NOW + ms).toISOString()

const USAGE: SessionUsage = {
  windows: [
    { id: 'five_hour', label: '5 hours', percent: 7.4, resetsAt: iso(3 * 3600_000 + 5 * 60_000) },
    { id: 'seven_day', label: 'Week (all)', percent: 91, resetsAt: iso(4 * 86400_000) },
  ],
  contextPercent: 72,
  contextWindowSize: 200000,
  model: 'claude-sonnet-4-6',
  effort: 'medium',
  observedAt: iso(0),
}

describe('usage-format', () => {
  it('formats percent and relative resets', () => {
    expect(formatPercent(7.4)).toBe('7%')
    expect(formatReset(iso(42 * 60_000), NOW)).toBe('in 42m')
    expect(formatReset(iso(3 * 3600_000 + 5 * 60_000), NOW)).toBe('in 3h 05m')
    expect(formatReset(iso(-5000), NOW)).toBe('in 0m')
    expect(formatReset('nope', NOW)).toBe('')
    expect(formatReset(iso(4 * 86400_000), NOW)).toMatch(/^[A-Z][a-z]{2} \d{2}:\d{2}$/)
  })

  it('grades usage levels', () => {
    expect(usageLevel(69.9)).toBe('ok')
    expect(usageLevel(70)).toBe('warn')
    expect(usageLevel(90)).toBe('high')
  })

  it('orders 5h, 7d, context and skips expired windows', () => {
    const items = usageItems(USAGE, NOW)
    expect(items.map(i => [i.label, i.level, i.reset !== ''])).toEqual([
      ['5h', 'ok', true],
      ['7d', 'high', true],
      ['ctx', 'warn', false],
    ])
    expect(items[2]!.title).toContain('of 200k tokens')
    const expired = usageItems({ ...USAGE, windows: [{ ...USAGE.windows[0]!, resetsAt: iso(-1) }] }, NOW)
    expect(expired.map(i => i.key)).toEqual(['context'])
    expect(usageItems(undefined, NOW)).toEqual([])
    expect(usageItems({ ...USAGE, windows: [], contextPercent: null }, NOW)).toEqual([])
  })

  it('keeps usage per tab', () => {
    setTabUsage('a', USAGE)
    setTabUsage('b', { ...USAGE, contextPercent: 1 })
    expect(usageByTab.value.get('a')?.contextPercent).toBe(72)
    setTabUsage('a', null)
    expect(usageByTab.value.has('a')).toBe(false)
    expect(usageByTab.value.get('b')?.contextPercent).toBe(1)
  })
})

describe('limits reported earlier', () => {
  it('notes the report time only when it is not recent', () => {
    expect(reportedNote(iso(-60_000), NOW)).toBe('')
    expect(reportedNote(null, NOW)).toBe('')
    expect(reportedNote(iso(-3 * 3600_000), NOW)).toMatch(/^ \(reported \d{2}:\d{2}, updates after the next reply\)$/)
    const items = usageItems({ ...USAGE, limitsObservedAt: iso(-3 * 3600_000) }, NOW)
    expect(items[0]!.title).toContain('updates after the next reply')
    expect(items[2]!.title).not.toContain('reported')
  })
})

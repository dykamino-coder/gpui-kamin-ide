import { describe, expect, it } from 'vitest'
import { combinePlanUsage, emptyPlanUsage, parseStatusline, parseUsageScreen } from './plan-usage'

const now = new Date().toISOString()
const future = Math.floor(Date.now() / 1000) + 86400
const structured = {
  version: '2.1.284',
  observedAt: now,
  rate_limits: {
    five_hour: { used_percentage: 22.5, resets_at: future },
    seven_day: { used_percentage: 44, resets_at: future + 86400 },
    seven_day_fable: { used_percentage: 99, resets_at: future },
  },
}
const tui = (model: string) => `Current week (${model})\n17% used\nResets next Monday\nExtra usage\nNot enabled`
describe('BR-20 dynamic plan windows and freshness', () => {
  it('prefers valid documented common windows and never invents an undocumented model capability', () => {
    const data = combinePlanUsage(parseUsageScreen('Current session\n91% used\nResets soon', now), structured, null)
    expect(data.windows).toHaveLength(2)
    expect(data.windows[0]).toMatchObject({
      id: 'five_hour',
      percent: 22.5,
      source: 'statusline',
      resetsAt: new Date(future * 1000).toISOString(),
    })
    expect(data.windows.some((window) => window.model)).toBe(false)
    expect(data).toMatchObject({ state: 'complete', claudeCodeVersion: '2.1.284', source: 'statusline' })
  })
  it.each(['Fable', 'Sonnet only', 'Future Model', 'Opus'])(
    'keeps optional TUI model %s as a dynamic window',
    (model) => {
      const data = combinePlanUsage(parseUsageScreen(tui(model), now), structured, null)
      expect(data.windows.find((window) => window.model)).toMatchObject({
        model: model.replace(/ only$/, ''),
        percent: 17,
        source: 'tui-compatibility',
        resetsAt: null,
        reason: 'reset-timestamp-unavailable',
      })
      expect(data.extra).toBe('not enabled')
    },
  )
  it('tolerates reordered sections and additional headings without crossing their percent/reset boundaries', () => {
    const screen =
      tui('Fable') +
      '\nCurrent year (future)\n99% used\nResets distant future\nCurrent session\n28% used\nResets 6pm (UTC)\nCurrent week (all models)\n42% used\nResets Monday\nBonus promotion!'
    const data = parseUsageScreen(screen, now)
    expect(data.windows.map((window) => [window.id, window.percent])).toEqual([
      ['five_hour', 28],
      ['seven_day', 42],
      ['seven_day_model:fable', 17],
    ])
    expect(data.diagnostics.missing).toContain('five_hour.reset-timestamp')
    expect(data.state).toBe('partial')
  })
  it('retains last-known rows with stale labels on partial or unavailable capture', () => {
    const previous = combinePlanUsage(parseUsageScreen(tui('Fable'), now), structured, null)
    const partial = combinePlanUsage(parseUsageScreen('Current session\n25% used\nResets soon', now), null, previous)
    expect(partial.windows.find((window) => window.id === 'seven_day')).toMatchObject({
      percent: 44,
      freshness: 'stale',
      reason: 'not-reported-current-capture',
    })
    expect(partial.windows.find((window) => window.model === 'Fable')?.freshness).toBe('stale')
    const unavailable = combinePlanUsage(emptyPlanUsage('capture-error', now), null, previous)
    expect(unavailable.windows).toHaveLength(3)
    expect(unavailable.windows.every((window) => window.freshness === 'stale')).toBe(true)
    expect(unavailable).toMatchObject({ state: 'unavailable', reason: 'capture-error' })
    expect(previous.windows.every((window) => window.freshness === 'fresh')).toBe(true)
  })
  it('fails closed on invalid, expired or oversized metadata and exposes bounded diagnostics on every partial result', () => {
    expect(
      parseStatusline({
        version: 'private token',
        rate_limits: {
          five_hour: { used_percentage: Infinity, resets_at: future },
          seven_day: { used_percentage: 30, resets_at: 0 },
        },
      }),
    ).toEqual({ windows: [], version: null })
    expect(parseStatusline({ rate_limits: { five_hour: { used_percentage: 10, resets_at: 1 } } }).windows).toEqual([])
    const data = parseUsageScreen('Error: synthetic credential=private-secret\nCurrent session\n101% used', now)
    expect(data.state).toBe('unavailable')
    expect(data.diagnostics.missing).toContain('five_hour')
    expect(JSON.stringify(data)).not.toContain('private-secret')
    const many = Array.from({ length: 40 }, (_, i) => tui(`Model ${i}`)).join('\n')
    expect(parseUsageScreen(many, now).windows).toHaveLength(16)
  })
  it('marks truncated output as stale rather than a successful fresh capture', () => {
    const current = parseUsageScreen(tui('Fable'), now)
    current.diagnostics.outputLimited = true
    const data = combinePlanUsage(current, structured, null)
    expect(data.state).toBe('unavailable')
    expect(data.windows.every((window) => window.freshness === 'stale')).toBe(true)
    expect(data.reason).toBe('output-limit')
  })
})

import type { SessionUsage } from '../../shared/types'

// Compact text for the limits row under the chat input. Labels mirror the
// dashboard plan-usage view (BR-20): "5 hours" → "5h", "Week (all)" → "7d".

const SHORT: Record<string, string> = { five_hour: '5h', seven_day: '7d' }

export function formatPercent(value: number): string {
  return `${Math.round(value)}%`
}

/** "in 3h 05m" / "in 42m" / "Mon 14:00" (beyond a day). */
export function formatReset(resetsAt: string, now: number = Date.now()): string {
  const at = Date.parse(resetsAt)
  if (!Number.isFinite(at)) return ''
  const mins = Math.max(0, Math.round((at - now) / 60_000))
  if (mins < 60) return `in ${mins}m`
  if (mins < 24 * 60) return `in ${Math.floor(mins / 60)}h ${String(mins % 60).padStart(2, '0')}m`
  const d = new Date(at)
  const day = d.toLocaleDateString('en-US', { weekday: 'short' })
  return `${day} ${String(d.getHours()).padStart(2, '0')}:${String(d.getMinutes()).padStart(2, '0')}`
}

export type UsageLevel = 'ok' | 'warn' | 'high'

export function usageLevel(percent: number): UsageLevel {
  if (percent >= 90) return 'high'
  if (percent >= 70) return 'warn'
  return 'ok'
}

export interface UsageItem {
  key: string
  label: string
  percent: number
  reset: string
  title: string
  level: UsageLevel
}

/** Items in display order: 5h, 7d, then context. Expired windows are skipped. */
export function usageItems(usage: SessionUsage | undefined, now: number = Date.now()): UsageItem[] {
  if (!usage) return []
  const items: UsageItem[] = []
  for (const w of usage.windows) {
    if (Date.parse(w.resetsAt) <= now) continue
    const label = SHORT[w.id] ?? w.label
    const reset = formatReset(w.resetsAt, now)
    items.push({
      key: w.id,
      label,
      percent: w.percent,
      reset,
      level: usageLevel(w.percent),
      title: `${w.label} limit: ${formatPercent(w.percent)} used, resets ${new Date(w.resetsAt).toLocaleString()}`,
    })
  }
  if (usage.contextPercent !== null) {
    const size = usage.contextWindowSize ? ` of ${Math.round(usage.contextWindowSize / 1000)}k tokens` : ''
    items.push({
      key: 'context',
      label: 'ctx',
      percent: usage.contextPercent,
      reset: '',
      level: usageLevel(usage.contextPercent),
      title: `Context window: ${formatPercent(usage.contextPercent)} used${size}`,
    })
  }
  return items
}

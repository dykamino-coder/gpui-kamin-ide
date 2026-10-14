import type { JSX } from 'preact'
import { useEffect, useLayoutEffect, useRef, useState } from 'preact/hooks'
import { activeTabId } from '../../signals/tabs'
import { usageByTab } from '../../signals/session-usage'
import { formatPercent, usageItems } from '../../lib/usage-format'
import type { UsageLevel } from '../../lib/usage-format'
import { SessionStats } from '../chat-header/SessionStats'
import { CHIP_LABEL_STYLE, CHIP_STYLE, METER_TRACK_STYLE, meterFillStyle } from './usage-chip'

const FILL: Record<UsageLevel, string> = {
  ok: 'var(--accent-green)',
  warn: 'var(--accent-yellow)',
  high: 'var(--accent-red)',
}

const VALUE: Record<UsageLevel, string> = {
  ok: 'var(--text-secondary)',
  warn: 'var(--accent-yellow)',
  high: 'var(--accent-red)',
}

/** Row under the chat input: 5-hour and 7-day subscription limits (% used,
 *  time to reset) from the CLI status line, then the session's context fill,
 *  tokens, cost and the Compact button (SessionStats). Each value is a pill
 *  with a thin meter coloured by level; when the pills do not fit on one line
 *  the meters are dropped (the coloured numbers stay) and come back once the
 *  row is wide enough again. API-key sessions never get `rate_limits`, so they
 *  show only the session pills. */
export function UsageLimitsRow(): JSX.Element | null {
  const tabId = activeTabId.value
  const usage = tabId ? usageByTab.value.get(tabId) : undefined
  // Re-render each minute so "in 42m" counts down between snapshots.
  const [now, setNow] = useState(() => Date.now())
  useEffect(() => {
    if (!usage) return
    const timer = setInterval(() => setNow(Date.now()), 60_000)
    return () => clearInterval(timer)
  }, [usage])

  const rowRef = useRef<HTMLDivElement>(null)
  const [meters, setMeters] = useState(true)
  // Width the row needed the last time it was measured WITH meters.
  const fullWidth = useRef(0)
  const fit = (): void => {
    const row = rowRef.current
    if (!row) return
    if (meters) {
      fullWidth.current = row.scrollWidth
      if (row.scrollWidth > row.clientWidth) setMeters(false)
    } else if (row.clientWidth >= fullWidth.current) {
      setMeters(true)
    }
  }
  // After every render: the numbers and the Compact pill change the width.
  useLayoutEffect(fit)
  useEffect(() => {
    const row = rowRef.current
    if (!row || typeof ResizeObserver === 'undefined') return
    const ro = new ResizeObserver(fit)
    ro.observe(row)
    return () => ro.disconnect()
  })

  if (!tabId) return null
  // Context comes from SessionStats (tokens + compact action); the CLI's own
  // context percentage would only duplicate it.
  const items = usageItems(usage, now).filter(item => item.key !== 'context')

  return (
    <div
      ref={rowRef}
      class="usage-limits-row"
      data-testid="usage-limits-row"
      style={`
        display:flex;align-items:center;gap:6px;
        padding:5px 10px 0;
        max-width:800px;width:100%;margin:0 auto;box-sizing:border-box;
        font-size:10px;line-height:1;color:var(--text-muted);
        font-family:ui-monospace, 'Cascadia Code', Menlo, Consolas, monospace;
        font-variant-numeric:tabular-nums;
        white-space:nowrap;overflow:hidden;
      `}
    >
      {items.map(item => (
        <span key={item.key} data-tooltip={item.title} title={item.title} style={CHIP_STYLE}>
          <span style={CHIP_LABEL_STYLE}>{item.label}</span>
          {meters && (
            <span aria-hidden="true" style={METER_TRACK_STYLE}>
              <span style={meterFillStyle(item.percent, FILL[item.level])} />
            </span>
          )}
          <span style={`color:${meters ? VALUE[item.level] : FILL[item.level]};font-weight:600`}>{formatPercent(item.percent)}</span>
          {item.reset && (
            <span style="color:var(--text-disabled);overflow:hidden;text-overflow:ellipsis">{item.reset}</span>
          )}
        </span>
      ))}
      <SessionStats variant="pills" meters={meters} />
    </div>
  )
}

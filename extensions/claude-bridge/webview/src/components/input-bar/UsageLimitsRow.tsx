import type { JSX } from 'preact'
import { useEffect, useState } from 'preact/hooks'
import { activeTabId } from '../../signals/tabs'
import { usageByTab } from '../../signals/session-usage'
import { formatPercent, usageItems } from '../../lib/usage-format'
import type { UsageLevel } from '../../lib/usage-format'

const COLOR: Record<UsageLevel, string> = {
  ok: 'var(--text-muted)',
  warn: 'var(--accent-yellow)',
  high: 'var(--accent-red)',
}

/** Compact row under the chat input: 5-hour and 7-day subscription limits
 *  (% used, time to reset) and context fill, from the CLI status line. Hidden
 *  until the first snapshot (API-key sessions never get `rate_limits`, but
 *  still show context). */
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
  const items = usageItems(usage, now)
  if (items.length === 0) return null

  return (
    <div
      class="usage-limits-row"
      data-testid="usage-limits-row"
      style={`
        display:flex;align-items:center;gap:12px;
        padding:2px 10px 0;
        max-width:800px;width:100%;margin:0 auto;box-sizing:border-box;
        font-size:10px;color:var(--text-muted);
        font-family:ui-monospace, 'Cascadia Code', Menlo, Consolas, monospace;
        white-space:nowrap;overflow:hidden;opacity:0.85;
      `}
    >
      {items.map(item => (
        <span key={item.key} data-tooltip={item.title} title={item.title} style="display:inline-flex;gap:4px;min-width:0">
          <span style="color:var(--text-disabled)">{item.label}</span>
          <span style={`color:${COLOR[item.level]}`}>{formatPercent(item.percent)}</span>
          {item.reset && <span style="overflow:hidden;text-overflow:ellipsis">· {item.reset}</span>}
        </span>
      ))}
    </div>
  )
}

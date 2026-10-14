import type { JSX } from 'preact'
import { useEffect, useState } from 'preact/hooks'
import { activeTabId } from '../../signals/tabs'
import { usageByTab } from '../../signals/session-usage'
import { formatPercent, usageItems } from '../../lib/usage-format'
import type { UsageLevel } from '../../lib/usage-format'

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

const CHIP = `
  display:inline-flex;align-items:center;gap:6px;min-width:0;
  height:18px;padding:0 8px;border-radius:999px;box-sizing:border-box;
  background:color-mix(in srgb, var(--bg-overlay) 55%, transparent);
  border:1px solid color-mix(in srgb, var(--text-muted) 16%, transparent);
`

/** Compact row under the chat input: 5-hour and 7-day subscription limits
 *  (% used, time to reset) and context fill, from the CLI status line. Each
 *  value is a pill with a thin meter coloured by level. Hidden until the first
 *  snapshot (API-key sessions never get `rate_limits`, but still show context). */
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
        display:flex;align-items:center;gap:6px;
        padding:5px 10px 0;
        max-width:800px;width:100%;margin:0 auto;box-sizing:border-box;
        font-size:10px;line-height:1;color:var(--text-muted);
        font-family:ui-monospace, 'Cascadia Code', Menlo, Consolas, monospace;
        font-variant-numeric:tabular-nums;
        white-space:nowrap;overflow:hidden;
      `}
    >
      {items.map(item => {
        const fill = Math.max(0, Math.min(100, item.percent))
        return (
          <span key={item.key} data-tooltip={item.title} title={item.title} style={CHIP}>
            <span style="color:var(--text-muted);letter-spacing:0.04em;text-transform:uppercase;font-size:9px">
              {item.label}
            </span>
            <span
              aria-hidden="true"
              style="position:relative;width:34px;height:4px;flex:none;border-radius:2px;overflow:hidden;background:color-mix(in srgb, var(--text-muted) 22%, transparent)"
            >
              <span
                style={`position:absolute;left:0;top:0;bottom:0;width:${fill}%;border-radius:2px;background:${FILL[item.level]};transition:width .4s ease`}
              />
            </span>
            <span style={`color:${VALUE[item.level]};font-weight:600`}>{formatPercent(item.percent)}</span>
            {item.reset && (
              <span style="color:var(--text-disabled);overflow:hidden;text-overflow:ellipsis">{item.reset}</span>
            )}
          </span>
        )
      })}
    </div>
  )
}

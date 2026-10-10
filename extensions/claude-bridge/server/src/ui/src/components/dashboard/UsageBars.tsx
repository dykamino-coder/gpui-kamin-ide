import type { UsageData } from '../../services/api-client'
import type { UsageWindow } from '../../../../shared/plan-usage'
import styles from './UsageBars.module.css'

interface UsageBarsProps {
  usage: UsageData | null
  loading?: boolean
}
function UsageBar({ window }: { window: UsageWindow }) {
  const expired = !!window.resetsAt && Date.parse(window.resetsAt) <= Date.now()
  const stale = window.freshness === 'stale' || expired
  const color = stale
    ? 'var(--text-muted)'
    : window.percent >= 90
      ? 'var(--accent-red)'
      : window.percent >= 70
        ? 'var(--accent-yellow)'
        : 'var(--accent-green)'
  const reset = window.resetsAt ? new Date(window.resetsAt).toLocaleString() : (window.resetText ?? 'unknown')
  return (
    <div class={styles.barRow} title={`Observed ${window.observedAt}; ${window.reason ?? window.source}`}>
      <div class={styles.barHeader}>
        <span class={styles.barLabel}>
          {window.label} <span class={styles.barResets}>— Resets {reset}</span>
        </span>
        <span class={styles.barPercent} style={{ color }}>
          {stale ? 'Stale · ' : ''}
          {window.percent}%
        </span>
      </div>
      <div class={styles.barTrack}>
        <div
          class={styles.barFill}
          style={{ width: `${Math.min(100, Math.max(0, window.percent))}%`, background: color }}
        />
      </div>
    </div>
  )
}
export function UsageBars({ usage, loading }: UsageBarsProps) {
  if (!usage) return loading ? <div class={styles.container}>Loading usage…</div> : null
  return (
    <div class={styles.container}>
      {loading && <div class={styles.loading}>Refreshing usage…</div>}
      {usage.state !== 'complete' && (
        <div class={styles.extra}>
          {usage.state === 'unavailable' ? 'Usage unavailable' : 'Partial usage'}
          {usage.windows.some((window) => window.freshness === 'stale') ? ' · last known rows retained' : ''}
        </div>
      )}
      {usage.windows.map((window) => (
        <UsageBar key={window.id} window={window} />
      ))}
      <div class={styles.extra}>
        Observed {new Date(usage.observedAt).toLocaleString()}
        {usage.claudeCodeVersion ? ` · Claude Code ${usage.claudeCodeVersion}` : ''}
      </div>
      {usage.reason && usage.reason !== 'reset-timestamp-unavailable' && (
        <div class={styles.extra}>
          {usage.reason === 'capture-error'
            ? 'Refresh failed; try again.'
            : usage.reason === 'output-limit'
              ? 'Capture exceeded its output limit.'
              : 'Some usage metadata is not reported.'}
        </div>
      )}
      {usage.windows.some((window) => !window.resetsAt) && (
        <div class={styles.extra}>Exact reset timestamp unavailable; CLI reset label shown.</div>
      )}
      {usage.extra && <div class={styles.extra}>Extra: {usage.extra}</div>}
    </div>
  )
}

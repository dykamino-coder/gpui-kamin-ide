// ChatGPT plan windows of the codex account, drawn like Claude's usage bars
// (UsageBars.tsx): percent used and when the window resets. Only windows
// codex reports are shown; nothing is estimated from token counts.

import { limitWindowLabel, type CodexLimitWindow, type CodexRateLimits } from '../../../../shared/codex-rate-limits'
import styles from './UsageBars.module.css'

function LimitBar({ label, window }: { label: string; window: CodexLimitWindow }) {
  const expired = window.resetsAt !== null && window.resetsAt <= Date.now()
  const color = expired
    ? 'var(--text-muted)'
    : window.usedPercent >= 90
      ? 'var(--accent-red)'
      : window.usedPercent >= 70
        ? 'var(--accent-yellow)'
        : 'var(--accent-green)'
  const reset = window.resetsAt ? new Date(window.resetsAt).toLocaleString() : 'unknown'
  return (
    <div class={styles.barRow}>
      <div class={styles.barHeader}>
        <span class={styles.barLabel}>
          {label} <span class={styles.barResets}>— Resets {reset}</span>
        </span>
        <span class={styles.barPercent} style={{ color }}>
          {expired ? 'Reset · ' : ''}
          {window.usedPercent}%
        </span>
      </div>
      <div class={styles.barTrack}>
        <div class={styles.barFill} style={{ width: `${window.usedPercent}%`, background: color }} />
      </div>
    </div>
  )
}

export function CodexLimitBars({ limits }: { limits: CodexRateLimits }) {
  const several = limits.buckets.length > 1
  return (
    <div class={styles.container}>
      {limits.buckets.length === 0 && <div class={styles.extra}>The ChatGPT plan reports no usage limits.</div>}
      {limits.buckets.map((b, i) => {
        const prefix = several ? `${b.name ?? b.id ?? 'Limit'} · ` : ''
        return (
          <div key={b.id ?? i}>
            {b.primary && (
              <LimitBar label={prefix + limitWindowLabel(b.primary.windowMinutes, 'Short limit')} window={b.primary} />
            )}
            {b.secondary && (
              <LimitBar
                label={prefix + limitWindowLabel(b.secondary.windowMinutes, 'Long limit')}
                window={b.secondary}
              />
            )}
            {b.reached && <div class={styles.extra}>Limit reached: {b.reached}</div>}
          </div>
        )
      })}
      {limits.usageAllowed === false && (
        <div class={styles.extra}>The plan does not allow ordinary usage right now.</div>
      )}
      {limits.credits && !limits.credits.unlimited && limits.credits.hasCredits && limits.credits.balance && (
        <div class={styles.extra}>Credits: {limits.credits.balance}</div>
      )}
      <div class={styles.extra}>Observed {new Date(limits.readAt).toLocaleString()}</div>
    </div>
  )
}

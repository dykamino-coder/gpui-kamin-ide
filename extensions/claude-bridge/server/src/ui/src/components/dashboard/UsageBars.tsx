import type { UsageData } from '../../services/api-client'
import styles from './UsageBars.module.css'

interface UsageBarsProps {
  usage: UsageData | null
  loading?: boolean
}

function barColor(percent: number): string {
  if (percent >= 90) return 'var(--accent-red)'
  if (percent >= 70) return 'var(--accent-yellow)'
  return 'var(--accent-green)'
}

function UsageBar({ label, percent, resets }: { label: string; percent: number; resets: string }) {
  return (
    <div class={styles.barRow}>
      <div class={styles.barHeader}>
        <span class={styles.barLabel}>{label} <span class={styles.barResets}>- Resets {resets}</span></span>
        <span class={styles.barPercent} style={{ color: barColor(percent) }}>{percent}%</span>
      </div>
      <div class={styles.barTrack}>
        <div
          class={styles.barFill}
          style={{ width: `${Math.min(percent, 100)}%`, background: barColor(percent) }}
        />
      </div>
    </div>
  )
}

export function UsageBars({ usage, loading }: UsageBarsProps) {
  if (loading) {
    return (
      <div class={styles.container}>
        <div class={styles.loading}>
          <i class="fa-solid fa-spinner fa-spin" style={{ marginRight: '6px' }} />
          Loading usage...
        </div>
      </div>
    )
  }

  if (!usage || (!usage.session && !usage.weekAll && !usage.weekSonnet)) {
    return null
  }

  return (
    <div class={styles.container}>
      {usage.session && (
        <UsageBar label="Session" percent={usage.session.percent} resets={usage.session.resets} />
      )}
      {usage.weekAll && (
        <UsageBar label="Week (all)" percent={usage.weekAll.percent} resets={usage.weekAll.resets} />
      )}
      {usage.weekSonnet && (
        <UsageBar label="Week (Sonnet)" percent={usage.weekSonnet.percent} resets={usage.weekSonnet.resets} />
      )}
      {usage.extra && (
        <div class={styles.extra}>Extra: {usage.extra}</div>
      )}
    </div>
  )
}

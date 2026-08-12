import type { ExtendedStats } from '../../types'
import { formatLastAt, formatTokens } from '../../utils/formatters'
import styles from './StatsGrid.module.css'

export interface StatFilter {
  label: string
  endpoint?: string
  pluginId?: string
  userName?: string
  status?: string
}

interface StatItem {
  label: string
  value: string | number
  subValue?: string | number
  icon: string
  color: string
  filter?: StatFilter
  lastAt?: string
  tokens?: { input: number; output: number }
}

interface StatsGridProps {
  stats: ExtendedStats | null
  onStatClick?: (filter: StatFilter) => void
  onDelete?: (filter: StatFilter) => void
}

function dualValue(userMsgs: number, total: number): { value: number; subValue?: number } {
  if (userMsgs > 0 && userMsgs < total) return { value: userMsgs, subValue: total }
  return { value: total }
}

export function StatsGrid({ stats, onStatClick, onDelete }: StatsGridProps) {
  const lrt = stats?.lastRequestTimes ?? {}

  const apiItems: StatItem[] = [
    { label: 'Requests', value: stats?.requests ?? 0, icon: 'fa-solid fa-arrow-right-arrow-left', color: 'var(--accent-blue)', filter: { label: 'All Requests' }, lastAt: lrt['total'] },
    { label: 'Errors', value: stats?.errors ?? 0, icon: 'fa-solid fa-triangle-exclamation', color: 'var(--accent-red)', filter: { label: 'Errors', status: 'error' }, lastAt: lrt['errors'] },
  ]

  const handleClick = (item: StatItem) => {
    if (item.filter && onStatClick) onStatClick(item.filter)
  }

  const handleDelete = (e: MouseEvent, item: StatItem) => {
    e.stopPropagation()
    if (!item.filter || !onDelete) return
    if (!confirm(`Delete all "${item.label}" requests?`)) return
    onDelete(item.filter)
  }

  const renderCard = (item: StatItem, iconEl: any) => {
    const time = formatLastAt(item.lastAt)
    const showDelete = onDelete && item.filter && Number(item.value) > 0
    return (
      <div
        key={item.label}
        class={`${styles.stat} ${item.filter && onStatClick ? styles.clickable : ''}`}
        onClick={() => handleClick(item)}
      >
        <div class={styles.statIcon} style={{ color: item.color }}>{iconEl}</div>
        <div class={styles.statContent}>
          <span class={styles.statValue}>
            {item.value}
            {item.subValue != null && <span class={styles.statSubValue}> ({item.subValue})</span>}
          </span>
          <span class={styles.statLabel}>{item.label}</span>
          {item.tokens && (item.tokens.input > 0 || item.tokens.output > 0) && (
            <span class={styles.statTokens}>
              {formatTokens(item.tokens.input)} in / {formatTokens(item.tokens.output)} out
            </span>
          )}
          {time && <span class={styles.statTime}>{time}</span>}
        </div>
        {showDelete && (
          <button
            class={styles.deleteBtn}
            onClick={(e: any) => handleDelete(e, item)}
            title={`Delete ${item.label} requests`}
          >
            <i class="fa-solid fa-trash-can" />
          </button>
        )}
      </div>
    )
  }

  return (
    <div class={styles.wrapper}>
      <div class={styles.section}>
        <div class={styles.sectionHeader}>
          <i class={`fa-solid fa-chart-bar ${styles.sectionIcon}`} />
          <span class={styles.sectionTitle}>Requests</span>
          <span class={styles.sectionCount}>{stats?.requests ?? 0}</span>
        </div>
        <div class={styles.grid}>
          {apiItems.map((item) => renderCard(item, <i class={item.icon} />))}
        </div>
      </div>
    </div>
  )
}

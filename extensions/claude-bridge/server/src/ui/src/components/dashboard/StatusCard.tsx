import type { ComponentChildren } from 'preact'
import { Card } from '../shared'
import styles from './StatusCard.module.css'

interface StatusCardProps {
  icon: string
  iconColor?: string
  title: string
  items: { label: string; value: string; color?: string; truncate?: boolean; onClick?: () => void }[]
  status?: 'ok' | 'warning' | 'error'
  action?: { icon: string; label: string; onClick: () => void; loading?: boolean; hint?: string }
  children?: ComponentChildren
}

export function StatusCard({ icon, iconColor, title, items, status = 'ok', action, children }: StatusCardProps) {
  return (
    <Card className={styles.statusCard}>
      <div class={styles.cardHeader}>
        <div class={styles.iconWrap} style={iconColor ? { color: iconColor } : undefined}>
          <i class={icon} />
        </div>
        <span class={styles.cardTitle}>{title}</span>
        {action && (
          <>
            {action.hint && <span class={styles.actionHint}>{action.hint}</span>}
            <button class={styles.actionBtn} onClick={action.onClick} title={action.label} disabled={action.loading}>
              <i class={action.loading ? 'fa-solid fa-spinner fa-spin' : action.icon} />
            </button>
          </>
        )}
        <span class={`${styles.statusDot} ${styles[status]}`} />
      </div>
      <div class={styles.items}>
        {items.map((item, i) => (
          <div key={i} class={styles.item}>
            <span class={styles.itemLabel}>{item.label}</span>
            <span
              class={`${styles.itemValue} ${item.truncate ? styles.truncated : ''} ${item.onClick ? styles.clickable : ''}`}
              style={item.color ? { color: item.color } : undefined}
              onClick={item.onClick}
              title={item.truncate ? item.value : undefined}
            >
              {item.value}
            </span>
          </div>
        ))}
      </div>
      {children}
    </Card>
  )
}

import { ComponentChildren } from 'preact'
import styles from './SectionHeader.module.css'

interface SectionHeaderProps {
  title: string
  icon?: string
  subtitle?: string
  action?: ComponentChildren
}

export function SectionHeader({ title, icon, subtitle, action }: SectionHeaderProps) {
  return (
    <div class={styles.sectionHeader}>
      <div class={styles.left}>
        {icon && <i class={`${icon} ${styles.icon}`} />}
        <span class={styles.title}>{title}</span>
        {subtitle && <span class={styles.subtitle}>{subtitle}</span>}
      </div>
      {action && <div class={styles.action}>{action}</div>}
    </div>
  )
}

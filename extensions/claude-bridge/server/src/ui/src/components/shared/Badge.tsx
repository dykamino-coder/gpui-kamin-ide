import { ComponentChildren } from 'preact'
import styles from './Badge.module.css'

export type BadgeVariant = 'default' | 'success' | 'warning' | 'error' | 'info' | 'anthropic' | 'openai'

interface BadgeProps {
  children: ComponentChildren
  variant?: BadgeVariant
  size?: 'sm' | 'md'
  dot?: boolean
  className?: string
}

export function Badge({
  children,
  variant = 'default',
  size = 'md',
  dot = false,
  className = '',
}: BadgeProps) {
  const cls = [styles.badge, styles[variant], styles[size], className].filter(Boolean).join(' ')
  return (
    <span class={cls}>
      {dot && <span class={styles.dot} />}
      {children}
    </span>
  )
}

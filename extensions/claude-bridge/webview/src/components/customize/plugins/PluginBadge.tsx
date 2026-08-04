import type { JSX } from 'preact'
import styles from './PluginBadge.module.css'

interface Props {
  icon: string
  label: string
}

export function PluginBadge({ icon, label }: Props): JSX.Element {
  return (
    <span class={styles.badge}>
      <i class={icon} />
      {label}
    </span>
  )
}

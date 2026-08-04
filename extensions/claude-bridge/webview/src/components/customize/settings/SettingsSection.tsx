import type { JSX, ComponentChildren } from 'preact'
import styles from './SettingsSection.module.css'

interface Props {
  title: string
  children: ComponentChildren
}

export function SettingsSection({ title, children }: Props): JSX.Element {
  return (
    <div class={styles.section}>
      <h3 class={styles.title}>{title}</h3>
      {children}
    </div>
  )
}

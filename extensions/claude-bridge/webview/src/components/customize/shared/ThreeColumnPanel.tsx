import type { JSX, ComponentChildren } from 'preact'
import styles from './ThreeColumnPanel.module.css'

interface Props {
  children: ComponentChildren
}

export function ThreeColumnPanel({ children }: Props): JSX.Element {
  return <div class={styles.panel}>{children}</div>
}

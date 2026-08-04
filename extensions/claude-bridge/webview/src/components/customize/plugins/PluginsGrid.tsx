import type { JSX, ComponentChildren } from 'preact'
import styles from './PluginsGrid.module.css'

interface Props {
  children: ComponentChildren
}

export function PluginsGrid({ children }: Props): JSX.Element {
  return <div class={styles.grid}>{children}</div>
}

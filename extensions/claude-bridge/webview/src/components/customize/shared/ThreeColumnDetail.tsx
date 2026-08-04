import type { JSX, ComponentChildren } from 'preact'
import styles from './ThreeColumnDetail.module.css'

interface Props {
  children: ComponentChildren
}

export function ThreeColumnDetail({ children }: Props): JSX.Element {
  return <div class={styles.detail}>{children}</div>
}

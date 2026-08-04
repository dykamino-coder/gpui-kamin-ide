import type { JSX } from 'preact'
import styles from './ConnectorToolItem.module.css'

interface Props {
  name: string
  description?: string
}

export function ConnectorToolItem({ name, description }: Props): JSX.Element {
  return (
    <div class={styles.item} data-tooltip={description}>
      <div class={styles.name}>{name}</div>
      {description && <div class={styles.desc}>{description}</div>}
    </div>
  )
}

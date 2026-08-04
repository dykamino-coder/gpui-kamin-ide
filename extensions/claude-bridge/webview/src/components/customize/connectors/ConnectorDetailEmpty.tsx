import type { JSX } from 'preact'
import styles from './ConnectorDetailEmpty.module.css'

export function ConnectorDetailEmpty(): JSX.Element {
  return (
    <div class={styles.empty}>
      Select a connector to view details
    </div>
  )
}

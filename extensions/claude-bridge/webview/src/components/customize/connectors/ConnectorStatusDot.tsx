import type { JSX } from 'preact'
import styles from './ConnectorStatusDot.module.css'

type Status = 'connected' | 'disconnected' | 'error' | 'testing' | 'connecting'

interface Props {
  status: Status
}

export function ConnectorStatusDot({ status }: Props): JSX.Element {
  const cls = status === 'connecting' ? 'testing' : status
  return <span class={`${styles.dot} ${styles[cls] ?? ''}`} />
}

import type { JSX, ComponentChildren } from 'preact'
import styles from './ConnectorGroup.module.css'

interface Props {
  title: string
  children: ComponentChildren
}

export function ConnectorGroup({ title, children }: Props): JSX.Element {
  return (
    <>
      <div class={styles.header}>{title}</div>
      {children}
    </>
  )
}

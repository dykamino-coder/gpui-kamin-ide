import type { JSX, ComponentChildren } from 'preact'
import styles from './ContentArea.module.css'

interface ContentAreaProps {
  jsonlPanel: ComponentChildren
  resizeHandle: ComponentChildren
  terminalPanel: ComponentChildren
}

export function ContentArea({ jsonlPanel, resizeHandle, terminalPanel }: ContentAreaProps): JSX.Element {
  return (
    <div class={styles.contentArea} id="content-area">
      {jsonlPanel}
      {resizeHandle}
      {terminalPanel}
    </div>
  )
}

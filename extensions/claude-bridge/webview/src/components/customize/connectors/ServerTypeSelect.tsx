import type { JSX } from 'preact'
import styles from './ServerTypeSelect.module.css'

type ServerType = 'stdio' | 'http' | 'sse' | 'ws'

interface Props {
  value: ServerType
  onChange: (value: ServerType) => void
}

export function ServerTypeSelect({ value, onChange }: Props): JSX.Element {
  return (
    <>
      <label class={styles.label}>Type</label>
      <select
        class={styles.select}
        value={value}
        onChange={(e) => onChange((e.target as HTMLSelectElement).value as ServerType)}
      >
        <option value="stdio">stdio (Local process)</option>
        <option value="http">HTTP (Streamable HTTP)</option>
        <option value="sse">SSE (Server-Sent Events)</option>
        <option value="ws">WebSocket</option>
      </select>
    </>
  )
}

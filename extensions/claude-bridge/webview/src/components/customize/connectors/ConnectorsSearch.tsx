import type { JSX } from 'preact'
import styles from './ConnectorsSearch.module.css'

interface Props {
  value: string
  onChange: (value: string) => void
}

export function ConnectorsSearch({ value, onChange }: Props): JSX.Element {
  return (
    <input
      class={styles.search}
      type="text"
      placeholder="Search connectors..."
      value={value}
      onInput={(e) => onChange((e.target as HTMLInputElement).value)}
    />
  )
}

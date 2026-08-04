import type { JSX } from 'preact'
import styles from './PluginsSearch.module.css'

interface Props {
  value: string
  onChange: (value: string) => void
}

export function PluginsSearch({ value, onChange }: Props): JSX.Element {
  return (
    <div class={styles.wrapper}>
      <i class={`fas fa-search ${styles.icon}`} />
      <input
        type="text"
        class={styles.input}
        placeholder="Search"
        value={value}
        onInput={(e) => onChange((e.target as HTMLInputElement).value)}
      />
    </div>
  )
}

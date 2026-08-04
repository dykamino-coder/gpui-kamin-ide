import type { JSX } from 'preact'
import styles from './SkillsSearch.module.css'

interface Props {
  value: string
  onChange: (value: string) => void
}

export function SkillsSearch({ value, onChange }: Props): JSX.Element {
  return (
    <input
      class={styles.search}
      type="text"
      placeholder="Search skills..."
      value={value}
      onInput={(e) => onChange((e.target as HTMLInputElement).value)}
    />
  )
}

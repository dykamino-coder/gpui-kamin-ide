import type { JSX } from 'preact'
import styles from './ThreeColumnListFooter.module.css'

interface Props {
  label: string
  onClick: () => void
  icon?: string
}

export function ThreeColumnListFooter({ label, onClick, icon = 'fas fa-plus' }: Props): JSX.Element {
  return (
    <button class={styles.btn} onClick={onClick}>
      <i class={icon} />
      {label}
    </button>
  )
}

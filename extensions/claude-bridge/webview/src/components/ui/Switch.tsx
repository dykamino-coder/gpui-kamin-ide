import type { JSX, ComponentChildren } from 'preact'
import styles from './Switch.module.css'

interface Props {
  checked: boolean
  onChange: (v: boolean) => void
  /** Primary label — the switch's accessible name. */
  label: ComponentChildren
  /** Optional secondary line under the label. */
  description?: ComponentChildren
  disabled?: boolean
}

/** Accessible toggle switch (role=switch). The whole row is one button, so it's
 *  keyboard-operable and has a single hit target — the premium replacement for a
 *  raw checkbox + hand-rolled label markup. */
export function Switch({ checked, onChange, label, description, disabled }: Props): JSX.Element {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      disabled={disabled}
      onClick={() => { if (!disabled) onChange(!checked) }}
      class={`${styles.row}${disabled ? ` ${styles.disabled}` : ''}`}
    >
      <span class={`${styles.track}${checked ? ` ${styles.on}` : ''}`}>
        <span class={styles.thumb} />
      </span>
      <span class={styles.body}>
        <span class={styles.label}>{label}</span>
        {description ? <span class={styles.desc}>{description}</span> : null}
      </span>
    </button>
  )
}

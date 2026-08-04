import type { JSX, ComponentChildren } from 'preact'
import styles from './Button.module.css'

type Variant = 'primary' | 'secondary' | 'danger' | 'ghost'

interface Props {
  children: ComponentChildren
  variant?: Variant
  type?: 'button' | 'submit'
  disabled?: boolean
  onClick?: () => void
  /** FontAwesome class WITHOUT the `fa-` prefix, e.g. 'fa-plus'. */
  icon?: string
  title?: string
  class?: string
}

/** The one button hierarchy: primary (affirmative) / secondary (neutral) /
 *  danger (subtle destructive) / ghost (tertiary). Replaces the per-form
 *  hand-rolled button chrome that drifted across the app. */
export function Button({ children, variant = 'secondary', type = 'button', disabled, onClick, icon, title, class: extra }: Props): JSX.Element {
  return (
    <button
      type={type}
      disabled={disabled}
      onClick={onClick}
      title={title}
      class={`${styles.btn} ${styles[variant]}${extra ? ` ${extra}` : ''}`}
    >
      {icon ? <i class={`fas ${icon} ${styles.icon}`} aria-hidden="true" /> : null}
      {children}
    </button>
  )
}

import type { JSX } from 'preact'
import styles from './TitlebarButton.module.css'

interface TitlebarButtonProps {
  icon: string
  onClick: () => void
  variant?: 'close' | 'devtools' | 'default'
  label?: string
  className?: string
}

export function TitlebarButton({ icon, onClick, variant = 'default', label, className }: TitlebarButtonProps): JSX.Element {
  const cls = [
    styles.btn,
    variant === 'close' ? styles.close : '',
    variant === 'devtools' ? styles.devtools : '',
    className ?? '',
  ].filter(Boolean).join(' ')

  return (
    <button class={cls} onClick={onClick} title={label} type="button">
      <i class={icon} />
      {variant === 'devtools' && label && <span>{label}</span>}
    </button>
  )
}

import { ComponentChildren } from 'preact'
import styles from './Card.module.css'

interface CardProps {
  children: ComponentChildren
  noPadding?: boolean
  hoverable?: boolean
  clickable?: boolean
  selected?: boolean
  className?: string
  onClick?: (e: MouseEvent) => void
}

export function Card({
  children,
  noPadding = false,
  hoverable = false,
  clickable = false,
  selected = false,
  className = '',
  onClick,
}: CardProps) {
  const classNames = [
    styles.card,
    noPadding && styles.noPadding,
    hoverable && styles.hoverable,
    clickable && styles.clickable,
    selected && styles.selected,
    className,
  ]
    .filter(Boolean)
    .join(' ')

  return (
    <div class={classNames} onClick={onClick}>
      {children}
    </div>
  )
}

import type { JSX, ComponentChildren } from 'preact'
import styles from './ThreeColumnList.module.css'

interface Props {
  title: string
  searchValue: string
  onSearchChange: (value: string) => void
  searchPlaceholder?: string
  children: ComponentChildren
  footer?: ComponentChildren
}

export function ThreeColumnList({
  title,
  searchValue,
  onSearchChange,
  searchPlaceholder = 'Search...',
  children,
  footer,
}: Props): JSX.Element {
  return (
    <div class={styles.list}>
      <div class={styles.header}>
        <h3>{title}</h3>
        <input
          type="text"
          class={styles.search}
          placeholder={searchPlaceholder}
          value={searchValue}
          onInput={(e) => onSearchChange((e.target as HTMLInputElement).value)}
        />
      </div>
      <div class={styles.items}>{children}</div>
      {footer && <div class={styles.footer}>{footer}</div>}
    </div>
  )
}

// Single tab chip — title, status dot, pin button, close button.
// Extracted from TabsBar.tsx (Sprint 5 / Stage E1). Pure presentational
// over a TabInfo + the small set of UI state flags the parent computes.

import type { JSX } from 'preact'
import type { TabInfo } from '../../../shared/types'
import styles from './TabsBar.module.css'

interface Props {
  tab: TabInfo
  isActive: boolean
  isPinned: boolean
  isSleeping: boolean
  isTinted: boolean
  isDragging: boolean
  isOverLeft: boolean
  isOverRight: boolean
  working: boolean
  color: string
  title: string
  tooltip: string
  onActivate: () => void
  onPointerDown: (e: PointerEvent) => void
  onContextMenu: (e: MouseEvent) => void
  onTogglePin: (e: MouseEvent) => void
  onClose: (e: MouseEvent) => void
}

export function TabChip({
  tab, isActive, isPinned, isSleeping, isTinted, isDragging, isOverLeft, isOverRight,
  working, color, title, tooltip,
  onActivate, onPointerDown, onContextMenu, onTogglePin, onClose,
}: Props): JSX.Element {
  const classes = [
    styles.tab,
    isActive ? styles.active : '',
    isPinned ? styles.pinned : '',
    isSleeping ? styles.sleeping : '',
    isTinted ? styles.tinted : '',
    isDragging ? styles.dndDragging : '',
    isOverLeft ? styles.dndOverLeft : '',
    isOverRight ? styles.dndOverRight : '',
  ].filter(Boolean).join(' ')

  return (
    <div
      data-tab-id={tab.id}
      class={classes}
      style={`--tab-color:${color}`}
      onClick={onActivate}
      role="tab"
      tabIndex={0}
      data-tooltip={tooltip}
      onPointerDown={onPointerDown}
      onContextMenu={onContextMenu}
    >
      <span class={styles.leading}>
        <span class={styles.dot} style={`background:${working ? 'var(--accent-yellow)' : color};${working ? 'animation:pulse-dot 1.2s infinite' : ''}`} />
        <button
          class={styles.pin}
          onClick={onTogglePin}
          aria-label={isPinned ? 'Unpin tab' : 'Pin tab'}
          title={isPinned ? 'Unpin tab' : 'Pin tab'}
        >
          <i class="fas fa-thumbtack" />
        </button>
      </span>
      <span class={styles.label}>{title}</span>
      <button
        class={styles.close}
        onClick={onClose}
        aria-label="Close tab"
        title="Close tab"
      >
        <i class="fas fa-xmark" />
      </button>
    </div>
  )
}

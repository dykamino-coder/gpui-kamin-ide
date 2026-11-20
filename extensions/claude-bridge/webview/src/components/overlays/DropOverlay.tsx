import type { JSX } from 'preact'
import styles from './DropOverlay.module.css'

interface DropOverlayProps {
  active: boolean
}

export function DropOverlay({ active }: DropOverlayProps): JSX.Element | null {
  if (!active) return null

  return (
    <div class={styles.dropOverlay}>
      <div class={styles.dropContent}>
        <i class="fas fa-paperclip" />
        <span>Drop files to attach</span>
      </div>
    </div>
  )
}

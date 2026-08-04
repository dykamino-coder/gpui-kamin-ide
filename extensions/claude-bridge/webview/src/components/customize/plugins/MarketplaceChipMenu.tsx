import type { JSX } from 'preact'
import { useRef, useEffect } from 'preact/hooks'
import styles from './MarketplaceChipMenu.module.css'

interface Props {
  visible: boolean
  autoUpdate: boolean
  onUpdate: () => void
  onToggleAutoUpdate: (next: boolean) => void
  onSetToken: () => void
  onRemove: () => void
  onClose: () => void
}

export function MarketplaceChipMenu({ visible, autoUpdate, onUpdate, onToggleAutoUpdate, onSetToken, onRemove, onClose }: Props): JSX.Element | null {
  const ref = useRef<HTMLDivElement>(null)

  useEffect(() => {
    if (!visible) return
    function handler(e: MouseEvent): void {
      if (ref.current && !ref.current.contains(e.target as Node)) {
        onClose()
      }
    }
    document.addEventListener('click', handler)
    return () => document.removeEventListener('click', handler)
  }, [visible, onClose])

  if (!visible) return null

  return (
    <div class={styles.dropdown} ref={ref}>
      <button class={styles.item} onClick={(e) => { e.stopPropagation(); onUpdate(); onClose() }}>
        <i class="fas fa-sync-alt" /> Check for updates
      </button>
      <label class={styles.item} style="cursor:pointer;display:flex;align-items:center;gap:8px;" onClick={(e) => e.stopPropagation()}>
        <input
          type="checkbox"
          checked={autoUpdate}
          onChange={(e) => onToggleAutoUpdate((e.target as HTMLInputElement).checked)}
        />
        <span>Auto-update on startup</span>
      </label>
      <button class={styles.item} onClick={(e) => { e.stopPropagation(); onSetToken(); onClose() }}>
        <i class="fas fa-key" /> Set access token
      </button>
      <button class={`${styles.item} ${styles.danger}`} onClick={(e) => { e.stopPropagation(); onRemove(); onClose() }}>
        <i class="fas fa-trash" /> Remove
      </button>
    </div>
  )
}

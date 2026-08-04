import type { JSX } from 'preact'
import styles from './MarketplaceAddDropdown.module.css'

interface Props {
  visible: boolean
  /** Viewport-anchored position computed by the parent via getBoundingClientRect.
   *  When null (first render before button is measured) the dropdown is not
   *  shown — parent always sets it before flipping `visible` to true. */
  anchor: { top: number; right: number } | null
  onGithub: () => void
  onUrl: () => void
  onLocal: () => void
}

export function MarketplaceAddDropdown({ visible, anchor, onGithub, onUrl, onLocal }: Props): JSX.Element | null {
  if (!visible || !anchor) return null

  // position:fixed anchored to viewport so parents with overflow:hidden
  // (e.g. sidebar / marketplace bar wrapper) don't clip the menu.
  const style = `top:${anchor.top}px;right:${anchor.right}px;`

  return (
    <div class={styles.dropdown} style={style}>
      <button class={styles.option} onClick={(e) => { e.stopPropagation(); onGithub() }}>
        <i class="fab fa-github" /> Add marketplace from GitHub
      </button>
      <button class={styles.option} onClick={(e) => { e.stopPropagation(); onUrl() }}>
        <i class="fas fa-link" /> Add marketplace by URL
      </button>
      <button class={styles.option} onClick={(e) => { e.stopPropagation(); onLocal() }}>
        <i class="fas fa-folder-open" /> Add plugin from local folder
      </button>
    </div>
  )
}

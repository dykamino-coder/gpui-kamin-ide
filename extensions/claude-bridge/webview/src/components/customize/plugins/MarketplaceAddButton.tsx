import type { JSX } from 'preact'
import { useState, useEffect, useRef } from 'preact/hooks'
import { MarketplaceAddDropdown } from './MarketplaceAddDropdown'
import styles from './MarketplaceAddButton.module.css'

interface Props {
  onGithubOrUrl: () => void
  onLocal: () => void
}

export function MarketplaceAddButton({ onGithubOrUrl, onLocal }: Props): JSX.Element {
  const [visible, setVisible] = useState(false)
  const [anchor, setAnchor] = useState<{ top: number; right: number } | null>(null)
  const btnRef = useRef<HTMLButtonElement>(null)

  useEffect(() => {
    function handler(): void { setVisible(false) }
    document.addEventListener('click', handler)
    return () => document.removeEventListener('click', handler)
  }, [])

  // Re-measure button position on each open AND on window resize while open.
  // The dropdown lives in the document root (via position: fixed) so nothing
  // up the tree can clip it — absolute positioning inside an `.overflow:hidden`
  // sidebar sibling was hiding the menu when no marketplaces had been added.
  useEffect(() => {
    if (!visible) return
    const measure = (): void => {
      const el = btnRef.current
      if (!el) return
      const r = el.getBoundingClientRect()
      setAnchor({ top: r.bottom + 4, right: window.innerWidth - r.right })
    }
    measure()
    window.addEventListener('resize', measure)
    window.addEventListener('scroll', measure, true)
    return () => {
      window.removeEventListener('resize', measure)
      window.removeEventListener('scroll', measure, true)
    }
  }, [visible])

  return (
    <div class={styles.wrapper}>
      <button
        ref={btnRef}
        class={styles.btn}
        onClick={(e) => { e.stopPropagation(); setVisible((v) => !v) }}
      >
        <i class="fas fa-plus" />
      </button>
      <MarketplaceAddDropdown
        visible={visible}
        anchor={anchor}
        onGithub={() => { setVisible(false); onGithubOrUrl() }}
        onUrl={() => { setVisible(false); onGithubOrUrl() }}
        onLocal={() => { setVisible(false); onLocal() }}
      />
    </div>
  )
}

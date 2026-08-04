// Right-click context menu for TabsBar (Sprint 5 / Stage E1). Owns its
// own dismiss-on-outside-click effect and the Color sub-row for pinned-
// title color overrides.

import type { JSX } from 'preact'
import { useEffect } from 'preact/hooks'
import { AGENT_PALETTE, PALETTE_TOKENS } from '../../utils/agent-color'
import { pinnedTitleColors, setPinnedTitleColor } from '../../signals/ui'
import type { TabInfo } from '../../../shared/types'
import styles from './TabsBar.module.css'

interface Props {
  x: number
  y: number
  tabId: string
  activeTabs: TabInfo[]
  onClose: () => void
  onCloseTab: (id: string) => void
  onCloseOthers: (keepId: string) => void
  onCloseAll: () => void
}

export function TabContextMenu({
  x, y, tabId, activeTabs, onClose, onCloseTab, onCloseOthers, onCloseAll,
}: Props): JSX.Element {
  useEffect(() => {
    function onDown(): void { onClose() }
    function onKey(e: KeyboardEvent): void { if (e.key === 'Escape') onClose() }
    window.addEventListener('mousedown', onDown)
    window.addEventListener('keydown', onKey)
    return () => {
      window.removeEventListener('mousedown', onDown)
      window.removeEventListener('keydown', onKey)
    }
  }, [onClose])

  const ctxTab = activeTabs.find(t => t.id === tabId)
  const ctxKey = ctxTab?.sessionTitle
  const ctxPinned = ctxKey ? pinnedTitleColors.value[ctxKey] : undefined
  const liveCount = activeTabs.filter(t => !t.id.startsWith('ghost:')).length
  const liveOthersCount = activeTabs.filter(t => t.id !== tabId && !t.id.startsWith('ghost:')).length

  return (
    <div
      class={styles.ctxMenu}
      style={`left:${x}px;top:${y}px`}
      onMouseDown={(e) => e.stopPropagation()}
    >
      {ctxKey && (
        <div style="display:flex;flex-wrap:wrap;gap:4px;padding:6px 8px;border-bottom:1px solid var(--bg-surface);align-items:center">
          <span style="font-size:10px;color:var(--text-muted);margin-right:4px;text-transform:uppercase;letter-spacing:0.04em">Color</span>
          {PALETTE_TOKENS.map((tok, i) => {
            const swatch = AGENT_PALETTE[i]
            const isActive = ctxPinned === tok
            return (
              <button
                key={tok}
                type="button"
                onClick={() => { setPinnedTitleColor(ctxKey, tok); onClose() }}
                data-tooltip={tok.replace('--accent-', '').replace('--', '')}
                style={`width:16px;height:16px;border-radius:50%;border:2px solid ${isActive ? 'var(--text-primary)' : 'transparent'};background:${swatch};cursor:pointer;padding:0`}
              />
            )
          })}
          <button
            type="button"
            onClick={() => { setPinnedTitleColor(ctxKey, null); onClose() }}
            data-tooltip="Clear (auto color)"
            style="width:16px;height:16px;border-radius:50%;border:1px dashed var(--text-muted);background:transparent;color:var(--text-muted);cursor:pointer;padding:0;font-size:8px;display:flex;align-items:center;justify-content:center"
          >
            <i class="fas fa-xmark" />
          </button>
        </div>
      )}
      <button
        type="button"
        class={styles.ctxItem}
        onClick={() => { onClose(); if (!tabId.startsWith('ghost:')) onCloseTab(tabId) }}
      >
        <i class="fas fa-xmark" /> Close tab
      </button>
      <button
        type="button"
        class={styles.ctxItem}
        onClick={() => onCloseOthers(tabId)}
        disabled={liveOthersCount === 0}
      >
        <i class="fas fa-rectangle-xmark" /> Close others
      </button>
      <button
        type="button"
        class={`${styles.ctxItem} ${styles.ctxDanger}`}
        onClick={onCloseAll}
        disabled={liveCount === 0}
      >
        <i class="fas fa-trash" /> Close all
      </button>
    </div>
  )
}

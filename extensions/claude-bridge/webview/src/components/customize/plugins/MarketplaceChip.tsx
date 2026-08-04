import type { JSX } from 'preact'
import { useState, useEffect } from 'preact/hooks'
import { useBridge } from '../../../hooks/useBridge'
import { showToast } from '../../../signals/toasts'
import { MarketplaceChipMenu } from './MarketplaceChipMenu'
import { SetTokenModal } from './SetTokenModal'
import styles from './MarketplaceChip.module.css'

interface Props {
  name: string
  autoUpdate: boolean
  isActive: boolean
  onClick: () => void
  onRefresh: () => void
}

export function MarketplaceChip({ name, autoUpdate, isActive, onClick, onRefresh }: Props): JSX.Element {
  const bridge = useBridge()
  const [menuVisible, setMenuVisible] = useState(false)
  const [updating, setUpdating] = useState(false)
  const [tokenModalOpen, setTokenModalOpen] = useState(false)
  const [localAutoUpdate, setLocalAutoUpdate] = useState(autoUpdate)

  useEffect(() => { setLocalAutoUpdate(autoUpdate) }, [autoUpdate])

  // Listen for background auto-refresh events — show a subtle spinner on
  // the matching chip so the user sees that refresh actually happened.
  useEffect(() => {
    return bridge.onMarketplaceUpdated((payload) => {
      if (payload.name !== name) return
      setUpdating(false)
      onRefresh()
    })
  }, [name])

  async function handleUpdate(): Promise<void> {
    setUpdating(true)
    try {
      const result = await bridge.updateMarketplace(name)
      if (result.ok) {
        showToast({
          type: 'success',
          title: name,
          message: result.changed ? 'Marketplace updated' : 'Already up to date',
        })
        onRefresh()
      } else {
        showToast({
          type: 'error',
          title: name,
          message: result.error || 'Update failed',
        })
      }
    } catch (err: any) {
      showToast({
        type: 'error',
        title: name,
        message: err?.message || String(err),
      })
    } finally {
      setUpdating(false)
    }
  }

  async function handleToggleAutoUpdate(next: boolean): Promise<void> {
    // Optimistic — flip locally, then persist. Revert on failure. Wrapped so
    // an unimplemented channel (null → `null.ok` TypeError) reverts the toggle
    // instead of leaving it lying + throwing an unhandled rejection.
    setLocalAutoUpdate(next)
    try {
      const result = await bridge.setMarketplaceAutoUpdate(name, next)
      if (!result?.ok) {
        setLocalAutoUpdate(!next)
        showToast({ type: 'error', title: name, message: result?.error || 'Plugin management is not available yet' })
      }
    } catch (err) {
      setLocalAutoUpdate(!next)
      showToast({ type: 'error', title: name, message: err instanceof Error ? err.message : String(err) })
    }
  }

  async function handleRemove(): Promise<void> {
    const show = (window as any).__showConfirmModal as ((o: { title: string; bodyHtml: string; confirmLabel?: string; isDanger?: boolean }) => Promise<boolean>) | undefined
    const escape = (s: string) => s.replace(/[&<>"']/g, c => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c]!))
    const ok = show
      ? await show({ title: 'Remove marketplace?', bodyHtml: `<p>Remove marketplace <b>${escape(name)}</b>? Plugins from it will become uninstallable until re-added.</p>`, confirmLabel: 'Remove', isDanger: true })
      : false
    if (!ok) return
    try {
      await bridge.removeMarketplace(name)
      onRefresh()
    } catch (err: any) {
      showToast({ type: 'error', title: name, message: err?.message || String(err) })
    }
  }

  return (
    <div class={`${styles.chip} ${isActive ? styles.active : ''}`} style="position:relative;">
      <span style="cursor:pointer;" onClick={onClick}>
        {name}
        {updating && <i class="fas fa-sync-alt fa-spin" style="margin-left:6px;font-size:10px;opacity:0.7;" />}
      </span>
      <span
        class={styles.dotsMenu}
        onClick={(e) => { e.stopPropagation(); setMenuVisible((v) => !v) }}
      >
        &middot;&middot;&middot;
      </span>
      <MarketplaceChipMenu
        visible={menuVisible}
        autoUpdate={localAutoUpdate}
        onUpdate={handleUpdate}
        onToggleAutoUpdate={handleToggleAutoUpdate}
        onSetToken={() => setTokenModalOpen(true)}
        onRemove={handleRemove}
        onClose={() => setMenuVisible(false)}
      />
      {tokenModalOpen && (
        <SetTokenModal
          marketplaceName={name}
          onClose={() => setTokenModalOpen(false)}
          onDone={() => onRefresh()}
        />
      )}
    </div>
  )
}

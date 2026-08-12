import type { JSX } from 'preact'
import { useEffect, useState } from 'preact/hooks'
import { useBridge } from '../../../hooks/useBridge'
import { PluginOptionsModal } from './PluginOptionsModal'
import { showToast } from '../../../signals/toasts'
import styles from './PluginCardActions.module.css'

interface Props {
  name: string
  marketplace: string
  isCached: boolean
  installPath?: string
  isRemoteSource?: boolean
  enabled: boolean
  onRefresh: () => void
}

export function PluginCardActions({ name, marketplace, isCached, installPath, isRemoteSource = false, enabled, onRefresh }: Props): JSX.Element {
  const bridge = useBridge()
  const [cacheLabel, setCacheLabel] = useState<string | null>(null)
  const [cacheDisabled, setCacheDisabled] = useState(false)
  const [hasUserConfig, setHasUserConfig] = useState(false)
  const [showOptions, setShowOptions] = useState(false)
  const [toggling, setToggling] = useState(false)
  const [restartNotice, setRestartNotice] = useState(false)

  const pluginId = `${name}@${marketplace}`

  // Peek at the manifest once per card — if it declares userConfig, show a
  // Configure button. The heavy work (reading schema, showing modal) happens
  // only when user clicks. One lightweight IPC call per card.
  useEffect(() => {
    if (!isCached) return
    // Channel may be unimplemented (resolves null) — guard the destructure so
    // it doesn't TypeError, and only show the config button when a schema came.
    bridge.getPluginOptionsSchema(pluginId).then((r) => {
      const schema = (r as { schema?: Record<string, unknown> } | null)?.schema
      setHasUserConfig(!!schema && Object.keys(schema).length > 0)
    }).catch(() => { /* no schema, no button */ })
  }, [pluginId, isCached])

  async function handleCache(): Promise<void> {
    setCacheLabel('<i class="fas fa-spinner fa-spin"></i>')
    setCacheDisabled(true)
    try {
      if (isRemoteSource) {
        // Remote git plugin — pull sub-clone first, then refresh cache in
        // a single backend roundtrip.
        const res: any = await bridge.refreshPluginSource?.(name, marketplace)
        if (!res?.ok) throw new Error(res?.error || 'refresh failed')
      } else {
        await bridge.syncPluginCache(name, marketplace)
      }
      onRefresh()
      setCacheLabel(null)
    } catch {
      setCacheLabel('<i class="fas fa-times"></i> Failed')
      setTimeout(() => setCacheLabel(null), 2000)
    } finally {
      // Always re-enable — the old code only reset in catch, so a successful
      // (or null-resolving) call left the button disabled forever.
      setCacheDisabled(false)
    }
  }

  async function handleOpenCache(): Promise<void> {
    if (installPath) bridge.openFolder(installPath)
  }

  async function handleOpenSource(): Promise<void> {
    try {
      const sourcePath = await bridge.getPluginSourcePath(name, marketplace)
      if (sourcePath) bridge.openFolder(sourcePath)
    } catch {}
  }

  async function handleToggle(): Promise<void> {
    setToggling(true)
    try {
      const result = await bridge.setPluginEnabled(pluginId, !enabled)
      if (!result?.ok) throw new Error(result?.error || 'toggle failed')
      if (result.restartRequired) {
        setRestartNotice(true)
        showToast({
          type: 'info',
          title: `${name}: restart required`,
          message: 'Existing chats can still use the previous plugin commands and hooks. Close and reopen those chats to apply the change.',
          duration: 12_000,
        })
      }
      onRefresh()
    } catch (err) {
      showToast({ type: 'error', title: name, message: err instanceof Error ? err.message : String(err) })
    } finally {
      setToggling(false)
    }
  }

  return (
    <>
      <div class={styles.actions}>
        <button class={styles.btn} onClick={handleToggle} disabled={toggling} title="Restart the session after changing plugin state">
          <i class={`fas ${enabled ? 'fa-toggle-on' : 'fa-toggle-off'}`} /> {enabled ? 'Enabled' : 'Disabled'}
        </button>
        <button class={`${styles.btn} ${styles.cacheBtn}`} onClick={handleCache} disabled={cacheDisabled}>
          {isCached ? <><i class="fas fa-sync-alt" /> Update</> : <><i class="fas fa-download" /> Cache</>}
        </button>
        {hasUserConfig && (
          <button class={styles.btn} onClick={() => setShowOptions(true)} title="Configure plugin options">
            <i class="fas fa-cog" /> Configure
          </button>
        )}
        {isCached && installPath && (
          <button class={styles.btn} onClick={handleOpenCache} title="Open cache folder">
            <i class="fas fa-folder-open" />
          </button>
        )}
        <button class={styles.btn} onClick={handleOpenSource} title="Open source folder">
          <i class="fas fa-store" />
        </button>
      </div>
      {restartNotice && (
        <div style="margin-top:6px;color:var(--accent-yellow);font-size:var(--fs-xs);line-height:var(--lh-base)">
          Existing chats still have the previous plugin commands and hooks. Close and reopen them to apply this change.
        </div>
      )}
      {showOptions && (
        <PluginOptionsModal
          pluginId={pluginId}
          pluginName={name}
          onClose={() => setShowOptions(false)}
          onSaved={onRefresh}
        />
      )}
    </>
  )
}

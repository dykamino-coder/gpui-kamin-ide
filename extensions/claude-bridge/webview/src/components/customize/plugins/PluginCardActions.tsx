import type { JSX } from 'preact'
import { useEffect, useState } from 'preact/hooks'
import { useBridge } from '../../../hooks/useBridge'
import { PluginOptionsModal } from './PluginOptionsModal'
import styles from './PluginCardActions.module.css'

interface Props {
  name: string
  marketplace: string
  isCached: boolean
  installPath?: string
  isRemoteSource?: boolean
  onRefresh: () => void
}

export function PluginCardActions({ name, marketplace, isCached, installPath, isRemoteSource = false, onRefresh }: Props): JSX.Element {
  const bridge = useBridge()
  const [cacheLabel, setCacheLabel] = useState<string | null>(null)
  const [cacheDisabled, setCacheDisabled] = useState(false)
  const [hasUserConfig, setHasUserConfig] = useState(false)
  const [showOptions, setShowOptions] = useState(false)

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

  return (
    <>
      <div class={styles.actions}>
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

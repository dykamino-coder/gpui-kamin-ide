// Extracted from PluginCard.tsx (Sprint 5 / Stage E1) — when a plugin's
// install dir is actually a marketplace (".claude-plugin/marketplace.json"
// but no plugin.json), the card can expand into a nested list of child
// plugins, each rendered as its own PluginCard. Owns the expand/collapse
// state and the lazy fetch of nested children via bridge.

import type { JSX } from 'preact'
import { useState } from 'preact/hooks'
import { useBridge } from '../../../hooks/useBridge'
import styles from './PluginCard.module.css'

interface Props {
  installPath?: string
  pluginPath?: string
  subPluginTotal?: number
  subPluginInstalled?: number
  /** Card constructor — passed to break the circular import (PluginCard
   *  imports this, this imports PluginCard would loop). */
  PluginCardComponent: (p: any) => JSX.Element
  onParentRefresh: () => void
}

export function PluginCardSubPlugins({
  installPath, pluginPath, subPluginTotal, subPluginInstalled = 0,
  PluginCardComponent, onParentRefresh,
}: Props): JSX.Element {
  const bridge = useBridge()
  const [subPlugins, setSubPlugins] = useState<any[] | null>(null)
  const [subExpanded, setSubExpanded] = useState(false)
  const [subLoading, setSubLoading] = useState(false)

  async function handleToggle(): Promise<void> {
    if (subExpanded) { setSubExpanded(false); return }
    if (!installPath && !pluginPath) { setSubExpanded(true); return }
    if (subPlugins === null) {
      setSubLoading(true)
      try {
        const list = await bridge.browseNestedMarketplace?.(installPath || pluginPath || '')
        setSubPlugins(Array.isArray(list) ? list : [])
      } catch { setSubPlugins([]) }
      setSubLoading(false)
    }
    setSubExpanded(true)
  }

  async function handleChildRefresh(): Promise<void> {
    try {
      const list = await bridge.browseNestedMarketplace?.(installPath || pluginPath || '')
      setSubPlugins(Array.isArray(list) ? list : [])
    } catch { /* keep stale state */ }
    onParentRefresh()
  }

  const label = subPluginTotal === undefined
    ? 'View sub-plugins'
    : `${subPluginTotal} sub-plugin${subPluginTotal === 1 ? '' : 's'} · ${subPluginInstalled} installed`

  return (
    <>
      <button
        class={styles.subMarketplaceToggle}
        onClick={handleToggle}
        type="button"
        aria-expanded={subExpanded}
      >
        <i class={`fas ${subExpanded ? 'fa-chevron-down' : 'fa-chevron-right'}`} style="margin-right:8px;width:10px" />
        <i class="fas fa-store" style="margin-right:8px;opacity:0.7" />
        <span>{label}</span>
        {subLoading && <i class="fas fa-spinner fa-spin" style="margin-left:auto" />}
      </button>
      {subExpanded && (
        <div class={styles.subPluginsPanel}>
          {subPlugins === null && !subLoading && (
            <div class={styles.placeholder}>No sub-plugins discovered</div>
          )}
          {subPlugins && subPlugins.length === 0 && (
            <div class={styles.placeholder}>Nested marketplace.json is empty</div>
          )}
          {subPlugins && subPlugins.map((sp: any) => (
            <PluginCardComponent
              key={`${sp.marketplace}/${sp.name}`}
              name={sp.name}
              version={sp.version}
              description={sp.description}
              marketplace={sp.marketplace}
              author={sp.author}
              isCached={sp.isCached}
              isInstalled={sp.isInstalled}
              installPath={sp.installPath}
              pluginPath={sp.pluginPath}
              counts={sp.counts}
              isRemoteSource={sp.isRemoteSource}
              onRefresh={handleChildRefresh}
            />
          ))}
        </div>
      )}
    </>
  )
}

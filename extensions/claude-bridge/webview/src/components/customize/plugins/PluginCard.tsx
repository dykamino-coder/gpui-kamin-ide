import type { JSX } from 'preact'
import { useState } from 'preact/hooks'
import { useBridge } from '../../../hooks/useBridge'
import { prettifyPluginName } from '../../../utils/prettify-plugin-name'
import { PluginCardHeader } from './PluginCardHeader'
import { PluginCardInfo } from './PluginCardInfo'
import { PluginBadges } from './PluginBadges'
import { PluginCardActions } from './PluginCardActions'
import { PluginContentsPreview } from './PluginContentsPreview'
import { PluginCardCloneError } from './PluginCardCloneError'
import { PluginCardSubPlugins } from './PluginCardSubPlugins'
import { PluginCardBrokenActions } from './PluginCardBrokenActions'
import { PluginCardDependencies } from './PluginCardDependencies'
import styles from './PluginCard.module.css'

interface Counts {
  commands?: number
  agents?: number
  skills?: number
  lspServers?: number
  mcpServers?: number
  hooks?: number
  isBrokenMarketplace?: boolean
  subPluginTotal?: number
  subPluginInstalled?: number
  nestedMarketplaceName?: string
}

interface Dependency {
  name: string
  requested: string
  installed: boolean
  enabled: boolean
}

interface Props {
  name: string
  version?: string
  description?: string
  marketplace: string
  author?: string
  isCached?: boolean
  isInstalled?: boolean
  enabled?: boolean
  installPath?: string
  pluginPath?: string
  counts?: Counts
  dependencies?: Dependency[]
  /** True when marketplace.json declares the plugin source as a git URL —
   *  the Update button then does `git pull` on the sub-clone before
   *  refreshing the installed cache. */
  isRemoteSource?: boolean
  /** Backend couldn't clone/resolve the plugin — card shows an error banner
   *  with Retry button instead of the normal Install/counts row. */
  cloneFailed?: boolean
  cloneError?: string
  cloneUrl?: string
  cloneRef?: string
  /** True when the failure is NOT a network/clone issue but a local-string
   *  `source: "./path"` in marketplace.json that doesn't exist on disk —
   *  e.g. git submodule that wasn't initialised, file removed from the
   *  marketplace repo, etc. Retry is pointless; show a muted note. */
  isLocalSourceMissing?: boolean
  declaredSourcePath?: string
  onRefresh: () => void
  onOpenAsMarketplace?: (marketplaceName: string) => void
}

export function PluginCard({
  name,
  version,
  description,
  marketplace,
  author,
  isCached = false,
  isInstalled = false,
  enabled = true,
  installPath,
  pluginPath,
  counts,
  dependencies,
  isRemoteSource = false,
  cloneFailed = false,
  cloneError,
  cloneUrl,
  cloneRef,
  isLocalSourceMissing = false,
  declaredSourcePath,
  onRefresh,
  onOpenAsMarketplace,
}: Props): JSX.Element {
  const bridge = useBridge()
  const [installing, setInstalling] = useState(false)
  const [uninstalling, setUninstalling] = useState(false)
  const [confirmUninstall, setConfirmUninstall] = useState(false)
  const [retrying, setRetrying] = useState(false)

  const displayName = prettifyPluginName(name)
  const authorDisplay = author || marketplace
  const isBrokenMarketplace = counts?.isBrokenMarketplace === true
  const iconColor = cloneFailed
    ? 'var(--accent-red)'
    : isBrokenMarketplace
      ? 'var(--accent-red)'
      : isInstalled ? (isCached ? 'var(--accent-green)' : 'var(--accent-yellow)') : 'var(--accent-primary)'

  async function handleRetry(): Promise<void> {
    setRetrying(true)
    try {
      await bridge.retryPluginSource?.(name, marketplace)
      onRefresh()
    } catch { /* user will see error on re-render */ }
    setRetrying(false)
  }

  async function handleInstall(): Promise<void> {
    setInstalling(true)
    try {
      await bridge.installPlugin(name, marketplace, pluginPath || '')
      onRefresh()
    } catch {
      setTimeout(() => setInstalling(false), 2000)
    }
    setInstalling(false)
  }

  function handleUninstall(): void {
    if (!confirmUninstall) {
      setConfirmUninstall(true)
      setTimeout(() => setConfirmUninstall(false), 3000)
      return
    }
    setConfirmUninstall(false)
    setUninstalling(true)
    bridge.uninstallPlugin(name, marketplace)
      .then(() => { onRefresh() })
      // Always clear the spinner. The old code only reset in catch, so if the
      // card didn't unmount after refresh (e.g. uninstall no-op'd) the
      // "Uninstalling…" spinner stuck forever.
      .finally(() => { setUninstalling(false) })
  }

  return (
    <div class={styles.card}>
      <PluginCardHeader
        iconColor={iconColor}
        isInstalled={isInstalled}
        name={name}
        marketplace={marketplace}
        pluginPath={pluginPath}
        onInstall={handleInstall}
        onUninstall={handleUninstall}
        installing={installing}
        uninstalling={uninstalling}
        confirmUninstall={confirmUninstall}
        installDisabled={cloneFailed || isLocalSourceMissing}
        installDisabledReason={
          isLocalSourceMissing
            ? 'Plugin files are not shipped with this marketplace clone'
            : cloneFailed ? 'Plugin source failed to download — retry first' : undefined
        }
      />
      <PluginCardInfo
        title={displayName}
        version={version}
        isInstalled={isInstalled}
        isCached={isCached}
      />
      <div class={styles.author}>{isInstalled ? 'from' : 'by'} {authorDisplay}</div>
      {isBrokenMarketplace && (
        <PluginCardSubPlugins
          installPath={installPath}
          pluginPath={pluginPath}
          subPluginTotal={counts?.subPluginTotal}
          subPluginInstalled={counts?.subPluginInstalled}
          PluginCardComponent={PluginCard}
          onParentRefresh={onRefresh}
        />
      )}
      {description && <div class={styles.desc}>{description}</div>}
      {cloneFailed && (
        <PluginCardCloneError
          isLocalSourceMissing={isLocalSourceMissing}
          declaredSourcePath={declaredSourcePath}
          cloneUrl={cloneUrl}
          cloneRef={cloneRef}
          cloneError={cloneError}
          retrying={retrying}
          onRetry={handleRetry}
        />
      )}
      {!cloneFailed && !isBrokenMarketplace && <PluginBadges counts={counts} />}
      {!cloneFailed && !isBrokenMarketplace && (counts?.skills || counts?.agents || counts?.commands || counts?.mcpServers || counts?.hooks) ? (
        <PluginContentsPreview
          dir={installPath || pluginPath}
          counts={counts}
        />
      ) : null}
      {dependencies && dependencies.length > 0 && (
        <PluginCardDependencies dependencies={dependencies} />
      )}
      {isInstalled && isBrokenMarketplace && (
        <PluginCardBrokenActions
          marketplace={marketplace}
          uninstalling={uninstalling}
          confirmUninstall={confirmUninstall}
          onUninstall={handleUninstall}
          onOpenAsMarketplace={onOpenAsMarketplace}
        />
      )}
      {isInstalled && !isBrokenMarketplace && (
        <PluginCardActions
          name={name}
          marketplace={marketplace}
          isCached={isCached}
          installPath={installPath}
          isRemoteSource={isRemoteSource}
          enabled={enabled}
          onRefresh={onRefresh}
        />
      )}
    </div>
  )
}

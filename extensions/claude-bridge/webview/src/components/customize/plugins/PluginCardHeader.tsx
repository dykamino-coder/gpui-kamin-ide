import type { JSX } from 'preact'
import styles from './PluginCardHeader.module.css'

interface Props {
  iconColor: string
  isInstalled: boolean
  name: string
  marketplace: string
  pluginPath?: string
  onInstall: () => void
  onUninstall: () => void
  installing: boolean
  uninstalling: boolean
  confirmUninstall: boolean
  /** Disables the Install button when the plugin source isn't actually
   *  on disk (marketplace.json declared a path that doesn't exist). */
  installDisabled?: boolean
  installDisabledReason?: string
}

export function PluginCardHeader({
  iconColor,
  isInstalled,
  name,
  onInstall,
  onUninstall,
  installing,
  uninstalling,
  confirmUninstall,
  installDisabled = false,
  installDisabledReason,
}: Props): JSX.Element {
  return (
    <div class={styles.header}>
      <div class={styles.icon}>
        <i class="fas fa-puzzle-piece" style={`color:${iconColor};`} />
      </div>
      {isInstalled ? (
        <button
          class={`${styles.actionBtn} ${styles.installed}`}
          onClick={onUninstall}
          disabled={uninstalling}
        >
          {uninstalling ? <i class="fas fa-spinner fa-spin" /> : confirmUninstall ? 'Confirm?' : 'Uninstall'}
        </button>
      ) : (
        <button
          class={styles.actionBtn}
          onClick={onInstall}
          disabled={installing || installDisabled}
          title={installDisabled ? installDisabledReason : undefined}
        >
          {installing ? <i class="fas fa-spinner fa-spin" /> : 'Install'}
        </button>
      )}
    </div>
  )
}

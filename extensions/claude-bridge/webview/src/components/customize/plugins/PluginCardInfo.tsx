import type { JSX } from 'preact'
import styles from './PluginCardInfo.module.css'

interface Props {
  title: string
  version?: string
  isInstalled: boolean
  isCached: boolean
}

export function PluginCardInfo({ title, version, isInstalled, isCached }: Props): JSX.Element {
  return (
    <div class={styles.row}>
      <span class={styles.title}>{title}</span>
      {version && <span class={styles.version}>v{version}</span>}
      {isInstalled && (
        isCached
          ? <span class={styles.cachedIcon} title="Cached locally"><i class="fas fa-check-circle" /></span>
          : <span class={styles.notCachedIcon} title="Not in cache"><i class="fas fa-exclamation-circle" /></span>
      )}
    </div>
  )
}

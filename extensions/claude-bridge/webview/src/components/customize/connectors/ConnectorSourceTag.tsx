import type { JSX } from 'preact'
import type { ExternalMcpServerInfo } from '../../../../shared/types'
import styles from './ConnectorSourceTag.module.css'

interface Props {
  server: ExternalMcpServerInfo
}

export function ConnectorSourceTag({ server }: Props): JSX.Element | null {
  if (!server.autoDiscovered) return null

  const st = (server as any).sourceType as string | undefined
  if (st === 'claude-config') {
    return <span class={`${styles.tag} ${styles.config}`}>.claude.json</span>
  }
  if (st === 'plugin') {
    const pn = (server as any).pluginName as string | undefined
    return <span class={`${styles.tag} ${styles.plugin}`}>{pn || 'plugin'}</span>
  }
  return <span class={`${styles.tag} ${styles.auto}`}>auto</span>
}

import type { JSX } from 'preact'
import type { ExternalMcpServerInfo } from '../../../../shared/types'
import { ConnectorStatusDot } from './ConnectorStatusDot'
import { ConnectorSourceTag } from './ConnectorSourceTag'
import styles from './ConnectorItem.module.css'

interface Props {
  server: ExternalMcpServerInfo
  isActive: boolean
  onClick: () => void
}

const iconMap: Record<string, string> = {
  http: 'fa-globe',
  sse: 'fa-satellite-dish',
  stdio: 'fa-terminal',
}

export function ConnectorItem({ server, isActive, onClick }: Props): JSX.Element {
  const icon = iconMap[server.type] ?? 'fa-plug'
  const dotStatus = server.status === 'connecting' ? 'testing' : server.status
  const descText = server.status === 'connecting'
    ? 'testing...'
    : server.error
      ? server.error
      : server.status === 'connected'
        ? `${server.tools.length} tools`
        : server.status

  return (
    <div
      class={`${styles.item} ${isActive ? styles.active : ''}`}
      onClick={onClick}
    >
      <div class={styles.icon}>
        <i class={`fas ${icon}`} />
      </div>
      <div class={styles.info}>
        <div class={styles.name}>
          {server.name}
          <ConnectorSourceTag server={server} />
        </div>
        <div class={styles.desc}>{descText}</div>
      </div>
      <ConnectorStatusDot status={dotStatus as any} />
    </div>
  )
}

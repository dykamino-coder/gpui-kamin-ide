import type { JSX } from 'preact'
import type { ExternalMcpServerInfo } from '../../../../shared/types'
import { ConnectorGroup } from './ConnectorGroup'
import { ConnectorItem } from './ConnectorItem'
import { BUILTIN_TOOLS } from '../../../../shared/builtin-tools'
import styles from './ConnectorsList.module.css'

interface Props {
  servers: ExternalMcpServerInfo[]
  filter: string
  selectedId: string | null
  onSelect: (id: string) => void
}

export function ConnectorsList({ servers, filter, selectedId, onSelect }: Props): JSX.Element {
  const q = filter.toLowerCase()
  const matches = (s: ExternalMcpServerInfo): boolean => !q || s.name.toLowerCase().includes(q)
  const connected = servers.filter(
    (s) => (s.status === 'connected' || s.status === 'connecting') && matches(s)
  )
  const rest = servers.filter((s) => s.status !== 'connected' && s.status !== 'connecting' && matches(s))
  // Ошибки — отдельно от спящих lazy-серверов: красная точка с OAuth/ECONNREFUSED
  // тонула в списке «disconnected», где остальные просто ждут первого вызова.
  const errored = rest.filter((s) => !!s.error || s.status === 'error')
  const sleeping = rest.filter((s) => !s.error && s.status !== 'error')

  const showBuiltin = !q || 'user tools'.includes(q)

  return (
    <div class={styles.container}>
      {showBuiltin && (
        <ConnectorGroup title="Built-in">
          <div
            class={`${styles.builtinItem} ${selectedId === '__builtin' ? styles.active : ''}`}
            onClick={() => onSelect('__builtin')}
          >
            <div class={styles.icon}><i class="fas fa-toolbox" /></div>
            <div class={styles.info}>
              <div class={styles.name}>User Tools</div>
              <div class={styles.desc}>{BUILTIN_TOOLS.length} tools</div>
            </div>
            <span class={styles.dotConnected} />
          </div>
        </ConnectorGroup>
      )}

      {connected.length > 0 && (
        <ConnectorGroup title="Connected">
          {connected.map((s) => (
            <ConnectorItem
              key={s.id}
              server={s}
              isActive={selectedId === s.id}
              onClick={() => onSelect(s.id)}
            />
          ))}
        </ConnectorGroup>
      )}

      {errored.length > 0 && (
        <ConnectorGroup title="Errors">
          {errored.map((s) => (
            <ConnectorItem
              key={s.id}
              server={s}
              isActive={selectedId === s.id}
              onClick={() => onSelect(s.id)}
            />
          ))}
        </ConnectorGroup>
      )}

      {sleeping.length > 0 && (
        <ConnectorGroup title="Not connected">
          {sleeping.map((s) => (
            <ConnectorItem
              key={s.id}
              server={s}
              isActive={selectedId === s.id}
              onClick={() => onSelect(s.id)}
            />
          ))}
        </ConnectorGroup>
      )}
    </div>
  )
}

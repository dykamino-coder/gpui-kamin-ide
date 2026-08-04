import type { JSX } from 'preact'
import { useState, useEffect } from 'preact/hooks'
import type { ExternalMcpServerInfo } from '../../../../shared/types'
import { useBridge } from '../../../hooks/useBridge'
import { ConnectorStatusDot } from './ConnectorStatusDot'
import { ConnectorActions } from './ConnectorActions'
import { ConnectorToolItem } from './ConnectorToolItem'
import { TestLogPane } from './TestLogPane'
import { BUILTIN_TOOLS } from '../../../../shared/builtin-tools'
import styles from './ConnectorDetail.module.css'

interface Props {
  server: ExternalMcpServerInfo | '__builtin'
  onRemoved: () => void
  onEdit: () => void
}

export function ConnectorDetail({ server, onRemoved, onEdit }: Props): JSX.Element {
  const bridge = useBridge()
  const [statusDot, setStatusDot] = useState<string | null>(null)
  const [statusText, setStatusText] = useState<string | null>(null)

  // Reset local state when switching servers
  const serverId = server === '__builtin' ? '__builtin' : server.id
  useEffect(() => {
    setStatusDot(null)
    setStatusText(null)
  }, [serverId])

  if (server === '__builtin') {
    return (
      <div class={styles.container}>
        <div class={styles.header}>
          <h2>User Tools (built-in)</h2>
          <div class={styles.meta}>
            <span class={styles.badge}>Built-in</span>
            <span>{BUILTIN_TOOLS.length} tools via MCP</span>
          </div>
        </div>
        <div class={styles.section}>
          <h3>Tools</h3>
          <div class={styles.toolsGrid}>
            {[...BUILTIN_TOOLS].sort((a, b) => a.localeCompare(b)).map((t) => <ConnectorToolItem key={t} name={t} />)}
          </div>
        </div>
        <div class={styles.nativeNote}>
          Native CLI tools (not via MCP): Agent, WebSearch, TeamCreate, TaskCreate, Skill, ToolSearch, etc.
        </div>
      </div>
    )
  }

  const sAny = server as any
  const currentStatusDot = statusDot ?? (server.status === 'connecting' ? 'testing' : server.status)
  const currentStatusText = statusText ?? (
    server.status === 'connecting' ? 'testing...' : `${server.status}${server.error ? ` — ${server.error}` : ''}`
  )

  return (
    <div class={styles.container}>
      <div class={styles.header}>
        <h2>{server.name}</h2>
        <div class={styles.meta}>
          <span class={styles.badge}>{server.type.toUpperCase()}</span>
          <span>
            {server.type === 'http' || server.type === 'sse'
              ? server.url || ''
              : [server.command, ...(server.args || [])].join(' ')}
          </span>
        </div>

        {sAny.sourceType === 'claude-config' && (
          <div class={styles.meta} style="margin-top:6px;">
            <span class={`${styles.badge} ${styles.badgeConfig}`}>~/.claude.json</span>
            <span>User MCP config</span>
          </div>
        )}
        {sAny.sourceType === 'plugin' && (
          <div class={styles.meta} style="margin-top:6px;">
            <span class={`${styles.badge} ${styles.badgePlugin}`}>
              Plugin: {sAny.pluginName || 'unknown'}
            </span>
            {sAny.marketplace && (
              <span style="color:var(--text-muted);">@ {sAny.marketplace}</span>
            )}
          </div>
        )}
        {server.autoDiscovered && !sAny.sourceType && (
          <div class={styles.meta} style="margin-top:6px;">
            <span class={`${styles.badge} ${styles.badgeAuto}`}>Auto-discovered</span>
          </div>
        )}

        {sAny.description && (
          <div class={styles.description}>{sAny.description}</div>
        )}

        {server.sourcePath && (
          <div
            class={styles.sourcePath}
            title="Click to open in editor"
            onClick={() => bridge.openFolder(server.sourcePath!)}
          >
            <i class="fas fa-external-link-alt" style="margin-right:4px;font-size:9px;" />
            {server.sourcePath}
          </div>
        )}

        <div class={styles.meta} style="margin-top:8px;">
          <ConnectorStatusDot status={currentStatusDot as any} />
          <span>{currentStatusText}</span>
        </div>
      </div>

      <ConnectorActions
        serverId={server.id}
        serverName={server.name}
        enabled={server.enabled}
        autoDiscovered={server.autoDiscovered ?? false}
        hasOAuth={!!(server as any).oauth?.authServerUrl}
        sourceType={(server as any).sourceType}
        onRemoved={onRemoved}
        onEdit={onEdit}
        onStatusChange={(dot, text, forServerId) => {
          // Only update status if this detail is still showing the same server
          if (forServerId === server.id) {
            setStatusDot(dot)
            setStatusText(text)
          }
        }}
      />

      {server.tools.length > 0 && (
        <div class={styles.section}>
          <h3>Tools ({server.tools.length})</h3>
          <div class={styles.toolsGrid}>
            {[...server.tools].sort((a, b) => a.localeCompare(b)).map((t) => {
              const info = (server as any).toolInfos?.find((x: { name: string }) => x.name === t)
              return <ConnectorToolItem key={t} name={t} description={info?.description} />
            })}
          </div>
        </div>
      )}

      <TestLogPane serverId={server.id} />
    </div>
  )
}

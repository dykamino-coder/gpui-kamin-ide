import type { RequestLogEntry } from '../../types'
import { ApiFormatBadge } from './ApiFormatBadge'
import styles from './RequestRow.module.css'

interface RequestRowProps {
  entry: RequestLogEntry
  onClick: () => void
}

export function RequestRow({ entry, onClick }: RequestRowProps) {
  const time = new Date(entry.timestamp).toLocaleTimeString()
  const isError = entry.status === 'error'
  const isStreaming = entry.status === 'streaming'

  return (
    <div class={`${styles.row} ${isError ? styles.error : ''}`} onClick={onClick}>
      <span class={styles.time}>{time}</span>
      <span class={`${styles.typeBadge} ${entry.isUserRequest ? styles.typeUser : styles.typeTool}`}>
        {entry.isUserRequest ? 'U' : 'T'}
      </span>
      <span class={styles.plugin}>{entry.endpoint === 'pty' ? entry.userName || 'pty' : entry.pluginId}</span>
      <ApiFormatBadge endpoint={entry.endpoint} />
      <span class={styles.model} title={entry.endpoint === 'pty' ? entry.userMessage : entry.model}>
        {entry.endpoint === 'pty' ? (entry.userMessage?.slice(0, 40) || '—') : entry.model}
      </span>
      <span class={styles.duration}>
        {entry.endpoint === 'pty' ? '—' : entry.durationMs > 0 ? `${(entry.durationMs / 1000).toFixed(1)}s` : '...'}
      </span>
      <span class={styles.tokens}>
        {(() => {
          const totalIn = entry.inputTokens + (entry.cacheReadTokens || 0) + (entry.cacheWriteTokens || 0)
          return totalIn > 0 ? `${totalIn}→${entry.outputTokens}` : '—'
        })()}
      </span>
      <span class={`${styles.statusIcon} ${isError ? styles.err : isStreaming ? styles.streaming : styles.ok}`}>
        {isError ? <i class="fa-solid fa-xmark" /> : isStreaming ? <i class="fa-solid fa-spinner fa-spin" /> : <i class="fa-solid fa-check" />}
      </span>
    </div>
  )
}

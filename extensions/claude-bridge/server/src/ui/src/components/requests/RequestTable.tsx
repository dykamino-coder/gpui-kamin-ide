import { useCallback, useRef, useState } from 'preact/hooks'
import type { RequestLogEntry } from '../../types'
import { capturedLog } from '../../signals/requests'
import { formatDateTime, formatTokens, formatModel } from '../../utils/formatters'
import { hasCapture } from './download'
import { DownloadMenu } from './DownloadMenu'
import { ToolsModal } from './ToolsModal'
import styles from './RequestTable.module.css'

const ROW_HEIGHT = 33
const OVERSCAN = 10
const COL_COUNT = 8

interface RequestTableProps {
  entries: RequestLogEntry[]
  onLoadMore?: () => void
  loading?: boolean
  loadingMore?: boolean
  maxHeight?: number
  emptyMessage?: string
}

export function RequestTable({
  entries,
  onLoadMore,
  loading = false,
  loadingMore = false,
  maxHeight = 400,
  emptyMessage = 'No requests found',
}: RequestTableProps) {
  const [scrollTop, setScrollTop] = useState(0)
  const [toolsRequest, setToolsRequest] = useState<RequestLogEntry | null>(null)
  const [toolsFilter, setToolsFilter] = useState<'tools' | 'mcp'>('tools')
  const scrollRef = useRef<HTMLDivElement>(null)

  const totalRows = entries.length

  const startIndex = Math.max(0, Math.floor(scrollTop / ROW_HEIGHT) - OVERSCAN)
  const endIndex = Math.min(totalRows, Math.ceil((scrollTop + maxHeight) / ROW_HEIGHT) + OVERSCAN)
  const visibleEntries = entries.slice(startIndex, endIndex)
  const topSpacerHeight = startIndex * ROW_HEIGHT
  const bottomSpacerHeight = Math.max(0, (totalRows - endIndex) * ROW_HEIGHT)

  const handleScroll = useCallback(() => {
    const el = scrollRef.current
    if (!el) return
    setScrollTop(el.scrollTop)
    if (onLoadMore && el.scrollHeight - el.scrollTop - el.clientHeight < 100) {
      onLoadMore()
    }
  }, [onLoadMore])

  if (loading) {
    return (
      <div class={styles.loadingState}>
        <i class="fa-solid fa-spinner fa-spin" /> Loading...
      </div>
    )
  }

  if (totalRows === 0) {
    return (
      <div class={styles.empty}>
        <i class="fa-solid fa-inbox" />
        <span>{emptyMessage}</span>
      </div>
    )
  }

  return (
    <>
      <div
        class={styles.scrollArea}
        style={{ maxHeight: `${maxHeight}px` }}
        ref={scrollRef}
        onScroll={handleScroll}
      >
        <table class={styles.table}>
          <thead>
            <tr>
              <th>Time</th>
              <th>User</th>
              <th>Session</th>
              <th>Model</th>
              <th>Tokens</th>
              <th>New</th>
              <th>Status</th>
              <th></th>
            </tr>
          </thead>
          <tbody>
            {topSpacerHeight > 0 && (
              <tr style={{ height: `${topSpacerHeight}px` }}>
                <td colSpan={COL_COUNT} style={{ padding: 0, border: 'none' }} />
              </tr>
            )}
            {visibleEntries.map((r) => {
              const captured = capturedLog.value.find(c => c.id === r.id)
              const downloadEntry = hasCapture(r) ? r : (captured && hasCapture(captured) ? captured : null)
              const total = r.inputTokens + (r.cacheReadTokens || 0) + (r.cacheWriteTokens || 0) + r.outputTokens
              // New = everything except cache reads (tokens actually processed by model)
              const newTokens = r.inputTokens + (r.cacheWriteTokens || 0) + r.outputTokens

              const bodyTools: any[] = ((r.requestBody as any)?.tools as any[]) ?? []
              const regularTools = bodyTools.filter(t => !t.name.startsWith('mcp__'))
              const mcpTools = bodyTools.filter(t => t.name.startsWith('mcp__'))
              const regularUsed = r.toolsUsed.filter(n => !n.startsWith('mcp__'))
              const mcpUsed = r.toolsUsed.filter(n => n.startsWith('mcp__'))
              const hasRegular = regularTools.length > 0 || regularUsed.length > 0
              const hasMcp = mcpTools.length > 0 || mcpUsed.length > 0

              return (
                <tr key={r.id}>
                  <td class={styles.time}>{formatDateTime(r.timestamp)}</td>
                  <td class={styles.userName}>{r.userName || '—'}</td>
                  <td class={styles.session} title={r.sessionKey}>{r.sessionKey ? r.sessionKey.slice(0, 8) : '—'}</td>
                  <td class={styles.model}>{r.model ? formatModel(r.model) : '—'}</td>
                  <td
                    class={styles.tokens}
                    title={`input: ${r.inputTokens}\noutput: ${r.outputTokens}\ncache read: ${r.cacheReadTokens || 0}\ncache write: ${r.cacheWriteTokens || 0}`}
                  >
                    {total > 0 ? formatTokens(total) : '—'}
                  </td>
                  <td
                    class={styles.tokens}
                    title={`raw input: ${r.inputTokens}\ncache write: ${r.cacheWriteTokens || 0}\noutput: ${r.outputTokens}\nsaved (cache read): ${r.cacheReadTokens || 0}`}
                  >
                    {newTokens > 0 ? formatTokens(newTokens) : '—'}
                  </td>
                  <td>
                    <span class={`${styles.statusBadge} ${styles[r.status]}`}>
                      {r.status === 'cancelled' ? 'aborted' : r.status}
                    </span>
                  </td>
                  <td>
                    <div class={styles.actions}>
                      {hasRegular && (
                        <button
                          class={styles.toolsBtn}
                          onClick={() => { setToolsFilter('tools'); setToolsRequest(r) }}
                          title={`${regularTools.length || regularUsed.length} tools`}
                        >
                          <i class="fa-solid fa-wrench" />
                          {regularTools.length > 0 && <span class={styles.toolCount}>{regularTools.length}</span>}
                        </button>
                      )}
                      {hasMcp && (
                        <button
                          class={`${styles.toolsBtn} ${styles.mcpBtn}`}
                          onClick={() => { setToolsFilter('mcp'); setToolsRequest(r) }}
                          title={`${mcpTools.length || mcpUsed.length} MCP tools`}
                        >
                          <i class="fa-solid fa-plug" />
                          {mcpTools.length > 0 && <span class={styles.toolCount}>{mcpTools.length}</span>}
                        </button>
                      )}
                      {downloadEntry && (
                        <DownloadMenu entry={downloadEntry} btnClass={styles.dlBtn} />
                      )}
                    </div>
                  </td>
                </tr>
              )
            })}
            {bottomSpacerHeight > 0 && (
              <tr style={{ height: `${bottomSpacerHeight}px` }}>
                <td colSpan={COL_COUNT} style={{ padding: 0, border: 'none' }} />
              </tr>
            )}
          </tbody>
        </table>
        {loadingMore && (
          <div class={styles.loadingMore}>
            <i class="fa-solid fa-spinner fa-spin" /> Loading more...
          </div>
        )}
      </div>

      <ToolsModal request={toolsRequest} filter={toolsFilter} onClose={() => setToolsRequest(null)} />
    </>
  )
}

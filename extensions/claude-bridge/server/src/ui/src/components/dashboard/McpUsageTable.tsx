import { Fragment } from 'preact'
import { useState } from 'preact/hooks'
import type { McpUsageBreakdown, McpUsageRow, McpServerUsage, McpUsageMetrics } from '../../../../shared/mcp-report'
import styles from './McpUsageReport.module.css'

export function catalogKB(bytes: number): string {
  return `${(bytes / 1024).toLocaleString(undefined, { maximumFractionDigits: 1 })} KB`
}

function Metrics({ row }: { row: McpUsageMetrics }) {
  return (
    <>
      <td>{catalogKB(row.schemaBytes)}</td>
      <td>{row.observedSessions.toLocaleString()}</td>
      <td>
        {row.calls.toLocaleString()}
        {row.incompleteSessions > 0 && <span title="Includes incomplete legacy telemetry"> *</span>}
      </td>
      <td>
        {row.lastCall
          ? new Date(row.lastCall).toLocaleString()
          : row.calls || row.incompleteSessions
            ? 'Not recorded'
            : 'Never'}
      </td>
      <td>{catalogKB(row.wastedBytes)}</td>
    </>
  )
}

function Breakdown({ rows }: { rows: McpUsageBreakdown[] }) {
  return (
    <table class={styles.table} aria-label="User and project breakdown">
      <thead>
        <tr>
          <th>User / project</th>
          <th>Catalog</th>
          <th>Sessions offered</th>
          <th>Calls</th>
          <th>Last call</th>
          <th>Unused catalog</th>
        </tr>
      </thead>
      <tbody>
        {rows.map((r) => (
          <tr key={JSON.stringify([r.userName, r.project])}>
            <td>
              {r.userName}
              <br />
              <span class={styles.note}>{r.project}</span>
            </td>
            <Metrics row={r} />
          </tr>
        ))}
      </tbody>
    </table>
  )
}

export function McpUsageTable({
  rows,
  servers = false,
}: {
  rows: (McpUsageRow | McpServerUsage)[]
  servers?: boolean
}) {
  const [expanded, setExpanded] = useState<Set<string>>(new Set())
  const [showAll, setShowAll] = useState(false)
  const visible = showAll ? rows : rows.slice(0, 20)
  return (
    <>
      <div class={styles.scroll}>
        <table class={styles.table} aria-label={servers ? 'MCP server usage' : 'MCP tool usage'}>
          <thead>
            <tr>
              <th>{servers ? 'Source' : 'Tool'}</th>
              {!servers && <th>Source</th>}
              <th>Catalog</th>
              <th>Sessions offered</th>
              <th>Calls</th>
              <th>Last call</th>
              <th>Unused catalog ↓</th>
            </tr>
          </thead>
          <tbody>
            {visible.map((row) => {
              const label = 'toolName' in row ? row.toolName : row.source
              const key = JSON.stringify([row.source, label])
              const open = expanded.has(key)
              return (
                <Fragment key={key}>
                  <tr data-mcp-tool={'toolName' in row ? row.toolName : undefined}>
                    <td>
                      <button
                        type="button"
                        class={styles.expand}
                        aria-expanded={open}
                        onClick={() =>
                          setExpanded((previous) => {
                            const next = new Set(previous)
                            if (next.has(key)) next.delete(key)
                            else next.add(key)
                            return next
                          })
                        }
                      >
                        {open ? '▾' : '▸'} {label}
                      </button>
                      {'toolCount' in row && <span class={styles.note}> · {row.toolCount} tools</span>}
                    </td>
                    {!servers && <td>{row.source === 'built-in' ? 'Built-in (Bridge MCP)' : row.source}</td>}
                    <Metrics row={row} />
                  </tr>
                  {open && (
                    <tr>
                      <td colSpan={servers ? 6 : 7} class={styles.detail}>
                        <Breakdown rows={row.breakdown} />
                      </td>
                    </tr>
                  )}
                </Fragment>
              )
            })}
          </tbody>
        </table>
      </div>
      {rows.length > 20 && (
        <button type="button" class={`${styles.button} ${styles.showAll}`} onClick={() => setShowAll(!showAll)}>
          {showAll ? 'Show top 20' : `Show all ${rows.length}`}
        </button>
      )}
    </>
  )
}

import { useEffect, useState } from 'preact/hooks'
import { api } from '../../services/api-client'
import { Card, SectionHeader } from '../shared'
import { McpUsageTable, catalogKB } from './McpUsageTable'
import type { McpUsageReport as Report } from '../../../../shared/mcp-report'
import styles from './McpUsageReport.module.css'

export function McpUsageReport() {
  const [report, setReport] = useState<Report | null>(null)
  const [error, setError] = useState(false)
  const [loading, setLoading] = useState(false)
  const [servers, setServers] = useState(false)
  const refresh = () => {
    setLoading(true)
    setError(false)
    api
      .getMcpReport()
      .then(setReport)
      .catch(() => setError(true))
      .finally(() => setLoading(false))
  }
  useEffect(refresh, [])
  return (
    <Card>
      <SectionHeader
        title="MCP tool usage"
        action={
          <button type="button" class={styles.button} disabled={loading} onClick={refresh}>
            Refresh
          </button>
        }
      />
      {report && (
        <p class={styles.summary}>
          {catalogKB(report.wastedBytes)} of tool catalog offered but unused in the last {report.sessionLimit} sessions
          per user/project.
        </p>
      )}
      <p class={styles.note}>
        Sorted by catalog bytes offered in sessions with no calls.{' '}
        {report && `${report.observedSessions} sessions observed.`}
      </p>
      {report?.rows.some((row) => row.incompleteSessions > 0) && (
        <p class={styles.note}>
          * Legacy counters are incomplete; their zero-call sessions are excluded from unused totals.
        </p>
      )}
      <details class={styles.note}>
        <summary>What is measured?</summary>
        <p>
          Only tools offered through Bridge MCP, including Bridge built-ins such as Bash and Edit. Native CLI tools and
          other direct MCP connections are excluded. Calls include failed attempts.
        </p>
        <p>
          Catalog size is UTF-8 JSON, not billed tokens. Unused bytes are summed across zero-call sessions, using each
          session’s catalog size. Up to 1,000 catalog tools per session are observed. Missing historical call times are
          shown as “Not recorded”. Telemetry may be incomplete after a crash or storage failure.
        </p>
      </details>
      {error && <p role="status">MCP report unavailable. Try Refresh.</p>}
      {!report && !error && <p role="status">Loading…</p>}
      {report && report.rows.length === 0 && <p>No MCP catalogs observed yet.</p>}
      {report && report.rows.length > 0 && (
        <>
          <div class={styles.switch} role="group" aria-label="MCP usage grouping">
            <button type="button" class={styles.button} aria-pressed={!servers} onClick={() => setServers(false)}>
              Tools ({report.rows.length})
            </button>
            <button type="button" class={styles.button} aria-pressed={servers} onClick={() => setServers(true)}>
              MCP servers ({report.servers.length})
            </button>
          </div>
          <McpUsageTable
            key={servers ? 'servers' : 'tools'}
            rows={servers ? report.servers : report.rows}
            servers={servers}
          />
        </>
      )}
    </Card>
  )
}

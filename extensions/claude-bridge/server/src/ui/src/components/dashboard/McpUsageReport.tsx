import { useEffect, useState } from 'preact/hooks'
import { api } from '../../services/api-client'
import { Card, SectionHeader } from '../shared'
import type { McpUsageReport as Report } from '../../../../shared/mcp-report'
import styles from './McpUsageReport.module.css'

export function McpUsageReport() {
  const [report, setReport] = useState<Report | null>(null)
  const [error, setError] = useState(false)
  const [loading, setLoading] = useState(false)
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
          <button type="button" disabled={loading} onClick={refresh}>
            Refresh
          </button>
        }
      />
      <p class={styles.note}>
        CLI 2.1.312 supports tool search for large catalogs. This report observes tools; it does not disable them.
        Catalog bytes are UTF-8 JSON size, not billed tokens.
      </p>
      <p class={styles.note}>
        No observed calls: most recent sessions offering each tool, since reporting began, within the last 20 Bridge
        sessions per user and project. Calls include failed attempts. Up to 1,000 tools per session are observed.
        Telemetry can be incomplete after a crash or storage failure.
      </p>
      {error && <p role="status">MCP report unavailable. Try Refresh.</p>}
      {!report && !error && <p role="status">Loading…</p>}
      {report && report.rows.length === 0 && <p>No MCP catalogs observed yet.</p>}
      {report && report.rows.length > 0 && (
        <div class={styles.scroll}>
          <table class={styles.table}>
            <thead>
              <tr>
                <th>User / project</th>
                <th>Tool</th>
                <th>Catalog bytes</th>
                <th>No observed calls</th>
                <th>Calls</th>
              </tr>
            </thead>
            <tbody>
              {report.rows.map((r) => (
                <tr key={`${r.userName}\0${r.project}\0${r.toolName}`}>
                  <td>
                    {r.userName}
                    <br />
                    {r.project}
                  </td>
                  <td>{r.toolName}</td>
                  <td>{r.schemaBytes.toLocaleString()}</td>
                  <td>
                    {r.unusedSessions} / {r.observedSessions} sessions
                  </td>
                  <td>{r.calls}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
      {report?.truncated && <p>Showing the first 1,000 tool rows.</p>}
    </Card>
  )
}

import type { DuckDBConnection } from '@duckdb/node-api'
import type {
  McpUsageBreakdown,
  McpUsageMetrics,
  McpUsageReport,
  McpUsageRow,
  McpServerUsage,
} from '../../shared/mcp-report'
import { MCP_TOOLS } from '../pty/mcp-tools-defs'
import { PROJECT_SESSION_TOOLS } from '../project-sessions/tools'

/** Read the durable per-session/tool rollup, never the raw transcript tables. */
export async function readMcpReport(db: DuckDBConnection): Promise<McpUsageReport> {
  const builtins = [...MCP_TOOLS, ...PROJECT_SESSION_TOOLS].map((t) => t.name)
  const result = (
    await db.runAndReadAll(
      `
    WITH sessions AS (
      SELECT bridge_session_id, user_name, project, MAX(timestamp) AS listed_at
      FROM mcp_tool_observations GROUP BY ALL
    ), ranked AS (
      SELECT *, ROW_NUMBER() OVER (
        PARTITION BY user_name, project ORDER BY listed_at DESC, bridge_session_id
      ) AS recency FROM sessions
    ), observations AS (
      SELECT o.* EXCLUDE(source), COALESCE(source, CASE
        WHEN tool_name IN (${builtins.map(() => '?').join(',')}) THEN 'built-in'
        ELSE 'MCP (server unknown)' END) AS source
      FROM mcp_tool_observations o JOIN ranked r USING (bridge_session_id, user_name, project)
      WHERE r.recency <= 20
    ), scoped AS (
      SELECT *, SUM(calls) OVER (PARTITION BY bridge_session_id, source) AS server_calls,
        BOOL_AND(calls_complete) OVER (PARTITION BY bridge_session_id, source) AS server_complete
      FROM observations
    )
    SELECT source, tool_name, user_name, project,
      GROUPING(source) AS global_row, GROUPING(tool_name) AS server_row, GROUPING(user_name) AS total_row,
      ARG_MAX(schema_bytes, timestamp) AS schema_bytes,
      COUNT(DISTINCT bridge_session_id) AS observed_sessions,
      CASE WHEN GROUPING(tool_name) = 1
        THEN COUNT(DISTINCT bridge_session_id) FILTER (WHERE server_calls = 0 AND server_complete)
        ELSE COUNT(*) FILTER (WHERE calls = 0 AND calls_complete) END AS unused_sessions,
      COUNT(DISTINCT bridge_session_id) FILTER (WHERE NOT calls_complete) AS incomplete_sessions,
      SUM(calls) AS calls, MAX(last_called_at) AS last_call,
      COALESCE(SUM(schema_bytes) FILTER (WHERE calls = 0 AND calls_complete), 0) AS wasted_bytes
    FROM scoped GROUP BY GROUPING SETS (
      (source, tool_name, user_name, project), (source, tool_name),
      (source, user_name, project), (source), ()
    )`,
      builtins,
    )
  ).getRowObjects()
  const tools = new Map<string, McpUsageRow>()
  const servers = new Map<string, McpServerUsage>()
  const details = new Map<string, McpUsageBreakdown[]>()
  const serverDetails = new Map<string, McpUsageBreakdown[]>()
  let observedSessions = 0
  let wastedBytes = 0
  for (const r of result) {
    const metrics: McpUsageMetrics = {
      schemaBytes: Number(r.schema_bytes ?? 0),
      observedSessions: Number(r.observed_sessions),
      unusedSessions: Number(r.unused_sessions),
      calls: Number(r.calls ?? 0),
      incompleteSessions: Number(r.incomplete_sessions),
      lastCall: r.last_call == null ? null : String(r.last_call),
      wastedBytes: Number(r.wasted_bytes),
    }
    if (Number(r.global_row)) {
      observedSessions = metrics.observedSessions
      wastedBytes = metrics.wastedBytes
      continue
    }
    const source = String(r.source)
    const server = Boolean(Number(r.server_row))
    const total = Boolean(Number(r.total_row))
    const toolName = String(r.tool_name)
    const key = JSON.stringify([source, toolName])
    if (total) {
      if (server) servers.set(source, { ...metrics, source, toolCount: 0, schemaBytes: 0, breakdown: [] })
      else tools.set(key, { ...metrics, source, toolName, breakdown: [] })
    } else {
      const map = server ? serverDetails : details
      const groupKey = server ? source : key
      const list = map.get(groupKey) ?? []
      list.push({ ...metrics, userName: String(r.user_name), project: String(r.project) })
      map.set(groupKey, list)
    }
  }
  const catalogSizes = new Map<string, number>()
  for (const [key, row] of tools) {
    row.breakdown = (details.get(key) ?? []).sort(byWaste)
    const server = servers.get(row.source)!
    server.schemaBytes += row.schemaBytes
    server.toolCount++
    for (const detail of row.breakdown) {
      const ownerKey = JSON.stringify([row.source, detail.userName, detail.project])
      catalogSizes.set(ownerKey, (catalogSizes.get(ownerKey) ?? 0) + detail.schemaBytes)
    }
  }
  for (const [source, row] of servers) {
    row.breakdown = (serverDetails.get(source) ?? []).sort(byWaste)
    for (const detail of row.breakdown) {
      detail.schemaBytes = catalogSizes.get(JSON.stringify([source, detail.userName, detail.project])) ?? 0
    }
  }
  return {
    rows: [...tools.values()].sort(byWaste),
    servers: [...servers.values()].sort(byWaste),
    sessionLimit: 20,
    observedSessions,
    wastedBytes,
  }
}

function byWaste(a: McpUsageMetrics, b: McpUsageMetrics): number {
  return b.wastedBytes - a.wastedBytes || b.schemaBytes - a.schemaBytes
}

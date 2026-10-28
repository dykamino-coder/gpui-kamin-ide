import { getDb } from './database'
import { withStatsWrite } from './database/write-lock'
import { warnLog } from '../logging'
import type { PtySession, ToolDefinition } from '../pty/types'
import type { McpUsageReport } from '../../shared/mcp-report'
import type { DuckDBConnection } from '@duckdb/node-api'

// Metadata only: never retain descriptions, arguments, results or prompts.
// Observation runs outside the MCP response path and cannot change the catalog.
const catalogs = new WeakMap<PtySession, Promise<void>>()
const fingerprints = new WeakMap<PtySession, string>()

export async function writeCatalog(
  db: DuckDBConnection,
  session: Pick<PtySession, 'id' | 'userName' | 'cwd'>,
  tools: Pick<ToolDefinition, 'name' | 'description' | 'inputSchema'>[],
): Promise<void> {
  if (!tools.length) return
  const values = tools.map((tool) => [
    session.id,
    session.userName,
    session.cwd,
    tool.name,
    Buffer.byteLength(JSON.stringify(tool), 'utf8'),
    new Date().toISOString(),
  ])
  await db.run(
    `INSERT INTO mcp_tool_observations
    (bridge_session_id, user_name, project, tool_name, schema_bytes, timestamp)
    VALUES ${values.map(() => '(?, ?, ?, ?, ?, ?)').join(',')}
    ON CONFLICT (bridge_session_id, tool_name) DO UPDATE SET
      schema_bytes = excluded.schema_bytes`,
    values.flat(),
  )
  // The report never reads older sessions. Keep its durable metadata bounded
  // without changing or deleting any CLI transcript or agent context.
  await db.run(
    `DELETE FROM mcp_tool_observations WHERE user_name = ? AND project = ?
      AND bridge_session_id NOT IN (
        SELECT bridge_session_id FROM mcp_tool_observations
        WHERE user_name = ? AND project = ? GROUP BY bridge_session_id
        ORDER BY MAX(timestamp) DESC, bridge_session_id LIMIT 20
      )`,
    [session.userName, session.cwd, session.userName, session.cwd],
  )
}

export function observeMcpCatalog(session: PtySession, tools: ToolDefinition[]): void {
  // Bound diagnostic work independently of the actual (unchanged) MCP response.
  const bounded = tools.slice(0, 1000)
  const fingerprint = JSON.stringify(bounded.map((t) => [t.name, Buffer.byteLength(JSON.stringify(t))]))
  if (fingerprints.get(session) === fingerprint) return
  fingerprints.set(session, fingerprint)
  const pending = Promise.resolve()
    .then(() => getDb())
    .then((db) => withStatsWrite(() => writeCatalog(db, session, bounded)))
    .catch(() => {
      warnLog('MCP catalog observation unavailable')
    })
  catalogs.set(session, pending)
}

export function observeMcpCall(session: PtySession, toolName: string): void {
  // Waiting is confined to telemetry; the caller never awaits this promise.
  void (catalogs.get(session) ?? Promise.resolve())
    .then(async () => {
      const db = await getDb()
      return withStatsWrite(async () => {
        await db.run(
          `UPDATE mcp_tool_observations SET calls = calls + 1
      WHERE bridge_session_id = ? AND tool_name = ?`,
          [session.id, toolName],
        )
      })
    })
    .catch(() => {
      warnLog('MCP call observation unavailable')
    })
}

export async function readMcpReport(db: DuckDBConnection): Promise<McpUsageReport> {
  const rows = (
    await db.runAndReadAll(`
    WITH sessions AS (
      SELECT bridge_session_id, user_name, project, MAX(timestamp) AS listed_at
      FROM mcp_tool_observations GROUP BY ALL
    ), ranked AS (
      SELECT *, ROW_NUMBER() OVER (
        PARTITION BY user_name, project ORDER BY listed_at DESC, bridge_session_id
      ) AS recency FROM sessions
    ), observations AS (
      SELECT o.*, r.recency FROM mcp_tool_observations o
      JOIN ranked r USING (bridge_session_id, user_name, project) WHERE r.recency <= 20
    )
    SELECT user_name, project, tool_name,
      ARG_MIN(schema_bytes, recency) AS schema_bytes,
      COUNT(*) AS observed_sessions, SUM(calls) AS calls,
      COUNT(*) FILTER (WHERE recency < COALESCE((
        SELECT MIN(i.recency) FROM observations i
        WHERE i.user_name = o.user_name AND i.project = o.project
          AND i.tool_name = o.tool_name AND i.calls > 0
      ), 21)) AS unused_sessions
    FROM observations o GROUP BY user_name, project, tool_name
    ORDER BY unused_sessions DESC, user_name, project, tool_name LIMIT 1001
  `)
  ).getRowObjects()
  return {
    sessionLimit: 20,
    truncated: rows.length > 1000,
    rows: rows.slice(0, 1000).map((r) => ({
      userName: String(r.user_name),
      project: String(r.project),
      toolName: String(r.tool_name),
      schemaBytes: Number(r.schema_bytes),
      observedSessions: Number(r.observed_sessions),
      unusedSessions: Number(r.unused_sessions),
      calls: Number(r.calls),
    })),
  }
}

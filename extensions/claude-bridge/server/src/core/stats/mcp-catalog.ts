import { getDb } from './database'
import { withStatsWrite } from './database/write-lock'
import { warnLog } from '../logging'
import type { PtySession, ToolDefinition } from '../pty/types'
import { builtinMcpTool, mcpCatalogBytes, mcpToolSource } from './mcp-tool-source'
export { readMcpReport } from './mcp-report'
import type { DuckDBConnection } from '@duckdb/node-api'

// Metadata only: never retain descriptions, arguments, results or prompts.
// Observation runs outside the MCP response path and cannot change the catalog.
const catalogs = new WeakMap<PtySession, Promise<void>>()
const fingerprints = new WeakMap<PtySession, string>()

export async function writeCatalog(
  db: DuckDBConnection,
  session: Pick<PtySession, 'id' | 'userName' | 'cwd'>,
  tools: ToolDefinition[],
): Promise<void> {
  if (!tools.length) return
  const values = tools.map((tool) => [
    session.id,
    session.userName,
    session.cwd,
    tool.name,
    mcpCatalogBytes(tool),
    new Date().toISOString(),
    mcpToolSource(tool),
  ])
  await db.run(
    `INSERT INTO mcp_tool_observations
    (bridge_session_id, user_name, project, tool_name, schema_bytes, timestamp, source, calls_complete)
    VALUES ${values.map(() => '(?, ?, ?, ?, ?, ?, ?, TRUE)').join(',')}
    ON CONFLICT (bridge_session_id, tool_name) DO UPDATE SET
      schema_bytes = excluded.schema_bytes, source = excluded.source`,
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
  const metadata = new Map(session.registeredTools.map((t) => [t.name, t.serverName]))
  const bounded = tools.slice(0, 1000).map((t) => ({ ...t, serverName: metadata.get(t.name) }))
  const fingerprint = JSON.stringify(bounded.map((t) => [t.name, mcpCatalogBytes(t), mcpToolSource(t)]))
  if (fingerprints.get(session) === fingerprint) return
  fingerprints.set(session, fingerprint)
  const pending = Promise.resolve()
    .then(() => getDb())
    .then((db) => withStatsWrite(() => writeCatalog(db, session, bounded)))
    .catch((error) => {
      if (fingerprints.get(session) === fingerprint) fingerprints.delete(session)
      warnLog('MCP catalog observation unavailable', { reason: String(error) })
    })
  catalogs.set(session, pending)
}

/** A cached catalog may outlive a Bridge session, so a call must create its
 *  observation before incrementing it. Only actual MCP routing calls enter here. */
export async function writeMcpCall(
  db: DuckDBConnection,
  session: Pick<PtySession, 'id' | 'userName' | 'cwd'>,
  tool: ToolDefinition,
): Promise<void> {
  await writeCatalog(db, session, [tool])
  await db.run(
    'UPDATE mcp_tool_observations SET calls = calls + 1, last_called_at = ? WHERE bridge_session_id = ? AND tool_name = ?',
    [new Date().toISOString(), session.id, tool.name],
  )
}

export function observeMcpCall(session: PtySession, toolName: string): Promise<void> {
  const tool = builtinMcpTool(toolName) ?? session.registeredTools.find((t) => t.name === toolName)
  if (!tool) return Promise.resolve()
  // Waiting is confined to telemetry; the caller never awaits this promise.
  return (catalogs.get(session) ?? Promise.resolve())
    .then(async () => {
      const db = await getDb()
      return withStatsWrite(() => writeMcpCall(db, session, tool))
    })
    .catch((error) => {
      warnLog('MCP call observation unavailable', { reason: String(error) })
    })
}

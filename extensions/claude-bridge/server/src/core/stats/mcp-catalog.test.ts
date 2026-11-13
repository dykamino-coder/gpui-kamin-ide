import { afterAll, beforeAll, beforeEach, expect, it, vi } from 'vitest'
import { DuckDBInstance, type DuckDBConnection } from '@duckdb/node-api'
import { Hono } from 'hono'
import { initSchema } from './database/schema'
import { observeMcpCall, observeMcpCatalog, readMcpReport, writeCatalog, writeMcpCall } from './mcp-catalog'
import { builtinMcpTool, mcpCatalogBytes } from './mcp-tool-source'
import type { PtySession, ToolDefinition } from '../pty/types'
import { registerMcpReportRoutes } from '../dashboard/controllers/mcp-report'

const { getDb } = vi.hoisted(() => ({ getDb: vi.fn() }))
vi.mock('./database', () => ({ getDb }))
let instance: DuckDBInstance
let db: DuckDBConnection
const rare: ToolDefinition = {
  name: 'Rare',
  description: 'Never called',
  inputSchema: { type: 'object' },
  serverName: 'docs',
}
const read: ToolDefinition = {
  name: 'SearchDocs',
  description: 'Search docs',
  inputSchema: { type: 'object' },
  serverName: 'docs',
}
const owner = (id: string, userName = 'alice', cwd = '/a') => ({ id, userName, cwd })
const session = (id: string, registeredTools: ToolDefinition[] = []) =>
  ({ ...owner(id), registeredTools }) as PtySession
beforeAll(async () => {
  instance = await DuckDBInstance.create(':memory:')
  db = await instance.connect()
  await initSchema(db)
})
beforeEach(async () => {
  await db.run('DELETE FROM mcp_tool_observations')
  getDb.mockReset().mockResolvedValue(db)
})
afterAll(() => {
  db.closeSync()
  instance.closeSync()
})

it('aggregates tools across users/projects, retains 20 sessions per owner and sums every zero-call session', async () => {
  for (let i = 1; i <= 23; i++) {
    await writeCatalog(db, owner(`s${i}`), i === 22 ? [read] : [read, rare])
    await db.run('UPDATE mcp_tool_observations SET timestamp = ? WHERE bridge_session_id = ?', [
      new Date(2026, 0, i).toISOString(),
      `s${i}`,
    ])
  }
  await writeMcpCall(db, owner('s21'), read)
  await writeMcpCall(db, owner('s21'), read)
  // A repeated listing preserves calls and the original session's recency.
  await writeCatalog(db, owner('s21'), [read, rare])
  await writeCatalog(db, owner('other-user', 'bob'), [read, rare])
  await writeCatalog(db, owner('other-project', 'alice', '/b'), [read, rare])
  const report = await readMcpReport(db)
  expect(report.rows).toHaveLength(2)
  expect(report.observedSessions).toBe(22)
  const row = report.rows.find((r) => r.toolName === read.name)!
  expect(row).toMatchObject({
    source: 'docs',
    observedSessions: 22,
    unusedSessions: 21,
    calls: 2,
    incompleteSessions: 0,
  })
  expect(row.lastCall).toMatch(/^\d{4}-/)
  expect(row.breakdown).toHaveLength(3)
  expect(row.breakdown.find((r) => r.userName === 'alice' && r.project === '/a')).toMatchObject({
    observedSessions: 20,
    unusedSessions: 19,
    calls: 2,
  })
  expect(report.rows.find((r) => r.toolName === rare.name)).toMatchObject({
    observedSessions: 21,
    unusedSessions: 21,
    calls: 0,
    schemaBytes: mcpCatalogBytes(rare),
    wastedBytes: 21 * mcpCatalogBytes(rare),
    lastCall: null,
  })
  const retained = (
    await db.runAndReadAll(
      "SELECT COUNT(DISTINCT bridge_session_id) AS n FROM mcp_tool_observations WHERE user_name = 'alice' AND project = '/a'",
    )
  ).getRowObjects()[0]
  expect(Number(retained?.n)).toBe(20)
})

it('does not lose Bash/Edit calls when a resumed CLI reuses its catalog', async () => {
  const s = session('cached')
  await Promise.all([observeMcpCall(s, 'Bash'), observeMcpCall(s, 'Edit'), observeMcpCall(s, 'Bash')])
  const report = await readMcpReport(db)
  expect(report.rows.find((r) => r.toolName === 'Bash')).toMatchObject({
    source: 'built-in',
    observedSessions: 1,
    calls: 2,
    unusedSessions: 0,
  })
  expect(report.rows.find((r) => r.toolName === 'Edit')).toMatchObject({
    source: 'built-in',
    observedSessions: 1,
    calls: 1,
    unusedSessions: 0,
  })
  expect(report.rows.every((r) => r.lastCall != null)).toBe(true)
  expect(report.wastedBytes).toBe(0)
})

it('retains external server metadata without adding it to wire catalog bytes, and waits for listing writes', async () => {
  const s = session('external', [read, rare])
  const { serverName: _source, ...wire } = read
  observeMcpCatalog(s, [wire, rare])
  await observeMcpCall(s, read.name)
  const report = await readMcpReport(db)
  expect(report.rows.find((r) => r.toolName === read.name)).toMatchObject({
    source: 'docs',
    calls: 1,
    schemaBytes: Buffer.byteLength(JSON.stringify(wire)),
  })
  expect(report.servers[0]).toMatchObject({
    source: 'docs',
    toolCount: 2,
    observedSessions: 1,
    unusedSessions: 0,
    calls: 1,
    wastedBytes: mcpCatalogBytes(rare),
  })
  expect(report.servers[0]?.breakdown[0]?.schemaBytes).toBe(mcpCatalogBytes(read) + mcpCatalogBytes(rare))
})

it('retries a catalog observation after a failed write instead of permanently deduplicating it', async () => {
  const s = session('retry', [read])
  getDb.mockRejectedValueOnce(new Error('fixture storage failure'))
  observeMcpCatalog(s, [read])
  await observeMcpCall(s, 'unknown')
  await vi.waitFor(() => expect(getDb).toHaveBeenCalledTimes(1))
  // Let the failed observation remove its fingerprint.
  await new Promise((resolve) => setImmediate(resolve))
  observeMcpCatalog(s, [read])
  await observeMcpCall(s, read.name)
  expect((await readMcpReport(db)).rows[0]).toMatchObject({ calls: 1, source: 'docs' })
})

it('weights unused bytes by the catalog actually offered, sorts by waste and counts server sessions once', async () => {
  await writeCatalog(db, owner('small'), [read, rare])
  const large = { ...read, description: 'x'.repeat(2048) }
  await writeCatalog(db, owner('large'), [large, rare])
  await writeMcpCall(db, owner('large'), rare)
  const report = await readMcpReport(db)
  expect(report.rows[0]).toMatchObject({
    toolName: read.name,
    wastedBytes: mcpCatalogBytes(read) + mcpCatalogBytes(large),
  })
  expect(report.servers[0]).toMatchObject({ observedSessions: 2, unusedSessions: 1, calls: 1 })
  expect(report.wastedBytes).toBe(mcpCatalogBytes(read) + mcpCatalogBytes(large) + mcpCatalogBytes(rare))
})

it('migrates legacy counters idempotently, preserves calls and excludes uncertain zeros from waste', async () => {
  await db.run('DROP TABLE mcp_tool_observations')
  await db.run(`CREATE TABLE mcp_tool_observations (
    bridge_session_id TEXT, user_name TEXT, project TEXT, tool_name TEXT, schema_bytes INTEGER,
    timestamp TEXT, calls BIGINT DEFAULT 0, PRIMARY KEY (bridge_session_id, tool_name))`)
  await db.run(
    "INSERT INTO mcp_tool_observations VALUES ('legacy', 'alice', '/a', 'Bash', 100, '2026-01-01', 3), ('legacy', 'alice', '/a', 'Edit', 80, '2026-01-01', 0)",
  )
  await initSchema(db)
  await initSchema(db)
  // A fresh listing cannot certify the historical part of this session.
  await writeCatalog(db, owner('legacy'), [builtinMcpTool('Bash')!, builtinMcpTool('Edit')!])
  await writeCatalog(db, owner('legacy'), [rare])
  const report = await readMcpReport(db)
  expect(report.rows.find((r) => r.toolName === 'Bash')).toMatchObject({
    calls: 3,
    lastCall: null,
    incompleteSessions: 1,
    source: 'built-in',
  })
  expect(report.rows.find((r) => r.toolName === 'Edit')).toMatchObject({
    calls: 0,
    unusedSessions: 0,
    wastedBytes: 0,
    incompleteSessions: 1,
  })
  expect(report.servers.find((r) => r.source === 'built-in')).toMatchObject({
    unusedSessions: 0,
    incompleteSessions: 1,
  })
  expect(report.wastedBytes).toBe(mcpCatalogBytes(rare))
})

it('returns tool aggregates beyond the old 1000 flat-row cutoff without mixing projects', async () => {
  const tools = Array.from({ length: 501 }, (_, i) => ({ ...read, name: `tool-${i}` }))
  await writeCatalog(db, owner('one'), tools)
  await writeCatalog(db, owner('two', 'bob', '/b'), tools)
  const report = await readMcpReport(db)
  expect(report.rows).toHaveLength(501)
  expect(report.rows.every((r) => r.observedSessions === 2 && r.breakdown.length === 2)).toBe(true)
  expect(report.servers[0]).toMatchObject({ observedSessions: 2, toolCount: 501 })
})

it('rejects a per-user API token before reading the global report', async () => {
  const app = new Hono()
  app.use('*', async (c, next) => {
    c.set('authKind' as never, 'api' as never)
    await next()
  })
  registerMcpReportRoutes(app)
  expect((await app.request('/api/dashboard/mcp-report')).status).toBe(403)
})

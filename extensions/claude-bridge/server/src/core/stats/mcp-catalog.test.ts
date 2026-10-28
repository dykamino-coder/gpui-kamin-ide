import { afterAll, beforeAll, expect, it } from 'vitest'
import { DuckDBInstance, type DuckDBConnection } from '@duckdb/node-api'
import { Hono } from 'hono'
import { initSchema } from './database/schema'
import { readMcpReport, writeCatalog } from './mcp-catalog'
import { registerMcpReportRoutes } from '../dashboard/controllers/mcp-report'

let instance: DuckDBInstance
let db: DuckDBConnection
beforeAll(async () => {
  instance = await DuckDBInstance.create(':memory:')
  db = await instance.connect()
  await initSchema(db)
})
afterAll(() => {
  db.closeSync()
  instance.closeSync()
})

it('counts only sessions offering a tool, stops unused streak at a call, and separates owners/projects', async () => {
  const tools = [
    { name: 'Read', description: 'Read a file', inputSchema: { type: 'object' } },
    { name: 'Rare', description: 'Not called', inputSchema: { type: 'object' } },
  ]
  for (let i = 1; i <= 23; i++) {
    await writeCatalog(db, { id: `s${i}`, userName: 'alice', cwd: '/a' }, i === 22 ? [tools[0]!] : tools)
    await db.run('UPDATE mcp_tool_observations SET timestamp = ? WHERE bridge_session_id = ?', [
      new Date(2026, 0, i).toISOString(),
      `s${i}`,
    ])
  }
  await db.run("UPDATE mcp_tool_observations SET calls = 2 WHERE bridge_session_id = 's21' AND tool_name = 'Read'")
  // Re-listing must preserve calls and not count a new session.
  await writeCatalog(db, { id: 's21', userName: 'alice', cwd: '/a' }, tools)
  await writeCatalog(db, { id: 'other-user', userName: 'bob', cwd: '/a' }, tools)
  await writeCatalog(db, { id: 'other-project', userName: 'alice', cwd: '/b' }, tools)
  const report = await readMcpReport(db)
  const alice = report.rows.filter((r) => r.userName === 'alice' && r.project === '/a')
  expect(alice.find((r) => r.toolName === 'Read')).toMatchObject({ observedSessions: 20, unusedSessions: 2, calls: 2 })
  expect(alice.find((r) => r.toolName === 'Rare')).toMatchObject({ observedSessions: 19, unusedSessions: 19, calls: 0 })
  expect(report.rows).toHaveLength(6)
  expect(alice[0]?.schemaBytes).toBe(Buffer.byteLength(JSON.stringify(tools[1])))
  const retained = (
    await db.runAndReadAll(
      "SELECT COUNT(DISTINCT bridge_session_id) AS n FROM mcp_tool_observations WHERE user_name = 'alice' AND project = '/a'",
    )
  ).getRowObjects()[0]
  expect(Number(retained?.n)).toBe(20)
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

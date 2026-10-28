import type { Hono } from 'hono'
import { denyApiToken } from '../authz'
import { getDb } from '../../stats/database'
import { readMcpReport } from '../../stats/mcp-catalog'

export function registerMcpReportRoutes(api: Hono): void {
  api.get('/api/dashboard/mcp-report', async (c) => {
    const denied = denyApiToken(c)
    if (denied) return denied
    return c.json(await readMcpReport(await getDb()))
  })
}

// ============================================================================
// Dashboard usage endpoint (Claude Max usage data)
// ============================================================================

import type { Hono } from 'hono'

export function registerUsageRoutes(api: Hono): void {
  // GET /api/dashboard/usage -- Claude Max usage data (session %, weekly %)
  api.get('/api/dashboard/usage', async (c) => {
    const force = c.req.query('force') === 'true' || c.req.query('force') === '1'
    try {
      const { captureUsage } = await import('../../server/utils/usage-capture')
      const data = await captureUsage(force)
      return c.json(data)
    } catch (err) {
      return c.json({ error: err instanceof Error ? err.message : 'Usage capture failed' }, 500)
    }
  })
}

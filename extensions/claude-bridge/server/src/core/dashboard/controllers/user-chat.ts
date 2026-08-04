// ============================================================================
// Dashboard user chat export endpoint
// ============================================================================

import type { Hono } from 'hono'
import { stats } from '../../stats/tracker'
import { denyForeignUser } from '../authz'

export function registerUserChatRoutes(api: Hono): void {
  // GET /api/dashboard/user-chat/:userName -- user messages only (for chat export)
  api.get('/api/dashboard/user-chat/:userName', (c) => {
    const userName = c.req.param('userName')
    const denied = denyForeignUser(c, userName); if (denied) return denied
    const limit = parseInt(c.req.query('limit') || '500')
    const entries = stats.getFilteredRequests({ userName, isUserRequest: true }, limit, 0)
    return c.json(entries)
  })
}

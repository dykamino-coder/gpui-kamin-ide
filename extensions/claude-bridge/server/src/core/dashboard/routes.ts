// ============================================================================
// Dashboard route registration (thin orchestrator)
// ============================================================================

import { Hono } from 'hono'
import { dashboardAuthMiddleware } from '../middleware/dashboard-auth'
import { registerAuthRoutes } from './controllers/auth'
import { registerStatusRoutes } from './controllers/status'
import { registerStatsRoutes } from './controllers/stats'
import { registerAccountRoutes } from './controllers/account'
import { registerHealthCheckRoutes } from './controllers/health-check'
import { registerSettingsRoutes } from './controllers/settings'
import { registerUserChatRoutes } from './controllers/user-chat'
import { registerUsageRoutes } from './controllers/usage'
import { registerTokenRoutes } from './controllers/tokens'
import { registerSessionRoutes } from './controllers/sessions'
import { registerUserRoutes } from './controllers/users'
import { registerTranscriptExportRoutes } from './controllers/transcripts-export'

export function createDashboardRoutes(): Hono {
  const api = new Hono()

  // Auth routes (login/session check) — must be before auth middleware
  registerAuthRoutes(api)

  // Dashboard auth middleware — protects all other /api/dashboard/* routes
  api.use('/api/dashboard/*', dashboardAuthMiddleware)

  registerStatusRoutes(api)
  registerStatsRoutes(api)
  registerAccountRoutes(api)
  registerHealthCheckRoutes(api)
  registerSettingsRoutes(api)
  registerUserChatRoutes(api)
  registerUsageRoutes(api)
  registerTokenRoutes(api)
  registerSessionRoutes(api)
  registerUserRoutes(api)
  registerTranscriptExportRoutes(api)

  return api
}

// ============================================================================
// Dashboard Auth Middleware — protects /api/dashboard/* routes
// ============================================================================

import type { Context, Next } from 'hono'
import { isDashboardAuthEnabled, validateDashboardSession } from '../auth/dashboard-auth'
import { resolveTokenSync } from '../auth/tokens'

/** Paths that don't require dashboard authentication */
const PUBLIC_PATHS = [
  '/api/dashboard/auth/login',
  '/api/dashboard/auth/session',
  // Token listing + creation is used by the KaminIDE client's Connection
  // settings to resolve the token → name mapping and bootstrap a new
  // install. Requiring dashboard auth here would create a chicken-and-
  // egg problem: the user has no token yet, so they can't log in, so
  // they can't mint a token.
  '/api/dashboard/tokens',
]

export async function dashboardAuthMiddleware(c: Context, next: Next) {
  // Skip if auth not enabled (no env vars set)
  if (!isDashboardAuthEnabled()) return next()

  // Skip public auth endpoints (exact or prefix match for /tokens/:id)
  const path = c.req.path
  if (PUBLIC_PATHS.some(p => path === p || path.startsWith(p + '/'))) return next()

  // Check for session token
  const auth = c.req.header('authorization')
  if (!auth?.startsWith('Bearer ')) {
    return c.json({ error: 'Authentication required' }, 401)
  }

  const token = auth.slice(7)
  const session = await validateDashboardSession(token)
  if (session) {
    c.set('authKind', 'admin')
    await next()
    return
  }

  // Fallback: accept a per-user API token (the same one the client uses for
  // /ws/session). Stamp the resolved userName onto the request context so
  // downstream endpoints can scope responses to the caller's data only —
  // an API-token holder must never see another user's stats.
  const resolved = resolveTokenSync(token)
  if (resolved) {
    c.set('authKind', 'api')
    c.set('apiUserName', resolved.userName)
    c.set('apiTokenId', resolved.tokenId)
    await next()
    return
  }

  return c.json({ error: 'Session expired or invalid' }, 401)
}

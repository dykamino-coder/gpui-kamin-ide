// ============================================================================
// Dashboard Auth Controller — login / logout / session check
// ============================================================================

import type { Hono } from 'hono'
import {
  isDashboardAuthEnabled,
  validateCredentials,
  createDashboardSession,
  deleteDashboardSession,
  validateDashboardSession,
} from '../../auth/dashboard-auth'

export function registerAuthRoutes(api: Hono): void {
  // Check if auth is enabled and whether current session is valid
  api.get('/api/dashboard/auth/session', async (c) => {
    if (!isDashboardAuthEnabled()) {
      return c.json({ authEnabled: false, authenticated: true })
    }

    const auth = c.req.header('authorization')
    if (!auth?.startsWith('Bearer ')) {
      return c.json({ authEnabled: true, authenticated: false })
    }

    const token = auth.slice(7)
    const session = await validateDashboardSession(token)
    if (!session) {
      return c.json({ authEnabled: true, authenticated: false })
    }

    return c.json({
      authEnabled: true,
      authenticated: true,
      expiresAt: session.expiresAt,
    })
  })

  // Login
  api.post('/api/dashboard/auth/login', async (c) => {
    if (!isDashboardAuthEnabled()) {
      return c.json({ error: 'Dashboard authentication is not configured' }, 400)
    }

    const body = await c.req.json().catch(() => null)
    if (!body || typeof body.username !== 'string' || typeof body.password !== 'string') {
      return c.json({ error: 'Username and password are required' }, 400)
    }

    if (!validateCredentials(body.username, body.password)) {
      return c.json({ error: 'Invalid username or password' }, 401)
    }

    const session = await createDashboardSession()
    return c.json({
      token: session.token,
      expiresAt: session.expiresAt,
    })
  })

  // Logout
  api.post('/api/dashboard/auth/logout', async (c) => {
    const auth = c.req.header('authorization')
    if (auth?.startsWith('Bearer ')) {
      await deleteDashboardSession(auth.slice(7))
    }
    return c.json({ ok: true })
  })
}

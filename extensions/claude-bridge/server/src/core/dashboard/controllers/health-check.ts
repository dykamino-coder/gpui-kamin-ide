// ============================================================================
// Dashboard health refresh endpoint
// ============================================================================

import type { Hono } from 'hono'
import { eventBus } from '../../events/bus'
import { getAllSessions } from '../../pty/session-manager'
import { TtlCache } from '../../utils/ttl-cache'
import { injectProxyEnv } from '../../config/settings'

const healthCache = new TtlCache<Record<string, unknown>>(15 * 60 * 1000)

export function registerHealthCheckRoutes(api: Hono): void {
  // POST /api/dashboard/health/refresh -- re-run health check via CLI
  // Cached for 15 minutes unless force=true query param is set (manual button click)
  api.post('/api/dashboard/health/refresh', async (c) => {
    const force = c.req.query('force') === 'true'
    const cached = healthCache.get(force)
    if (cached) return c.json(cached)

    try {
      const { execFile } = await import('child_process')
      const { promisify } = await import('util')
      const execFileAsync = promisify(execFile)
      const claudeCmd = process.platform === 'win32' ? 'claude.cmd' : 'claude'

      const env = { ...process.env, HOME: process.env.HOME || '/home/bridge' } as Record<string, string>
      injectProxyEnv(env)

      const [authResult, versionResult] = await Promise.allSettled([
        execFileAsync(claudeCmd, ['--dangerously-skip-permissions', 'auth', 'status'], {
          timeout: 10_000,
          env,
        }),
        execFileAsync(claudeCmd, ['--version'], {
          timeout: 5_000,
          env,
        }),
      ])

      if (authResult.status === 'rejected') throw authResult.reason
      const parsed = JSON.parse(authResult.value.stdout.trim())

      const cliVersion = versionResult.status === 'fulfilled'
        ? versionResult.value.stdout.trim()
        : 'unknown'

      const sessions = getAllSessions()
      const health = {
        sdk: parsed.loggedIn === true,
        account: {
          email: parsed.email || parsed.accountEmail,
          plan: parsed.plan || parsed.accountPlan,
        },
        apiPing: null,
        cliVersion,
        sessions: {
          total: sessions.length,
          active: sessions.filter(s => s.state === 'running').length,
        },
      }
      healthCache.set(health)
      eventBus.emit('health:updated', health)
      return c.json(health)
    } catch (err) {
      const health = { sdk: false, account: null, apiPing: null, error: err instanceof Error ? err.message : 'Health check failed' }
      eventBus.emit('health:updated', health)
      return c.json(health, 503)
    }
  })
}

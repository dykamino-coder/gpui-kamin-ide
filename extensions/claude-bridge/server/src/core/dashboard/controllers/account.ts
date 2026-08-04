// ============================================================================
// Dashboard account & auth-status endpoints
// ============================================================================

import type { Hono } from 'hono'
import { eventBus } from '../../events/bus'
import { readClaudeLocalInfo } from '../helpers/system-info'
import { TtlCache } from '../../utils/ttl-cache'

const accountCache = new TtlCache<Record<string, unknown>>(5 * 60 * 1000)
const authCache = new TtlCache<{ loggedIn: boolean; authMethod?: string; apiProvider?: string }>(2 * 60 * 1000)

export function registerAccountRoutes(api: Hono): void {
  // GET /api/dashboard/account -- lightweight account info
  // Cached for 5 minutes unless force=true query param is set
  api.get('/api/dashboard/account', async (c) => {
    const force = c.req.query('force') === 'true'
    const cached = accountCache.get(force)
    if (cached) return c.json(cached)

    // Use local Claude files + CLI auth status
    const localInfo = readClaudeLocalInfo()

    try {
      const { execFile } = await import('child_process')
      const { promisify } = await import('util')
      const execFileAsync = promisify(execFile)
      const { stdout } = await execFileAsync(process.platform === 'win32' ? 'claude.cmd' : 'claude', ['--dangerously-skip-permissions', 'auth', 'status'], {
        timeout: 10000,
        env: { ...process.env, HOME: process.env.HOME || '/home/bridge' },
      })
      const parsed = JSON.parse(stdout.trim())

      const account = {
        email: parsed.email || parsed.accountEmail || localInfo.email || null,
        plan: parsed.plan || parsed.accountPlan || localInfo.plan || null,
        organization: parsed.organization || null,
        expiresAt: localInfo.expiresAt || null,
        displayName: localInfo.displayName || null,
      }
      accountCache.set(account)
      eventBus.emit('account:updated', account)
      return c.json(account)
    } catch (err) {
      const fallback = {
        email: localInfo.email || null,
        plan: localInfo.plan || null,
        organization: null,
        expiresAt: localInfo.expiresAt || null,
        displayName: localInfo.displayName || null,
        error: err instanceof Error ? err.message : 'CLI unavailable, using cached credentials',
      }
      if (!accountCache.get()) accountCache.set(fallback)
      return c.json(fallback)
    }
  })

  // GET /api/dashboard/auth-status -- check if Claude CLI is authenticated
  // Cached for 2 minutes
  api.get('/api/dashboard/auth-status', async (c) => {
    const force = c.req.query('force') === 'true'

    const cachedAuth = authCache.get(force)
    if (cachedAuth) return c.json(cachedAuth)

    try {
      const { execFile } = await import('child_process')
      const { promisify } = await import('util')
      const execFileAsync = promisify(execFile)
      const { stdout, stderr } = await execFileAsync(process.platform === 'win32' ? 'claude.cmd' : 'claude', ['--dangerously-skip-permissions', 'auth', 'status'], {
        timeout: 10000,
        env: { ...process.env, HOME: process.env.HOME || '/home/bridge' },
      })
      const trimmed = stdout.trim()
      if (!trimmed) {
        const fallback = { loggedIn: false, error: `Empty stdout from \`claude auth status\`. stderr: ${(stderr || '').slice(0, 200)}` }
        authCache.set(fallback as any)
        return c.json(fallback)
      }
      let parsed: any
      try {
        parsed = JSON.parse(trimmed)
      } catch {
        const fallback = { loggedIn: false, error: `claude auth status returned non-JSON: ${trimmed.slice(0, 200)}` }
        authCache.set(fallback as any)
        return c.json(fallback)
      }
      const status = {
        loggedIn: parsed.loggedIn === true,
        authMethod: parsed.authMethod || undefined,
        apiProvider: parsed.apiProvider || undefined,
      }
      authCache.set(status)
      return c.json(status)
    } catch (err) {
      const fallback = { loggedIn: false, error: err instanceof Error ? err.message : 'Auth check failed' }
      authCache.set(fallback as any)
      return c.json(fallback)
    }
  })
}

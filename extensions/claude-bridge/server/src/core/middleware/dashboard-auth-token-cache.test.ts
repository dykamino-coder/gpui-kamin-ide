// Тянет logging → потоковый прокси → @peculiar/x509 → tsyringe (нужен reflect).
import 'reflect-metadata'

import { mkdtempSync, rmSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { Hono } from 'hono'
import { afterAll, beforeAll, describe, expect, it, vi } from 'vitest'

/**
 * Token create/rename/delete drop the in-memory token cache. The dashboard
 * middleware resolved API tokens with the SYNC lookup, which returns null
 * while the cache is empty: every valid API token got 401 right after a token
 * was managed, until some unrelated async call re-read the table.
 * Real DuckDB in a temp data directory; dashboard auth enabled.
 */

let dir = ''
let lc: typeof import('../stats/database/lifecycle')
let tokens: typeof import('../auth/tokens')
let app: Hono

beforeAll(async () => {
  dir = mkdtempSync(join(tmpdir(), 'token-cache-db-'))
  vi.spyOn(process, 'cwd').mockReturnValue(dir)
  vi.stubEnv('DASHBOARD_USER', 'admin')
  vi.stubEnv('DASHBOARD_PASSWORD', 'synthetic-admin-password')
  vi.resetModules()
  lc = await import('../stats/database/lifecycle')
  tokens = await import('../auth/tokens')
  const { dashboardAuthMiddleware } = await import('./dashboard-auth')
  await lc.initStatsDb()
  await tokens.primeTokenCache()
  app = new Hono()
  app.use('/api/dashboard/*', dashboardAuthMiddleware)
  app.get('/api/dashboard/stats', (c) => c.json({ user: c.get('apiUserName' as never) ?? null }))
})

afterAll(async () => {
  await lc.closeDb()
  vi.unstubAllEnvs()
  vi.restoreAllMocks()
  rmSync(dir, { recursive: true, force: true })
})

const call = (token: string) => app.request('/api/dashboard/stats', { headers: { authorization: `Bearer ${token}` } })

describe('dashboard API-token auth right after token management', () => {
  it('a valid token passes at once after create, rename and delete of tokens', async () => {
    const a = await tokens.createToken('alice')
    let res = await call(a.token)
    expect(res.status).toBe(200)
    expect(await res.json()).toEqual({ user: 'alice' })

    const b = await tokens.createToken('bob')
    await tokens.updateToken(a.id, { name: 'alice-renamed' })
    res = await call(a.token)
    expect(res.status).toBe(200)
    expect(await res.json()).toEqual({ user: 'alice-renamed' })

    await tokens.deleteToken(b.id)
    expect((await call(a.token)).status).toBe(200)
    // The deleted one is refused; an unknown one too.
    expect((await call(b.token)).status).toBe(401)
    expect((await call('not-a-token')).status).toBe(401)
  })
})

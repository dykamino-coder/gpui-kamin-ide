import 'reflect-metadata'
import { afterAll, beforeAll, describe, expect, it, vi } from 'vitest'
import { DuckDBInstance, type DuckDBConnection } from '@duckdb/node-api'
import { Hono } from 'hono'
import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'

const fixture = vi.hoisted(() => ({
  authEnabled: false,
  dataDir: '',
  account: {
    loggedIn: true,
    email: 'synthetic@example.invalid',
    planType: 'test',
    accountType: 'chatgpt',
    pendingLogin: null,
    error: null,
  },
}))
vi.mock('../stats/database/lifecycle', () => ({ getDb: vi.fn(), getDataDir: () => fixture.dataDir }))
vi.mock('../auth/dashboard-auth', () => ({
  isDashboardAuthEnabled: () => fixture.authEnabled,
  validateDashboardSession: async (token: string) => (token === 'synthetic-admin' ? { id: 'admin' } : null),
  validateCredentials: (user: string, password: string) => user === 'synthetic' && password === 'test-only',
}))
vi.mock('../logging', () => ({
  DEBUG: false,
  VERBOSE: false,
  LOG_PATH: '',
  warnLog: vi.fn(),
  errorLog: vi.fn(),
  infoLog: vi.fn(),
  debugLog: vi.fn(),
  verboseLog: vi.fn(),
  writeLogFile: vi.fn(),
}))
vi.mock('../dashboard/helpers/system-info', () => ({
  getSDKVersion: () => 'synthetic',
  readClaudeLocalInfo: () => ({ email: 'synthetic@example.invalid', expiresAt: 1800000000000 }),
}))
vi.mock('../pty/dev-cli', () => ({
  claudeInvocation: () => {
    throw new Error('Synthetic CLI unavailable')
  },
}))
vi.mock('../dashboard/system-stats', () => ({
  startSystemStatsSampler: vi.fn(),
  getSystemStats: () => ({ cpuPercent: 1 }),
  getProcessTreeStats: () => null,
}))
vi.mock('../codex/codex-runtime', () => ({
  codexStatus: async () => ({ enabled: true }),
  codexRuntime: () => ({
    onAccountChange: vi.fn(),
    accountStatus: async () => fixture.account,
    rateLimits: async () => ({
      buckets: [
        {
          id: 'test',
          name: 'Test',
          primary: { usedPercent: 20, windowMinutes: 300, resetsAt: 1800000000 },
          secondary: null,
          reached: null,
        },
      ],
      credits: null,
      planType: 'test',
      usageAllowed: true,
      readAt: 1800000000,
    }),
    startDeviceLogin: async () => ({
      loginId: 'test',
      verificationUrl: 'https://example.invalid',
      userCode: 'TEST',
      startedAt: 1,
    }),
    cancelLogin: vi.fn(),
    logout: vi.fn(),
    models: async () => [{ id: 'synthetic', displayName: 'Synthetic' }],
  }),
}))

import { createApp } from './index'
import { getDb } from '../stats/database/lifecycle'
import { initSchema } from '../stats/database/schema'
import { startRollupBuild } from '../stats/database/rollup'
import { contracts, createSpecification, registeredContracts, openApiPath, contractMiddleware } from '../api/registry'
import { recordEvent } from '../telemetry/store'
import { resolveToken } from '../auth/tokens'
import { tokenHash } from '../sync/routes'
import { generateSyntheticStats } from '../stats/database/synthetic-stats'

let instance: DuckDBInstance
let db: DuckDBConnection
beforeAll(async () => {
  fixture.dataDir = fs.mkdtempSync(path.join(process.env.BRIDGE_TEST_DATA_DIR || os.tmpdir(), 'bridge-api-'))
  fs.mkdirSync(path.join(fixture.dataDir, 'bridge-sync'))
  instance = await DuckDBInstance.create(':memory:', { threads: '2', memory_limit: '256MB' })
  db = await instance.connect()
  vi.mocked(getDb).mockResolvedValue(db)
  await initSchema(db)
  await generateSyntheticStats(db, {
    users: 2,
    sessions: 8,
    eventsPerSession: 9,
    days: 7,
    endIso: new Date().toISOString(),
  })
  await startRollupBuild(async () => db)
  await recordEvent({
    sessionId: 'synthetic-session',
    userName: 'Synthetic',
    eventName: 'claude_code.api_request',
    timestamp: new Date().toISOString(),
    data: { model: 'synthetic', inputTokens: 12, outputTokens: 4 },
  })
}, 30000)
afterAll(() => {
  db?.disconnectSync()
  instance?.closeSync()
  if (fixture.dataDir) fs.rmSync(fixture.dataDir, { recursive: true, force: true })
})

describe('admin API discovery', () => {
  it('serves OpenAPI 3.1 and covers every production/development route', async () => {
    for (const mode of ['production', 'test']) {
      vi.stubEnv('NODE_ENV', mode)
      const app = createApp()
      const response = await app.request('/api/openapi.json')
      expect(response.status).toBe(200)
      const spec = await response.json()
      expect(spec.openapi).toBe('3.1.0')
      const registered = registeredContracts(app)
      expect(registered.length).toBeGreaterThan(90)
      expect(new Set(contracts.map((c) => `${c.method} ${c.path}`)).size).toBe(contracts.length)
      for (const route of registered)
        expect(spec.paths[openApiPath(route.path)][route.method.toLowerCase()]).toBeDefined()
      expect(spec.paths['/api/test/ping'] !== undefined).toBe(mode !== 'production')
      expect(spec.paths['/api/dashboard/tokens/{id}'].patch).toBeDefined()
      expect(spec.paths['/api/dashboard/tokens'].get.security).toEqual([{ adminBearer: [] }, {}])
      expect(spec.paths['/api/dashboard/tokens/{id}'].patch.security).toEqual([{ adminBearer: [] }, {}])
      const undocumented = createApp()
      undocumented.get('/accidentally-undocumented', (c) => c.json({ ok: true }))
      expect(() => createSpecification(undocumented)).toThrow('GET /accidentally-undocumented')
    }
    vi.unstubAllEnvs()
  })

  it('serves Scalar using only local assets', async () => {
    const response = await createApp().request('/api/docs')
    expect(response.status).toBe(200)
    const html = await response.text()
    expect(html).toContain('Bridge API')
    expect(html).toContain('/ui/assets/')
    expect(html).not.toMatch(/https?:\/\/(?:cdn|unpkg|fonts)\./)
  })

  it('validates real responses for populated and empty synthetic dashboard data', async () => {
    const app = createApp()
    for (const url of [
      '/health',
      '/api/bridge-version',
      '/api/download/check',
      '/api/dashboard/status',
      '/api/dashboard/config',
      '/api/dashboard/proxy-settings',
      '/api/dashboard/account',
      '/api/dashboard/auth-status',
      '/api/dashboard/codex/account',
      '/api/dashboard/codex/limits',
      '/api/dashboard/codex/models',
      '/api/dashboard/terminal',
      '/api/dashboard/sessions',
      '/api/dashboard/sessions/stats',
      '/api/dashboard/sessions/tree?tokenId=synthetic',
      '/api/dashboard/stats',
      '/api/dashboard/stats/timeseries',
      '/api/dashboard/stats/overview',
      '/api/dashboard/stats/overview?agg=hm&tz=300',
      '/api/dashboard/stats/users-overview',
      '/api/dashboard/users',
      '/api/dashboard/tokens/tok-user00/sessions',
      '/api/dashboard/users/Synthetic/requests',
      '/api/dashboard/user-chat/Synthetic',
      '/api/dashboard/requests',
      '/api/dashboard/errors/count',
      '/api/dashboard/errors/timeseries',
      '/api/dashboard/errors/recent',
      '/api/dashboard/mcp-report',
    ]) {
      const response = await app.request(url)
      expect(response.status, url).toBe(200)
      const body = await response.json()
      const route = registeredContracts(app).find(
        (c) =>
          c.method === 'GET' && new RegExp('^' + c.path.replace(/:[^/]+/g, '[^/]+') + '$').test(url.split('?')[0]!),
      )!
      expect(route.response?.safeParse(body).success, url).toBe(true)
    }
    const requests = await (await app.request('/api/dashboard/requests')).json()
    expect(requests).toHaveLength(1)
    expect(requests[0].inputTokens).toBe(12)
    const users = await (await app.request('/api/dashboard/users')).json()
    expect(users.length).toBeGreaterThan(0)
    const cards = await (await app.request('/api/dashboard/stats/users-overview')).json()
    expect(Object.keys(cards.cards).length).toBeGreaterThan(0)
  })

  it('validates token writes before mutation and masks non-admin read/rename responses', async () => {
    const app = createApp()
    const write = (url: string, method: string, body: unknown) =>
      app.request(url, { method, headers: { 'content-type': 'application/json' }, body: JSON.stringify(body) })
    expect((await write('/api/dashboard/tokens', 'POST', { name: 17 })).status).toBe(400)
    const created = await write('/api/dashboard/tokens', 'POST', { name: 'Synthetic token' })
    expect(created.status).toBe(201)
    const token = await created.json()
    expect((await write(`/api/dashboard/tokens/${token.id}`, 'PATCH', { name: '   ' })).status).toBe(400)
    const renamed = await write(`/api/dashboard/tokens/${token.id}`, 'PATCH', { name: 'Renamed' })
    expect(renamed.status).toBe(200)
    expect((await renamed.json()).token).not.toBe(token.token)
    const listed = await (await app.request('/api/dashboard/tokens')).json()
    expect(listed.find((t: { id: string }) => t.id === token.id).name).toBe('Renamed')
    expect(JSON.stringify(listed)).not.toContain(token.token)
    expect(
      (await write(`/api/dashboard/tokens/${token.id}/streaming-settings`, 'PUT', { enabled: 'yes' })).status,
    ).toBe(400)
    expect(
      (await write(`/api/dashboard/tokens/${token.id}/streaming-settings`, 'PUT', { enabled: false })).status,
    ).toBe(200)
    // Sync has a bounded raw-body reader; schema validation must preserve it.
    const synced = await app.request(`/api/sync/${tokenHash(token.token)}/user`, {
      method: 'POST',
      headers: { 'content-type': 'application/json', authorization: `Bearer ${token.token}` },
      body: JSON.stringify({ skills: { 'synthetic/SKILL.md': '# Synthetic' } }),
    })
    expect(synced.status).toBe(200)
    expect((await app.request(`/api/dashboard/tokens/${token.id}`, { method: 'DELETE' })).status).toBe(200)
  })

  it('preserves administrator token reads and returns the same representation on rename', async () => {
    fixture.authEnabled = true
    try {
      const app = createApp()
      const created = await app.request('/api/dashboard/tokens', {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify({ name: 'Admin copy regression' }),
      })
      const token = await created.json()
      for (const authorization of [
        undefined,
        'Bearer invalid-session',
        `Bearer ${token.token}`,
        'Bearer synthetic-admin',
      ]) {
        const headers = authorization ? { authorization } : undefined
        const listed = await (await app.request('/api/dashboard/tokens', { headers })).json()
        const entry = listed.find((t: { id: string }) => t.id === token.id)
        if (authorization === 'Bearer synthetic-admin') expect(entry.token).toBe(token.token)
        else expect(entry.token).not.toBe(token.token)
        const renamed = await app.request(`/api/dashboard/tokens/${token.id}`, {
          method: 'PATCH',
          headers: { ...headers, 'content-type': 'application/json' },
          body: JSON.stringify({ name: 'Admin copy regression renamed' }),
        })
        expect(renamed.status).toBe(200)
        expect((await renamed.json()).token).toBe(entry.token)
      }
      expect((await app.request(`/api/dashboard/tokens/${token.id}`, { method: 'DELETE' })).status).toBe(200)
    } finally {
      fixture.authEnabled = false
    }
  })

  it('protects documentation with administrator Basic or bearer credentials', async () => {
    fixture.authEnabled = true
    try {
      const app = createApp()
      const denied = await app.request('/api/docs')
      expect(denied.status).toBe(401)
      expect(denied.headers.get('www-authenticate')).toContain('Basic')
      for (const authorization of [
        'Basic ' + Buffer.from('synthetic:test-only').toString('base64'),
        'Bearer synthetic-admin',
      ]) {
        expect((await app.request('/api/openapi.json', { headers: { authorization } })).status).toBe(200)
        expect((await app.request('/api/dashboard/config', { headers: { authorization } })).status).toBe(200)
      }
      await db.run(
        "INSERT INTO api_tokens (id,name,plugin_id,token,created_at,cache) VALUES ('scoped','Scoped','user','synthetic-scoped','2026-01-01',1)",
      )
      await resolveToken('synthetic-scoped')
      expect(
        (await app.request('/api/openapi.json', { headers: { authorization: 'Bearer synthetic-scoped' } })).status,
      ).toBe(403)
    } finally {
      fixture.authEnabled = false
    }
  })

  it('rejects malformed JSON responses using the published schema', async () => {
    const app = new Hono()
    app.use('*', contractMiddleware)
    app.get('/api/dashboard/config', (c) => c.json({ port: 'wrong' }))
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => {})
    try {
      expect((await app.request('/api/dashboard/config')).status).toBe(500)
      expect(warn).toHaveBeenCalled()
    } finally {
      warn.mockRestore()
    }
  })

  // Opt-in acceptance against the real local Scalar bundle, using the same
  // isolated database/account fixture as the handler tests. No CLI is launched.
  it.skipIf(!process.env.BRIDGE_API_BROWSER_EVIDENCE)(
    'renders Scalar offline in Chromium',
    async () => {
      const { serve } = await import('@hono/node-server')
      const { default: puppeteer } = await import('puppeteer-core')
      const evidence = process.env.BRIDGE_API_BROWSER_EVIDENCE!
      const server = serve({ fetch: createApp().fetch, hostname: '127.0.0.1', port: 0 })
      await new Promise<void>((resolve) => server.once('listening', resolve))
      const address = server.address() as { port: number }
      const origin = `http://127.0.0.1:${address.port}`
      const browser = await puppeteer.launch({
        executablePath: process.env.BRIDGE_API_BROWSER_EXECUTABLE,
        headless: true,
        userDataDir: path.join(fixture.dataDir, 'browser-profile'),
      })
      try {
        const page = await browser.newPage()
        await page.setViewport({ width: 1440, height: 1000 })
        const external: string[] = []
        const errors: string[] = []
        page.on('request', (request) => {
          if (/^https?:/.test(request.url()) && !request.url().startsWith(origin + '/')) external.push(request.url())
        })
        page.on('pageerror', (error) => errors.push(String(error)))
        fixture.authEnabled = true
        await page.authenticate({ username: 'synthetic', password: 'test-only' })
        await page.goto(origin + '/api/docs', { waitUntil: 'networkidle0' })
        await page.screenshot({ path: path.join(evidence, 'api-docs.png'), fullPage: false })
        fs.writeFileSync(
          path.join(evidence, 'browser-initial.json'),
          JSON.stringify({ body: await page.evaluate('document.body.innerText'), external, errors }, null, 2),
        )
        await page.waitForFunction(
          "document.body.innerText.includes('OpenAPI 3.1.0') && document.body.innerText.includes('Dashboard')",
        )
        const result = (await page.evaluate(`(async () => {
        const spec = await (await fetch('/api/openapi.json')).json()
        const configResponse = await fetch('/api/dashboard/config')
        const config = await configResponse.json()
        return { title: document.title, openapi: spec.openapi, operations: Object.values(spec.paths).reduce((count, path) => count + Object.keys(path).length, 0), configStatus: configResponse.status, configPort: config.port, buttons: Array.from(document.querySelectorAll('button')).map(button => button.innerText).filter(Boolean).slice(0, 30) }
      })()`)) as { title: string; openapi: string; operations: number; configStatus: number; configPort: number }
        expect(result.openapi).toBe('3.1.0')
        expect(result.configStatus).toBe(200)
        expect(typeof result.configPort).toBe('number')
        expect(external).toEqual([])
        expect(errors).toEqual([])
        await page.screenshot({ path: path.join(evidence, 'api-docs.png'), fullPage: false })
        fs.writeFileSync(path.join(evidence, 'browser.json'), JSON.stringify({ ...result, external, errors }, null, 2))
      } finally {
        fixture.authEnabled = false
        await browser.close()
        await new Promise<void>((resolve, reject) => server.close((error) => (error ? reject(error) : resolve())))
      }
    },
    90000,
  )
})

import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { Hono } from 'hono'
import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { createHash } from 'node:crypto'
const h = vi.hoisted(() => ({ file: '', owner: 'owner', rows: vi.fn() }))
vi.mock('../../stats/database/lifecycle', () => ({
  getDb: async () => ({
    runAndReadAll: async (sql: string) => ({
      getRowObjects: () => (sql.includes('SELECT user_name') ? [{ user_name: h.owner }] : [{ file_path: h.file }]),
    }),
  }),
}))
vi.mock('../../telemetry/store', () => ({
  getTokenUsageSummary: vi.fn(),
  getCostSummary: vi.fn(),
  getMetricsSummary: vi.fn(),
  queryEvents: vi.fn(),
  getEventById: vi.fn(),
  deleteEventById: vi.fn(),
  clearOtelData: vi.fn(),
  clearOtelDataForUser: vi.fn(),
}))
vi.mock('../../stats/database/aggregates', () => ({ getUserTimeSeries: vi.fn() }))
vi.mock('../../stats/database/crud', () => ({ clearAll: vi.fn(), deleteRequestsByFilter: vi.fn() }))
vi.mock('../../pty/session-manager', () => ({ getAllSessions: () => [] }))
vi.mock('../../auth/dashboard-auth', () => ({
  isDashboardAuthEnabled: () => true,
  validateDashboardSession: async () => null,
}))
vi.mock('../../auth/tokens', () => ({
  resolveTokenSync: (token: string) =>
    token === 'synthetic-own'
      ? { userName: 'owner', tokenId: 'token' }
      : token === 'synthetic-foreign'
        ? { userName: 'other', tokenId: 'other-token' }
        : null,
}))
import { dashboardAuthMiddleware } from '../../middleware/dashboard-auth'
import { registerStatsRoutes } from './stats'
let root: string
let app: Hono
beforeEach(() => {
  root = fs.mkdtempSync(path.join(os.tmpdir(), 'jsonl-export-test-'))
  h.file = path.join(root, 'session.jsonl')
  h.owner = 'owner'
  fs.writeFileSync(h.file, Buffer.alloc(2 * 1024 * 1024, 97))
  app = new Hono()
  app.use('/api/dashboard/*', dashboardAuthMiddleware)
  registerStatsRoutes(app)
})
afterEach(() => {
  vi.useRealTimers()
  if (
    path.dirname(path.resolve(root)) !== path.resolve(os.tmpdir()) ||
    !path.basename(root).startsWith('jsonl-export-test-')
  )
    throw new Error('unexpected fixture root')
  fs.rmSync(root, { recursive: true, force: true })
})
const auth = { Authorization: 'Bearer synthetic-own' }
describe('INC-2026-0018 authorized HTTP export', () => {
  it('starts a bounded byte stream instead of a whole-file string', async () => {
    const response = await app.request('/api/dashboard/sessions/session/jsonl', { headers: auth })
    expect(response.status).toBe(200)
    const reader = response.body!.getReader()
    const first = await reader.read()
    expect(first.value!.byteLength).toBeLessThanOrEqual(64 * 1024)
    await reader.cancel()
  })
  it('preserves raw bytes and excludes appends after the opened size snapshot', async () => {
    const data = Buffer.from([0xff, 0, 0xc3, 0xa9, 0x0a])
    fs.writeFileSync(h.file, data)
    const response = await app.request('/api/dashboard/sessions/session/jsonl', { headers: auth })
    fs.appendFileSync(h.file, Buffer.alloc(5000, 98))
    expect(response.headers.get('content-length')).toBe(String(data.length))
    expect(Buffer.from(await response.arrayBuffer())).toEqual(data)
  })
  it('preserves auth/ownership and returns a deleted-file response', async () => {
    expect((await app.request('/api/dashboard/sessions/session/jsonl')).status).toBe(401)
    expect(
      (
        await app.request('/api/dashboard/sessions/session/jsonl', {
          headers: { Authorization: 'Bearer synthetic-foreign' },
        })
      ).status,
    ).toBe(403)
    fs.unlinkSync(h.file)
    expect((await app.request('/api/dashboard/sessions/session/jsonl', { headers: auth })).status).toBe(404)
  })
  it('bounds concurrent exports and releases capacity on reader cancellation', async () => {
    const a = await app.request('/api/dashboard/sessions/session/jsonl', { headers: auth })
    const b = await app.request('/api/dashboard/sessions/session/jsonl', { headers: auth })
    expect((await app.request('/api/dashboard/sessions/session/jsonl', { headers: auth })).status).toBe(429)
    await a.body!.cancel()
    await b.body!.cancel()
    const next = await app.request('/api/dashboard/sessions/session/jsonl', { headers: auth })
    expect(next.status).toBe(200)
    await next.body!.cancel()
  })
  it('grants only one exact attachment route via an HttpOnly cookie, never a URL Bearer', async () => {
    const issue = await app.request('/api/dashboard/sessions/session/jsonl-download', { method: 'POST', headers: auth })
    expect(issue.status).toBe(200)
    const { url } = await issue.json()
    expect(url).toBe('/api/dashboard/sessions/session/jsonl-download')
    const cookie = issue.headers.get('set-cookie')!
    expect(cookie).toContain('HttpOnly')
    expect(cookie).toContain('SameSite=Strict')
    expect(cookie).toContain('Max-Age=30')
    const headers = { Cookie: cookie.split(';')[0]! }
    expect((await app.request('/api/dashboard/sessions/other/jsonl-download', { headers })).status).toBe(401)
    expect((await app.request('/api/dashboard/sessions/session/jsonl', { headers })).status).toBe(401)
    const response = await app.request(url, { headers })
    expect(response.status).toBe(200)
    await response.body!.cancel()
    expect((await app.request(url, { headers })).status).toBe(401)
  })
  it('rechecks ownership after grant issuance and refuses a foreign grant request', async () => {
    expect(
      (
        await app.request('/api/dashboard/sessions/session/jsonl-download', {
          method: 'POST',
          headers: { Authorization: 'Bearer synthetic-foreign' },
        })
      ).status,
    ).toBe(403)
    const issue = await app.request('/api/dashboard/sessions/session/jsonl-download', { method: 'POST', headers: auth })
    const cookie = issue.headers.get('set-cookie')!.split(';')[0]!
    h.owner = 'changed'
    expect(
      (await app.request('/api/dashboard/sessions/session/jsonl-download', { headers: { Cookie: cookie } })).status,
    ).toBe(403)
  })
  it('expires and replaces unconsumed grants without granting arbitrary API access', async () => {
    const first = await app.request('/api/dashboard/sessions/session/jsonl-download', { method: 'POST', headers: auth })
    const old = first.headers.get('set-cookie')!.split(';')[0]!
    const second = await app.request('/api/dashboard/sessions/session/jsonl-download', {
      method: 'POST',
      headers: auth,
    })
    const cookie = second.headers.get('set-cookie')!.split(';')[0]!
    expect(
      (await app.request('/api/dashboard/sessions/session/jsonl-download', { headers: { Cookie: old } })).status,
    ).toBe(401)
    vi.useFakeTimers()
    vi.advanceTimersByTime(30_001)
    expect(
      (await app.request('/api/dashboard/sessions/session/jsonl-download', { headers: { Cookie: cookie } })).status,
    ).toBe(401)
    vi.useRealTimers()
  })
  it('closes and returns capacity when the HTTP request signal is aborted', async () => {
    const abort = new AbortController()
    const response = await app.request(
      new Request('http://localhost/api/dashboard/sessions/session/jsonl', { headers: auth, signal: abort.signal }),
    )
    const reader = response.body!.getReader()
    await reader.read()
    abort.abort()
    await expect(reader.read()).rejects.toThrow()
    const next = await app.request('/api/dashboard/sessions/session/jsonl', { headers: auth })
    expect(next.status).toBe(200)
    await next.body!.cancel()
  })
  it.skipIf(process.platform !== 'linux' || process.env.BRIDGE_HTTP_ACCEPTANCE !== '1')(
    'disposable Linux HTTP gate: slow/cancelled exports keep status responsive and preserve bytes',
    async () => {
      const { serve } = await import('@hono/node-server')
      const data = Buffer.alloc(64 * 1024 * 1024, 97)
      fs.writeFileSync(h.file, data)
      app.get('/fixture-status', (c) => c.json({ ok: true }))
      let ready!: (address: any) => void
      const listening = new Promise<any>((resolve) => {
        ready = resolve
      })
      const server = serve({ fetch: app.fetch, hostname: '127.0.0.1', port: 0 }, ready)
      const address = await listening
      const origin = `http://127.0.0.1:${address.port}`
      const exports: Response[] = []
      try {
        const a = await fetch(origin + '/api/dashboard/sessions/session/jsonl', { headers: auth })
        exports.push(a)
        const b = await fetch(origin + '/api/dashboard/sessions/session/jsonl', { headers: auth })
        exports.push(b)
        await new Promise((resolve) => setTimeout(resolve, 50))
        expect(await (await fetch(origin + '/fixture-status', { signal: AbortSignal.timeout(2000) })).json()).toEqual({
          ok: true,
        })
        expect((await fetch(origin + '/api/dashboard/sessions/session/jsonl', { headers: auth })).status).toBe(429)
        await a.body!.cancel()
        await b.body!.cancel()
        let next: Response | undefined
        for (let i = 0; i < 30; i++) {
          next = await fetch(origin + '/api/dashboard/sessions/session/jsonl', { headers: auth })
          if (next.status === 200) break
          await new Promise((resolve) => setTimeout(resolve, 10))
        }
        expect(next!.status).toBe(200)
        exports.push(next!)
        const hash = createHash('sha256')
        const reader = next!.body!.getReader()
        let count = 0
        for (;;) {
          const { value, done } = await reader.read()
          if (done) break
          count += value.byteLength
          hash.update(value)
        }
        expect(count).toBe(data.length)
        expect(hash.digest('hex')).toBe(createHash('sha256').update(data).digest('hex'))
      } finally {
        for (const response of exports) if (!response.bodyUsed) await response.body?.cancel()
        await new Promise<void>((resolve) => server.close(() => resolve()))
      }
    },
    15_000,
  )
  it.skipIf(process.env.BRIDGE_BROWSER_ACCEPTANCE !== '1')(
    'disposable browser gate: actual dashboard button saves the authorized attachment once',
    async () => {
      const { serve } = await import('@hono/node-server')
      const { build } = await import('esbuild')
      const { default: puppeteer } = await import('puppeteer-core')
      const uiRoot = path.resolve('src/ui')
      const bundle = await build({
        stdin: {
          contents:
            "import {render} from 'preact'; import {TokenSessionsList} from './src/components/dashboard/StatsCard'; render(<TokenSessionsList tokenId=\"token\"/>, document.getElementById('app')!);",
          resolveDir: uiRoot,
          sourcefile: 'fixture.tsx',
          loader: 'tsx',
        },
        bundle: true,
        write: false,
        format: 'iife',
        platform: 'browser',
        jsx: 'automatic',
        jsxImportSource: 'preact',
        loader: { '.css': 'empty' },
        outfile: 'fixture.js',
      })
      app.get('/fixture.js', (c) =>
        c.body(bundle.outputFiles![0]!.text, 200, { 'Content-Type': 'application/javascript' }),
      )
      app.get('/', (c) =>
        c.html(
          '<html><head><style>body{font-family:Arial,sans-serif;font-size:14px}.fa-download:before{content:"↓"}.fa-trash:before{content:"×"}</style></head><body><div id="app"></div><script>localStorage.setItem("dashboard_session_token","synthetic-own")</script><script src="/fixture.js"></script></body></html>',
        ),
      )
      // Register before the real token-list route, whose unrelated analytics query
      // is outside this export fixture. The export route/authorization stay real.
      const browserApp = new Hono()
      browserApp.get('/api/dashboard/tokens/token/sessions', (c) =>
        c.json({
          sessions: [
            {
              sessionId: 'session',
              title: 'Synthetic transcript',
              folder: 'fixture',
              model: 'test',
              startedAt: '2026-01-01T00:00:00Z',
              lastActivityAt: '2026-01-01T00:00:00Z',
              userMessages: 1,
              assistantMessages: 1,
              compactCount: 0,
              inTokens: 1,
              cacheReadTokens: 0,
              cacheWriteTokens: 0,
              outTokens: 1,
              contextTokens: 1,
              jsonlAvailable: true,
            },
          ],
        }),
      )
      browserApp.route('/', app)
      let ready!: (address: any) => void
      const listening = new Promise<any>((resolve) => {
        ready = resolve
      })
      const server = serve({ fetch: browserApp.fetch, hostname: '127.0.0.1', port: 0 }, ready)
      const address = await listening
      const downloads = path.join(root, 'downloads')
      fs.mkdirSync(downloads)
      const browser = await puppeteer.launch({
        executablePath: process.env.BRIDGE_BROWSER_EXECUTABLE,
        headless: true,
        userDataDir: path.join(root, 'browser-profile'),
      })
      try {
        const cdp = await browser.target().createCDPSession()
        await cdp.send('Browser.setDownloadBehavior', {
          behavior: 'allow',
          downloadPath: downloads,
          eventsEnabled: true,
        })
        const completed = new Promise<void>((resolve) =>
          cdp.on('Browser.downloadProgress', (event) => {
            if (event.state === 'completed') resolve()
          }),
        )
        const page = await browser.newPage()
        await page.setViewport({ width: 1600, height: 900 })
        await page.goto(`http://127.0.0.1:${address.port}/`)
        await page.waitForSelector('button[title="Download JSONL"]')
        const artifacts = process.env.BRIDGE_EXPORT_GATE_ARTIFACTS
        if (artifacts) {
          fs.mkdirSync(artifacts, { recursive: true })
          await page.screenshot({ path: path.join(artifacts, 'INC-2026-0018-before.png') })
        }
        const button = await page.$('button[title="Download JSONL"]')
        await button!.click()
        await button!.click() // disabled button cannot mint a second grant
        await completed
        expect(
          await page.$eval(
            'button[title^="Download requested"]',
            (node) => (node as unknown as { disabled: boolean }).disabled,
          ),
        ).toBe(true)
        expect(fs.readFileSync(path.join(downloads, 'session.jsonl'))).toEqual(fs.readFileSync(h.file))
        expect(fs.readdirSync(downloads)).toEqual(['session.jsonl'])
        page.once('dialog', (dialog) => {
          void dialog.dismiss()
        })
        await page.click('button[title="Soft-delete"]')
        expect(await page.$eval('tbody tr', (node) => node.textContent)).toContain('Synthetic transcript')
        if (artifacts) await page.screenshot({ path: path.join(artifacts, 'INC-2026-0018-after.png') })
      } finally {
        await browser.close()
        await new Promise<void>((resolve) => server.close(() => resolve()))
      }
    },
    30_000,
  )
})

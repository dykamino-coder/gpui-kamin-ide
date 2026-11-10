import assert from 'node:assert/strict'
import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { createServer, get } from 'node:http'
import { test } from 'node:test'
import { CREDENTIAL_FILE, SESSION_FILE, loadCredentials, resetCredentials } from '../src/credentials.js'
import { WorkspaceAuth } from '../src/auth.js'
import { WorkspaceSessionState } from '../src/session-state.js'
import { startWorkspaceServer } from '../src/server.js'

test('browser auth and open tabs persist independently across restart and credential reset', async t => {
  const storage = fs.mkdtempSync(path.join(os.tmpdir(), 'bridgeweb-credentials-'))
  t.after(() => fs.rmSync(storage, { recursive: true }))
  const credentials = await loadCredentials(storage, 'http://127.0.0.1:1', { BRIDGE_TOKEN: 'synthetic-token' })
  const persistent = { secret: credentials.browserSecret, file: path.join(storage, SESSION_FILE) }
  const auth = () => new WorkspaceAuth(credentials.token, async () => ({ id: 'identity' }), 60_000, Date.now, persistent)
  const cookie = await auth().loginLocal()
  assert.ok(cookie)
  const originalAuth = fs.readFileSync(persistent.file, 'utf8')
  const store = () => new WorkspaceSessionState(storage, '/workspace', 'ws://127.0.0.1:1', credentials.token)
  const tabs = [{ id: 'open-tab', cwd: '/workspace', label: '#1', createdAt: new Date().toISOString() }]
  store().save(tabs, 'open-tab')
  assert.equal(await auth().validate(cookie), true)
  assert.equal(fs.readFileSync(persistent.file, 'utf8'), originalAuth)
  assert.deepEqual(store().load(), { active: 'open-tab', tabs })
  auth().logout(cookie)
  assert.equal(await auth().validate(cookie), false)
  assert.deepEqual(store().load(), { active: 'open-tab', tabs })
  resetCredentials(storage)
  assert.equal(fs.existsSync(persistent.file), false)
  assert.deepEqual(store().load(), { active: 'open-tab', tabs })
})

test('service registers once, imports once, persists private secrets and resets explicitly', async t => {
  const storage = fs.mkdtempSync(path.join(os.tmpdir(), 'bridgeweb-credentials-'))
  t.after(() => fs.rmSync(storage, { recursive: true }))
  let requests = 0
  const upstream = createServer(async (request, response) => {
    assert.equal(request.url, '/api/dashboard/tokens')
    let body = ''
    for await (const chunk of request) body += chunk
    assert.match(JSON.parse(body).name, /^workspace-[a-f0-9-]+$/)
    requests++
    response.writeHead(201, { 'Content-Type': 'application/json' })
    response.end(JSON.stringify({ token: 'synthetic-registered-token' }))
  })
  await new Promise<void>(resolve => upstream.listen(0, '127.0.0.1', resolve))
  t.after(() => new Promise<void>(resolve => upstream.close(() => resolve())))
  const origin = `http://127.0.0.1:${(upstream.address() as { port: number }).port}`
  const first = await loadCredentials(storage, origin, {})
  assert.equal(first.token, 'synthetic-registered-token')
  assert.equal(requests, 1)
  assert.deepEqual(await loadCredentials(storage, origin, { BRIDGE_TOKEN_FILE: '/must-not-read' }), first)
  assert.equal(requests, 1)
  await assert.rejects(loadCredentials(storage, 'http://127.0.0.1:1', {}), /changed server/)
  if (process.platform !== 'win32') {
    assert.equal(fs.statSync(path.join(storage, CREDENTIAL_FILE)).mode & 0o777, 0o600)
    assert.equal(fs.statSync(storage).mode & 0o777, 0o700)
  }
  fs.writeFileSync(path.join(storage, SESSION_FILE), '[]', { mode: 0o600 })
  resetCredentials(storage)
  assert.equal(fs.existsSync(path.join(storage, SESSION_FILE)), false)
  const imported = await loadCredentials(storage, origin, { BRIDGE_TOKEN: 'synthetic-imported-token' })
  assert.equal(imported.token, 'synthetic-imported-token')
  assert.notEqual(imported.browserSecret, first.browserSecret)
  assert.equal(requests, 1)
  resetCredentials(storage)
  fs.writeFileSync(path.join(storage, CREDENTIAL_FILE), '{}')
  await assert.rejects(loadCredentials(storage, origin, {}), /Invalid credentials/)
  assert.equal(requests, 1) // Corrupt state never silently rotates credentials.
})

test('restricted registration needs a bootstrap file once; refusal never creates credentials', async t => {
  const storage = fs.mkdtempSync(path.join(os.tmpdir(), 'bridgeweb-credentials-'))
  t.after(() => fs.rmSync(storage, { recursive: true }))
  const upstream = createServer((_request, response) => { response.writeHead(403); response.end() })
  await new Promise<void>(resolve => upstream.listen(0, '127.0.0.1', resolve))
  t.after(() => new Promise<void>(resolve => upstream.close(() => resolve())))
  const origin = `http://127.0.0.1:${(upstream.address() as { port: number }).port}`
  await assert.rejects(loadCredentials(storage, origin, {}), /provide a bootstrap secret/)
  assert.equal(fs.existsSync(path.join(storage, CREDENTIAL_FILE)), false)
  const bootstrap = path.join(storage, 'synthetic-bootstrap')
  fs.writeFileSync(bootstrap, 'synthetic-file-token\n', { mode: 0o600 })
  const first = await loadCredentials(storage, origin, { BRIDGE_TOKEN_FILE: bootstrap })
  fs.unlinkSync(bootstrap)
  assert.equal(first.token, 'synthetic-file-token')
  assert.deepEqual(await loadCredentials(storage, origin, { BRIDGE_TOKEN_FILE: bootstrap }), first)
})

test('loopback first visit, restart cookie, persisted logout, foreign Origin/Host and remote opt-in', async t => {
  const storage = fs.mkdtempSync(path.join(os.tmpdir(), 'bridgeweb-credentials-'))
  t.after(() => fs.rmSync(storage, { recursive: true }))
  const credentials = await loadCredentials(storage, 'http://127.0.0.1:1', { BRIDGE_TOKEN: 'synthetic-token' })
  const persistent = { secret: credentials.browserSecret, file: path.join(storage, SESSION_FILE) }
  const auth = () => new WorkspaceAuth(credentials.token, async () => ({ id: 'identity' }), 60_000, Date.now, persistent)
  const backend = { attach() { throw new Error('Unused') } }
  let server = await startWorkspaceServer({ auth: auth(), backend, port: 0, autoLogin: true })
  t.after(() => server.close())
  const health = await fetch(`${server.origin}/health`)
  assert.deepEqual(await health.json(), { ready: true })
  assert.equal(health.headers.has('set-cookie'), false)
  assert.equal(fs.existsSync(persistent.file), false)
  let response = await fetch(`${server.origin}/auth/session`)
  assert.deepEqual(await response.json(), { authenticated: true })
  const cookie = response.headers.get('set-cookie')!.split(';')[0]
  assert.match(response.headers.get('set-cookie')!, /HttpOnly; SameSite=Strict/)
  assert.equal(fs.readFileSync(persistent.file, 'utf8').includes(cookie.split('=')[1]), false)
  await server.close()
  server = await startWorkspaceServer({ auth: auth(), backend, port: 0, autoLogin: true })
  response = await fetch(`${server.origin}/auth/session`, { headers: { Cookie: cookie } })
  assert.deepEqual(await response.json(), { authenticated: true })
  assert.equal(response.headers.has('set-cookie'), false) // The old session itself survived.
  for (const headers of [{ Origin: 'http://foreign.test' }, { Host: 'foreign.test' }, { 'Sec-Fetch-Site': 'cross-site' }]) {
    await new Promise<void>((resolve, reject) => {
      get(`${server.origin}/auth/session`, { headers }, rejected => {
        rejected.resume()
        try {
          assert.equal(rejected.statusCode, 403)
          assert.equal(rejected.headers['set-cookie'], undefined)
          resolve()
        } catch (error) { reject(error) }
      }).on('error', reject)
    })
  }
  await fetch(`${server.origin}/auth/logout`, { method: 'POST', headers: { Origin: server.origin, Cookie: cookie } })
  assert.equal(await auth().validate(cookie.split('=')[1]), false)
  assert.equal(await new WorkspaceAuth(credentials.token, async () => ({ id: 'identity' }), 60_000, Date.now,
    { ...persistent, secret: 'rotated-secret' }).validate(cookie.split('=')[1]), false)
  await assert.rejects(startWorkspaceServer({ auth: auth(), backend, origin: 'http://remote.test', autoLogin: true }), /loopback/)
  await server.close()
  server = await startWorkspaceServer({ auth: auth(), backend, port: 0, autoLogin: false })
  assert.deepEqual(await (await fetch(`${server.origin}/auth/session`)).json(), { authenticated: false })
})

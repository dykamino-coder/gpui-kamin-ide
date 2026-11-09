import assert from 'node:assert/strict'
import { createServer } from 'node:http'
import type { AddressInfo } from 'node:net'
import { test } from 'node:test'
import { WorkspaceAuth, readSessionCookie, serverTokenResolver } from '../src/auth.js'

test('token login pins identity, rejects other tokens, expires and fails closed on revocation', async () => {
  let now = 0
  let id: string | null = 'workspace-a'
  let resolutions = 0
  const auth = new WorkspaceAuth('test-token', async () => { resolutions++; return id ? { id } : null }, 1000, () => now)
  assert.equal(await auth.login('different-valid-token'), null)
  assert.equal(resolutions, 0)
  const cookie = await auth.login('test-token')
  assert.ok(cookie)
  assert.equal(cookie.includes('test-token'), false)
  assert.equal(await auth.validate(cookie), true)
  assert.match(auth.cookieHeader(cookie, true), /HttpOnly; SameSite=Strict; Max-Age=1; Secure$/)
  now = 1000
  assert.equal(await auth.validate(cookie), false)
  const second = await auth.login('test-token')
  assert.ok(second)
  id = null
  assert.equal(await auth.validate(second), false)
  id = 'workspace-b'
  assert.equal(await auth.login('test-token'), null)
})

test('logout during upstream validation cannot resurrect a session', async () => {
  let release: (() => void) | undefined
  let wait = false
  const auth = new WorkspaceAuth('test-token', async () => {
    if (wait) await new Promise<void>(resolve => { release = resolve })
    return { id: 'a' }
  })
  const cookie = await auth.login('test-token')
  assert.ok(cookie)
  wait = true
  const validation = auth.validate(cookie)
  auth.logout(cookie)
  release!()
  assert.equal(await validation, false)
})

test('upstream resolver exceptions fail closed without exposing error details', async () => {
  let fail = false
  const auth = new WorkspaceAuth('test-token', async () => {
    if (fail) throw new Error('Private upstream detail')
    return { id: 'a' }
  })
  const cookie = await auth.login('test-token')
  assert.ok(cookie)
  fail = true
  assert.equal(await auth.login('test-token'), null)
  assert.equal(await auth.validate(cookie), false)
})

test('bounded sessions and strict duplicate-cookie handling', async () => {
  const auth = new WorkspaceAuth('test-token', async () => ({ id: 'a' }))
  const cookie = await auth.login('test-token')
  assert.ok(cookie)
  assert.equal(readSessionCookie(`other=x; kamin_workspace=${cookie}`), cookie)
  assert.equal(readSessionCookie(`kamin_workspace=${cookie}; kamin_workspace=${cookie}`), undefined)
  assert.equal(readSessionCookie('kamin_workspace=../../token'), undefined)
  for (let i = 1; i < 128; i++) assert.ok(await auth.login('test-token'))
  assert.equal(await auth.login('test-token'), null)
  auth.logout(cookie)
  assert.ok(await auth.login('test-token'))
  assert.throws(() => new WorkspaceAuth('', async () => null), /required/)
})

test('real resolver uses existing token endpoint and rejects auth-disabled/redirect responses', async t => {
  let mode = 'valid'
  const upstream = createServer(async (request, response) => {
    assert.equal(request.url, '/api/dashboard/tokens/resolve')
    assert.equal(request.method, 'POST')
    let body = ''
    for await (const chunk of request) body += chunk
    assert.deepEqual(JSON.parse(body), { token: 'fixture-token' })
    if (mode === 'redirect') { response.writeHead(302, { Location: '/other' }); response.end(); return }
    response.setHeader('Content-Type', 'application/json')
    response.end(JSON.stringify(mode === 'valid' ? { id: 'identity-a', name: 'test' } : { authenticated: true, authDisabled: true }))
  })
  await new Promise<void>(resolve => upstream.listen(0, '127.0.0.1', resolve))
  t.after(() => new Promise<void>(resolve => upstream.close(() => resolve())))
  const resolve = serverTokenResolver(`ws://127.0.0.1:${(upstream.address() as AddressInfo).port}/?ignored=yes`)
  assert.deepEqual(await resolve('fixture-token'), { id: 'identity-a' })
  mode = 'disabled'
  assert.equal(await resolve('fixture-token'), null)
  mode = 'redirect'
  assert.equal(await resolve('fixture-token'), null)
  assert.throws(() => serverTokenResolver('file:///tmp/a'), /Invalid/)
  assert.throws(() => serverTokenResolver('https://user:secret@example.test'), /Invalid/)
})

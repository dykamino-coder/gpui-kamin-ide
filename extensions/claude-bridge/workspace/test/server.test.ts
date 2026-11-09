import assert from 'node:assert/strict'
import { EventEmitter, once } from 'node:events'
import { get } from 'node:http'
import { test, type TestContext } from 'node:test'
import { WebSocket } from 'ws'
import { WorkspaceAuth } from '../src/auth.js'
import { startWorkspaceServer } from '../src/server.js'
import type { ClientView, WorkspaceBackend } from '../src/protocol.js'

async function fixture(t: TestContext, revalidateMs = 15_000) {
  let valid = true
  let attached = 0
  let disposed = 0
  const lifecycle = new EventEmitter()
  const views = new Set<ClientView>()
  const auth = new WorkspaceAuth('fixture-token', async () => valid ? { id: 'fixture-identity' } : null)
  // Deliberate transport-only peer: this is not a model or shared-handler acceptance.
  const backend: WorkspaceBackend = {
    attach(view) {
      attached++
      views.add(view)
      return {
        async receive(frame) {
          if (frame.kind === 'invoke') await view.postMessage({
            kind: 'invoke-reply', id: frame.id, generation: frame.generation,
            ok: frame.channel === 'fixture:echo',
            ...(frame.channel === 'fixture:echo' ? { result: frame.args } : { error: 'Unsupported channel' }),
          })
          else if (frame.kind === 'send') for (const sink of views) await sink.postMessage({ kind: 'event', channel: frame.channel, args: frame.args })
        },
        dispose() { disposed++; views.delete(view); lifecycle.emit('dispose') },
      }
    },
  }
  const server = await startWorkspaceServer({ auth, backend, port: 0, revalidateMs })
  t.after(() => server.close())
  const login = async (origin = server.origin, token = 'fixture-token') => fetch(`${server.origin}/auth/login`, {
    method: 'POST', headers: { Origin: origin, 'Content-Type': 'application/json' }, body: JSON.stringify({ token }),
  })
  const connect = (cookie: string, origin = server.origin, path = '/bridge') => new WebSocket(
    server.origin.replace('http:', 'ws:') + path, { headers: { Origin: origin, Cookie: cookie } },
  )
  const waitDisposed = (count: number) => new Promise<void>(resolve => {
    if (disposed >= count) { resolve(); return }
    const listener = () => {
      if (disposed >= count) { lifecycle.off('dispose', listener); resolve() }
    }
    lifecycle.on('dispose', listener)
  })
  return { server, login, connect, waitDisposed, revoke: () => { valid = false }, counts: () => ({ attached, disposed }) }
}

test('loopback default, same-origin login, opaque session and strict Host', async t => {
  const f = await fixture(t)
  assert.equal(f.server.address.address, '127.0.0.1')
  assert.equal((await f.login('https://attacker.test')).status, 403)
  assert.equal((await f.login(f.server.origin, 'other-token')).status, 401)
  const response = await f.login()
  assert.equal(response.status, 200)
  const cookie = response.headers.get('set-cookie')!
  assert.match(cookie, /HttpOnly; SameSite=Strict/)
  assert.equal(cookie.includes('fixture-token'), false)
  const cookiePair = cookie.split(';')[0]
  assert.deepEqual(await (await fetch(`${f.server.origin}/auth/session`, { headers: { Cookie: cookiePair } })).json(), { authenticated: true })
  assert.deepEqual(await (await fetch(`${f.server.origin}/auth/session`)).json(), { authenticated: false })
  const wrongHost = await new Promise<number | undefined>((resolve, reject) => {
    get(`${f.server.origin}/auth/session`, { headers: { Host: 'attacker.test' } }, response => {
      response.resume(); resolve(response.statusCode)
    }).on('error', reject)
  })
  assert.equal(wrongHost, 403)
  assert.equal((await fetch(`${f.server.origin}/`)).status, 503)
  assert.equal((await fetch(`${f.server.origin}/bridge?token=fixture-token`)).status, 404)
})

test('upgrade rejects missing auth, cross origin, and token-in-URL before attaching', async t => {
  const f = await fixture(t)
  const cookie = (await f.login()).headers.get('set-cookie')!.split(';')[0]
  for (const ws of [f.connect(''), f.connect(cookie, 'https://attacker.test'), f.connect(cookie, f.server.origin, '/bridge?token=fixture-token')]) {
    await new Promise<void>(resolve => ws.once('error', () => resolve()))
    assert.notEqual(ws.readyState, WebSocket.OPEN)
  }
  assert.equal(f.counts().attached, 0)
})

test('replies stay with requester; events fan out; reload identity is preserved', async t => {
  const f = await fixture(t)
  const cookie = (await f.login()).headers.get('set-cookie')!.split(';')[0]
  const first = f.connect(cookie)
  const second = f.connect(cookie)
  await Promise.all([once(first, 'open'), once(second, 'open')])
  let secondMessages = 0
  second.on('message', () => { secondMessages++ })
  const reply = once(first, 'message')
  first.send(JSON.stringify({ kind: 'invoke', id: 7, generation: 'document-a', channel: 'fixture:echo', args: ['text'] }))
  assert.deepEqual(JSON.parse(String((await reply)[0])), { kind: 'invoke-reply', id: 7, generation: 'document-a', ok: true, result: ['text'] })
  assert.equal(secondMessages, 0)
  const events = [once(first, 'message'), once(second, 'message')]
  first.send(JSON.stringify({ kind: 'send', channel: 'fixture:event', args: [42] }))
  for (const event of await Promise.all(events)) assert.deepEqual(JSON.parse(String(event[0])), { kind: 'event', channel: 'fixture:event', args: [42] })
  const unknown = once(first, 'message')
  first.send(JSON.stringify({ kind: 'invoke', id: 8, generation: 'document-a', channel: 'unknown', args: [] }))
  assert.equal(JSON.parse(String((await unknown)[0])).ok, false)
  const disposed = f.waitDisposed(2)
  const closed = [once(first, 'close'), once(second, 'close'), disposed]
  first.close(); second.close()
  await Promise.all(closed)
  assert.deepEqual(f.counts(), { attached: 2, disposed: 2 })
})

test('logout closes existing sockets and prevents another upgrade', async t => {
  const f = await fixture(t)
  const cookie = (await f.login()).headers.get('set-cookie')!.split(';')[0]
  const socket = f.connect(cookie)
  await once(socket, 'open')
  const closed = once(socket, 'close')
  assert.equal((await fetch(`${f.server.origin}/auth/logout`, { method: 'POST', headers: { Origin: f.server.origin, Cookie: cookie } })).status, 200)
  assert.equal((await closed)[0], 4001)
  const refused = f.connect(cookie)
  await new Promise<void>(resolve => refused.once('error', () => resolve()))
  await f.waitDisposed(1)
  assert.deepEqual(f.counts(), { attached: 1, disposed: 1 })
})

test('upstream revocation closes a connected socket without waiting for browser reload', async t => {
  const f = await fixture(t, 10)
  const cookie = (await f.login()).headers.get('set-cookie')!.split(';')[0]
  const socket = f.connect(cookie)
  await once(socket, 'open')
  const closed = once(socket, 'close')
  f.revoke()
  assert.equal((await closed)[0], 4001)
  await f.waitDisposed(1)
})

test('malformed/forged and binary frames close the connection and release its view', async t => {
  const f = await fixture(t)
  const cookie = (await f.login()).headers.get('set-cookie')!.split(';')[0]
  for (const data of ['{', JSON.stringify({ kind: 'event', channel: 'jsonl-entries', args: [] }), Buffer.from('binary')]) {
    const socket = f.connect(cookie)
    await once(socket, 'open')
    const closed = once(socket, 'close')
    socket.send(data)
    assert.ok([4002, 4008].includes((await closed)[0]))
  }
  await f.waitDisposed(3)
  assert.deepEqual(f.counts(), { attached: 3, disposed: 3 })
})

test('login request size and repeated failures are bounded', async t => {
  const f = await fixture(t)
  assert.equal((await f.login(f.server.origin, 'x'.repeat(9000))).status, 400)
  for (let i = 0; i < 9; i++) assert.equal((await f.login(f.server.origin, 'wrong')).status, 401)
  assert.equal((await f.login()).status, 429)
})

test('remote binding requires an explicit canonical origin', async () => {
  const auth = new WorkspaceAuth('token', async () => null)
  const backend: WorkspaceBackend = { attach() { throw new Error('Unused') } }
  await assert.rejects(startWorkspaceServer({ auth, backend, bind: '0.0.0.0' }), /explicit browser origin/)
  await assert.rejects(startWorkspaceServer({ auth, backend, origin: 'http://localhost:3460/path' }), /Invalid browser origin/)
})

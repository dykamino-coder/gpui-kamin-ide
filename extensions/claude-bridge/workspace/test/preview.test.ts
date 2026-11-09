import assert from 'node:assert/strict'
import { once } from 'node:events'
import { createServer } from 'node:http'
import { test } from 'node:test'
import { WebSocket, WebSocketServer } from 'ws'
import { WorkspacePreview } from '../src/preview.js'
import { WorkspaceAuth } from '../src/auth.js'
import { startWorkspaceServer } from '../src/server.js'

test('preview validates local entry and JSON contract and retains a detached descriptor', async () => {
  const preview = new WorkspacePreview()
  preview.blockPorts([3460, 3462])
  const input = { name: 'fixture', entry: 'http://localhost:5173/remoteEntry.js', exposedModule: './Panel', props: { count: 1 } }
  for (const entry of ['https://localhost:5173/remoteEntry.js', 'http://example.com:5173/x', 'http://localhost:3460/x',
    'http://localhost:3462/x', 'http://user:secret@localhost:5173/x', 'http://localhost:5173/x?token=secret']) {
    assert.throws(() => preview.mount({ ...input, entry }))
  }
  assert.throws(() => preview.mount({ ...input, name: '../bad' }))
  assert.throws(() => preview.mount({ ...input, exposedModule: './../bad' }))
  assert.throws(() => preview.mount({ ...input, props: [] }))
  assert.throws(() => preview.mount({ ...input, props: { long: 'a'.repeat(65536) } }))
  const first = preview.mount(input)
  assert.equal(first.entry, '/preview/remotes/fixture/remoteEntry.js')
  first.props.count = 99
  assert.equal(preview.copy()!.props.count, 1)
  assert.deepEqual(preview.target('/preview/remotes/fixture/chunk.js?x=1'), { port: 5173, path: '/chunk.js?x=1' })
  assert.deepEqual(preview.target('/preview/remotes/fixture/ws?x=1', 'websocket'), { port: 5173, path: '/preview/remotes/fixture/ws?x=1' })
  assert.equal(preview.target('/preview/remotes/other/chunk.js'), undefined)
  assert.equal(preview.target('/preview/remotes/fixture//evil'), undefined)
  assert.equal(preview.update({ count: 2 }).revision, first.revision)
  assert.ok(preview.mount(input).revision > first.revision)
  preview.unmount()
  assert.deepEqual(await preview.snapshot(), { descriptor: null, build: null, available: false })
  assert.throws(() => preview.update({}), /No remote/)
})

for (const prefixed of [false, true]) test(`same-origin preview proxies ${prefixed ? 'prefixed' : 'root'} HTTP assets and documented HMR path, strips credentials and revokes sockets`, async t => {
  const base = '/preview/remotes/fixture'
  const assetBase = prefixed ? base : ''
  const requests: Array<{ path: string; cookie?: string; authorization?: string }> = []
  let body = 'entry build one', valid = true
  const upstream = createServer((request, response) => {
    requests.push({ path: request.url!, cookie: request.headers.cookie, authorization: request.headers.authorization })
    response.setHeader('Set-Cookie', 'untrusted=1')
    if (request.url === assetBase + '/redirect') { response.writeHead(302, { Location: assetBase + '/chunk.js?build=1#asset' }); response.end(); return }
    if (request.url === assetBase + '/relative-redirect') { response.writeHead(302, { Location: 'chunk.js' }); response.end(); return }
    if (request.url?.startsWith(assetBase + '/remoteEntry.js')) response.end(body)
    else if (request.url?.startsWith(assetBase + '/chunk.js')) response.end('chunk')
    else { response.writeHead(404); response.end() }
  })
  const wss = new WebSocketServer({ server: upstream, path: base + '/ws' })
  wss.on('connection', (socket, request) => {
    requests.push({ path: request.url!, cookie: request.headers.cookie, authorization: request.headers.authorization })
    socket.on('message', data => socket.send(data))
  })
  await new Promise<void>(resolve => upstream.listen(0, '127.0.0.1', resolve))
  const port = (upstream.address() as { port: number }).port
  t.after(async () => {
    for (const socket of wss.clients) socket.terminate()
    await new Promise<void>(resolve => wss.close(() => resolve()))
    await new Promise<void>(resolve => upstream.close(() => resolve()))
  })
  const preview = new WorkspacePreview()
  const mount = { name: 'fixture', entry: `http://127.0.0.1:${port}${assetBase}/remoteEntry.js`, exposedModule: './Panel' }
  const descriptor = preview.mount(mount)
  assert.equal(descriptor.entry, base + '/remoteEntry.js', 'An already prefixed entry must not gain a second prefix')
  const auth = new WorkspaceAuth('synthetic-preview-token', async () => valid ? { id: 'fixture' } : null)
  const server = await startWorkspaceServer({ auth, port: 0, preview, revalidateMs: 50,
    backend: { attach: () => ({ receive: async () => {}, dispose() {} }) } })
  t.after(() => server.close())
  const url = server.origin + descriptor.entry
  assert.equal((await fetch(url)).status, 401)
  const login = await fetch(server.origin + '/auth/login', { method: 'POST',
    headers: { Origin: server.origin, 'Content-Type': 'application/json' }, body: JSON.stringify({ token: 'synthetic-preview-token' }) })
  const cookie = login.headers.get('set-cookie')!.split(';')[0]
  const response = await fetch(url + '?build=abc', { headers: { Cookie: cookie, Authorization: 'not-forwarded' } })
  assert.equal(await response.text(), body)
  assert.equal(response.headers.get('set-cookie'), null)
  assert.deepEqual(requests.at(-1), { path: assetBase + '/remoteEntry.js?build=abc', cookie: undefined, authorization: undefined })
  assert.equal(await (await fetch(server.origin + base + '/chunk.js?build=abc', { headers: { Cookie: cookie } })).text(), 'chunk')
  assert.deepEqual(requests.at(-1), { path: assetBase + '/chunk.js?build=abc', cookie: undefined, authorization: undefined })
  const redirect = await fetch(server.origin + '/preview/remotes/fixture/redirect', { headers: { Cookie: cookie }, redirect: 'manual' })
  assert.equal(redirect.headers.get('location'), server.origin + base + '/chunk.js?build=1#asset')
  const relative = await fetch(server.origin + base + '/relative-redirect', { headers: { Cookie: cookie }, redirect: 'manual' })
  assert.equal(relative.headers.get('location'), server.origin + base + '/chunk.js')
  const initial = await preview.snapshot()
  assert.equal(initial.available, true)
  body = 'entry build two'
  assert.notEqual((await preview.snapshot()).build, initial.build)
  const wsUrl = server.origin.replace('http:', 'ws:') + '/preview/remotes/fixture/ws?build=abc'
  const denied = new WebSocket(wsUrl, { headers: { Origin: 'http://attacker.invalid', Cookie: cookie } })
  await once(denied, 'error')
  const socket = new WebSocket(wsUrl, { headers: { Origin: server.origin, Cookie: cookie, Authorization: 'not-forwarded' } })
  socket.on('error', () => {})
  t.after(() => socket.terminate())
  await once(socket, 'open')
  const message = once(socket, 'message')
  socket.send('hmr-update')
  assert.equal((await message)[0].toString(), 'hmr-update')
  assert.deepEqual(requests.at(-1), { path: base + '/ws?build=abc', cookie: undefined, authorization: undefined })
  valid = false
  await once(socket, 'close')
  assert.equal((await fetch(url, { headers: { Cookie: cookie } })).status, 401)
  valid = true
  const again = await fetch(server.origin + '/auth/login', { method: 'POST',
    headers: { Origin: server.origin, 'Content-Type': 'application/json' }, body: JSON.stringify({ token: 'synthetic-preview-token' }) })
  const freshCookie = again.headers.get('set-cookie')!.split(';')[0]
  assert.equal((await fetch(server.origin + '/preview/unmount', { method: 'POST', headers: { Cookie: freshCookie, Origin: 'http://attacker.invalid' } })).status, 403)
  const replaced = new WebSocket(wsUrl, { headers: { Origin: server.origin, Cookie: freshCookie } })
  replaced.on('error', () => {})
  await once(replaced, 'open')
  preview.update({ label: 'updated' })
  assert.equal(replaced.readyState, WebSocket.OPEN, 'Props updates retain the HMR connection')
  const disconnected = once(replaced, 'close')
  preview.unmount()
  await disconnected
  preview.mount(mount)
  await new Promise<void>(resolve => upstream.close(() => resolve()))
  assert.equal((await preview.snapshot()).available, false)
  assert.equal((await fetch(url, { headers: { Cookie: freshCookie } })).status, 502)
})

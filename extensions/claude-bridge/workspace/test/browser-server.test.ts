import assert from 'node:assert/strict'
import { createServer } from 'node:http'
import { test } from 'node:test'
import { withBrowserServer } from '../src/browser-server.js'
import type { HostFrame } from '../src/protocol.js'

test('browser settings reuse shared HTTP helpers with a pinned destination and private credentials', async t => {
  const requests: Array<{ route: string; body: string; auth?: string }> = []
  const server = createServer(async (req, res) => {
    const chunks = []
    for await (const chunk of req) chunks.push(chunk)
    const body = Buffer.concat(chunks).toString()
    requests.push({ route: req.url!, body, auth: req.headers.authorization })
    if (req.url === '/redirect') { res.writeHead(302, { Location: '/redirect-target' }); res.end(); return }
    if (req.url === '/large') { res.end(Buffer.alloc(8 * 1024 * 1024 + 1)); return }
    res.setHeader('Content-Type', 'application/json')
    res.end(JSON.stringify(req.url === '/api/dashboard/tokens' ? { token: 'new-fixture-token', name: JSON.parse(body).name }
      : req.url === '/api/bridge-version' ? { version: 'fixture-version' } : { body, token: 'private-fixture-token' }))
  })
  await new Promise<void>(resolve => server.listen(0, '127.0.0.1', resolve))
  t.after(() => new Promise<void>(resolve => server.close(() => resolve())))
  const address = server.address() as { port: number }
  const frames: HostFrame[] = []
  let delegated = 0
  let disposed = 0
  const backend = withBrowserServer({ attach() { return {
    async receive() { delegated++ }, dispose() { disposed++ },
  } } }, { serverUrl: `ws://127.0.0.1:${address.port}/ws/session`, token: 'private-fixture-token' })
  const view = backend.attach({ async postMessage(frame) { frames.push(frame); return true } })
  t.after(() => view.dispose())
  async function invoke(channel: string, ...args: unknown[]) {
    const id = frames.length + 1
    await view.receive({ kind: 'invoke', id, generation: 'doc', channel, args })
    return frames.at(-1)!
  }
  const version = await invoke('bridge:server-version', 'http://untrusted.invalid')
  assert.deepEqual(version.result, { version: 'fixture-version' })
  const created = await invoke('bridge:create-token', 'http://untrusted.invalid', 'fixture')
  assert.deepEqual(created.result, { token: 'new-fixture-token', name: 'fixture' })
  const resolved = await invoke('bridge:server-fetch', 'http://untrusted.invalid', '/api/dashboard/tokens/resolve', {
    method: 'POST', body: JSON.stringify({ token: '[workspace credential]' }),
  })
  assert.equal(requests.at(-1)?.body, JSON.stringify({ token: 'private-fixture-token' }))
  assert.equal(JSON.stringify(resolved).includes('private-fixture-token'), false)
  await invoke('bridge:server-fetch', '', '/api/sync/test/tree', { headers: { Authorization: 'Bearer attacker' } })
  assert.equal(requests.at(-1)?.auth, 'Bearer private-fixture-token')
  const before = requests.length
  const escaped = await invoke('bridge:server-fetch', '', '//untrusted.invalid/escape')
  assert.equal((escaped.result as any).ok, false)
  assert.equal(requests.length, before)
  const redirected = await invoke('bridge:server-fetch', '', '/redirect')
  assert.equal((redirected.result as any).ok, false)
  assert.equal(requests.some(request => request.route === '/redirect-target'), false)
  const large = await invoke('bridge:server-fetch', '', '/large')
  assert.equal((large.result as any).ok, false)
  await view.receive({ kind: 'send', channel: 'tab:switch', args: ['tab'] })
  assert.equal(delegated, 1)
  assert.equal(disposed, 0)
  view.dispose()
  assert.equal(disposed, 1)
})

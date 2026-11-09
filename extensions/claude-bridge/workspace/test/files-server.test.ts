import assert from 'node:assert/strict'
import { once } from 'node:events'
import { readFile, writeFile } from 'node:fs/promises'
import { join } from 'node:path'
import { test } from 'node:test'
import { WebSocket } from 'ws'
import { WorkspaceAuth } from '../src/auth.js'
import { startWorkspaceServer } from '../src/server.js'
import { fileFixture } from './file-fixture.js'

test('authenticated sockets use real file services and logout detaches their views', async t => {
  const f = await fileFixture(t)
  const server = await startWorkspaceServer({ backend: f.files, port: 0,
    auth: new WorkspaceAuth('synthetic-file-token', async () => ({ id: 'file-fixture' })) })
  t.after(() => server.close())
  const response = await fetch(`${server.origin}/auth/login`, { method: 'POST',
    headers: { Origin: server.origin, 'Content-Type': 'application/json' }, body: JSON.stringify({ token: 'synthetic-file-token' }) })
  const cookie = response.headers.get('set-cookie')!.split(';')[0]
  const connect = () => new WebSocket(server.origin.replace('http:', 'ws:') + '/bridge',
    { headers: { Origin: server.origin, Cookie: cookie } })
  const first = connect()
  const second = connect()
  await Promise.all([once(first, 'open'), once(second, 'open')])
  let id = 0
  async function invoke(socket: WebSocket, channel: string, ...args: unknown[]) {
    const reply = once(socket, 'message')
    const requestId = ++id
    socket.send(JSON.stringify({ kind: 'invoke', id: requestId, generation: 'real-file-document', channel, args }))
    const frame = JSON.parse(String((await reply)[0]))
    assert.equal(frame.id, requestId)
    assert.equal(frame.generation, 'real-file-document')
    return frame
  }
  const path = join(f.root, 'socket.txt')
  assert.equal((await invoke(first, 'file-viewer:write', path, 'from browser')).ok, true)
  assert.equal(await readFile(path, 'utf8'), 'from browser')
  assert.equal((await invoke(second, 'file-viewer:read', path)).result.content, 'from browser')
  // A following invoke is an acknowledgement barrier: watch installation is
  // synchronous inside its earlier dispatch, not an arbitrary sleep.
  first.send(JSON.stringify({ kind: 'send', channel: 'file-viewer:watch', args: [path] }))
  assert.equal((await invoke(first, 'file-tree:list-dir', f.root)).ok, true)
  const event = once(first, 'message')
  await writeFile(path, 'external bytes')
  const frame = JSON.parse(String((await event)[0]))
  assert.deepEqual({ kind: frame.kind, channel: frame.channel, path: frame.args[0].path, content: frame.args[0].content },
    { kind: 'event', channel: 'file-viewer:external-change', path, content: 'external bytes' })
  const missing = await invoke(second, 'file-viewer:read', 'missing')
  assert.equal(missing.ok, false)
  assert.match(missing.error, /ENOENT/)
  assert.equal((await invoke(second, 'unknown-file-operation')).ok, false)
  const closes = [once(first, 'close'), once(second, 'close')]
  assert.equal((await fetch(`${server.origin}/auth/logout`, { method: 'POST', headers: { Origin: server.origin, Cookie: cookie } })).status, 200)
  for (const closed of await Promise.all(closes)) assert.equal(closed[0], 4001)
  await f.waitDisposed(2)
  assert.equal(f.disposals(), 2)
  assert.deepEqual(f.errors, [])
})

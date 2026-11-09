import assert from 'node:assert/strict'
import { once } from 'node:events'
import { test } from 'node:test'
import { WebSocket } from 'ws'
import { WorkspaceAuth } from '../src/auth.js'
import { startWorkspaceServer } from '../src/server.js'
import { fixture, nodeCommand, until, alive } from './terminal-fixture.js'

test('authenticated socket drives real PTY; logout disposes its shell and child', async t => {
  const f = await fixture(t)
  const auth = new WorkspaceAuth('terminal-fixture', async () => ({ id: 'terminal-owner' }))
  const server = await startWorkspaceServer({ auth, backend: f.backend, port: 0 })
  t.after(() => server.close())
  const login = await fetch(`${server.origin}/auth/login`, {
    method: 'POST', headers: { Origin: server.origin, 'Content-Type': 'application/json' },
    body: JSON.stringify({ token: 'terminal-fixture' }),
  })
  const cookie = login.headers.get('set-cookie')!.split(';')[0]
  const socket = new WebSocket(server.origin.replace('http:', 'ws:') + '/bridge', {
    headers: { Origin: server.origin, Cookie: cookie },
  })
  t.after(() => socket.terminate())
  await once(socket, 'open')
  const reply = once(socket, 'message')
  socket.send(JSON.stringify({ kind: 'invoke', id: 1, generation: 'socket-document', channel: 'pty:spawn', args: [{
    cwd: f.root, cols: 80, rows: 24, shellId: process.platform === 'win32' ? 'cmd' : 'sh',
  }] }))
  const frame = JSON.parse(String((await reply)[0]))
  assert.equal(frame.kind, 'invoke-reply')
  assert.equal(frame.generation, 'socket-document')
  assert.equal(frame.ok, true, frame.error)
  socket.send(JSON.stringify({ kind: 'send', channel: 'pty:write', args: [{ ptyId: frame.result.ptyId,
    data: nodeCommand("require('node:fs').writeFileSync('socket-child.pid',String(process.pid));setInterval(()=>{},1000)"),
  }] }))
  await until(async () => !!await f.read('socket-child.pid'))
  const pid = Number(await f.read('socket-child.pid'))
  const closed = once(socket, 'close')
  assert.equal((await fetch(`${server.origin}/auth/logout`, {
    method: 'POST', headers: { Origin: server.origin, Cookie: cookie },
  })).status, 200)
  await closed
  await until(() => !alive(pid), 'Logout left a PTY child running')
  assert.equal(f.disposed(), 1)
})

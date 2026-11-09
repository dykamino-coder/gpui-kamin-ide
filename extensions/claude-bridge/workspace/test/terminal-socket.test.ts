import assert from 'node:assert/strict'
import { once } from 'node:events'
import { test } from 'node:test'
import { WebSocket } from 'ws'
import { WorkspaceAuth } from '../src/auth.js'
import { startWorkspaceServer } from '../src/server.js'
import { fixture, nodeCommand, until, alive } from './terminal-fixture.js'

test('authenticated logout detaches the page; re-login restores its shell and explicit Disconnect kills it', async t => {
  const f = await fixture(t)
  const auth = new WorkspaceAuth('terminal-fixture', async () => ({ id: 'terminal-owner' }))
  const server = await startWorkspaceServer({ auth, backend: f.backend, port: 0 })
  t.after(() => server.close())
  async function login() {
    const response = await fetch(`${server.origin}/auth/login`, {
      method: 'POST', headers: { Origin: server.origin, 'Content-Type': 'application/json' },
      body: JSON.stringify({ token: 'terminal-fixture' }),
    })
    const cookie = response.headers.get('set-cookie')!.split(';')[0]
    const socket = new WebSocket(server.origin.replace('http:', 'ws:') + '/bridge', {
      headers: { Origin: server.origin, Cookie: cookie },
    })
    const frames: any[] = []
    socket.on('message', raw => frames.push(JSON.parse(String(raw))))
    t.after(() => socket.terminate())
    await once(socket, 'open')
    return { cookie, socket, frames }
  }
  const a = await login()
  a.socket.send(JSON.stringify({ kind: 'invoke', id: 1, generation: 'socket-document', channel: 'pty:spawn', args: [{
    tabId: 'socket-tab', cwd: f.root, cols: 80, rows: 24, shellId: process.platform === 'win32' ? 'cmd' : 'sh',
  }] }))
  await until(() => a.frames.some(f => f.kind === 'invoke-reply'))
  const frame = a.frames.find(f => f.kind === 'invoke-reply')
  assert.equal(frame.generation, 'socket-document')
  assert.equal(frame.ok, true, frame.error)
  a.socket.send(JSON.stringify({ kind: 'send', channel: 'pty:write', args: [{ ptyId: frame.result.ptyId,
    data: nodeCommand("console.log('socket-alive');require('node:fs').writeFileSync('socket-child.pid',String(process.pid));setInterval(()=>{},1000)"),
  }] }))
  await until(async () => !!await f.read('socket-child.pid'))
  const pid = Number(await f.read('socket-child.pid'))
  const closed = once(a.socket, 'close')
  assert.equal((await fetch(`${server.origin}/auth/logout`, {
    method: 'POST', headers: { Origin: server.origin, Cookie: a.cookie },
  })).status, 200)
  await closed
  assert.ok(alive(pid))
  const b = await login()
  b.socket.send(JSON.stringify({ kind: 'send', channel: 'workspace:terminal-sync', args: ['socket-tab'] }))
  await until(() => b.frames.some(f => f.channel === 'workspace:terminal-state'))
  const snapshot = b.frames.find(f => f.channel === 'workspace:terminal-state').args[0]
  assert.equal(snapshot.ptyId, frame.result.ptyId)
  assert.match(snapshot.screen, /socket-alive/)
  b.socket.send(JSON.stringify({ kind: 'send', channel: 'disconnect', args: [] }))
  await until(() => !alive(pid), 'Explicit Disconnect left a PTY child running')
  assert.equal(f.disposed(), 1)
})

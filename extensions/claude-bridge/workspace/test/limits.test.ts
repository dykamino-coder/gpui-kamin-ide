import assert from 'node:assert/strict'
import { once } from 'node:events'
import { test, type TestContext } from 'node:test'
import { WebSocket } from 'ws'
import { WorkspaceAuth } from '../src/auth.js'
import { startWorkspaceServer } from '../src/server.js'
import type { WorkspaceBackend } from '../src/protocol.js'

async function connect(t: TestContext, backend: WorkspaceBackend) {
  const server = await startWorkspaceServer({
    auth: new WorkspaceAuth('test-token', async () => ({ id: 'a' })), backend, port: 0,
  })
  t.after(() => server.close())
  const login = await fetch(`${server.origin}/auth/login`, {
    method: 'POST', headers: { Origin: server.origin, 'Content-Type': 'application/json' },
    body: JSON.stringify({ token: 'test-token' }),
  })
  const socket = new WebSocket(server.origin.replace('http:', 'ws:') + '/bridge', {
    headers: { Origin: server.origin, Cookie: login.headers.get('set-cookie')!.split(';')[0] },
  })
  await once(socket, 'open')
  return socket
}

test('backend attach and synchronous dispatch failures close only their socket', async t => {
  for (const backend of [
    { attach() { throw new Error('Private backend detail') } },
    { attach() { return { receive() { throw new Error('Private dispatch detail') }, dispose() {} } } },
  ]) {
    const socket = await connect(t, backend)
    const closed = once(socket, 'close')
    socket.send(JSON.stringify({ kind: 'send', channel: 'fixture', args: [] }))
    const [code, reason] = await closed
    assert.equal(code, 1011)
    assert.equal(String(reason).includes('Private'), false)
  }
})

test('too many outstanding dispatches close the view without starting additional work', async t => {
  let started = 0
  const release: Array<() => void> = []
  const socket = await connect(t, {
    attach() { return {
      async receive() { started++; await new Promise<void>(resolve => release.push(resolve)) },
      dispose() { for (const resolve of release) resolve() },
    } },
  })
  const closed = once(socket, 'close')
  for (let id = 1; id <= 65; id++) socket.send(JSON.stringify({ kind: 'invoke', id, generation: 'a', channel: 'fixture', args: [] }))
  assert.equal((await closed)[0], 4008)
  assert.equal(started, 64)
})

test('oversized inbound and outbound messages are bounded', async t => {
  const large = 'x'.repeat(17 * 1024 * 1024)
  let accepted: boolean | undefined
  const outbound = await connect(t, {
    attach(view) { return {
      async receive() { accepted = await view.postMessage({ kind: 'event', channel: 'fixture', args: [large] }) },
      dispose() {},
    } },
  })
  const outputClosed = once(outbound, 'close')
  outbound.send(JSON.stringify({ kind: 'send', channel: 'fixture', args: [] }))
  assert.equal((await outputClosed)[0], 4008)
  assert.equal(accepted, false)
  const socket = await connect(t, { attach() { return { async receive() { assert.fail('Oversized frame reached backend') }, dispose() {} } } })
  const closed = once(socket, 'close')
  socket.send(JSON.stringify({ kind: 'send', channel: 'fixture', args: [large] }))
  assert.equal((await closed)[0], 1009)
})

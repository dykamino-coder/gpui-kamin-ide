import assert from 'node:assert/strict'
import { afterEach, beforeEach, test } from 'node:test'
import { connectHot } from '../src/preview/hot'

type FakeSocket = { onmessage?: (event: { data: string }) => void; close(): void }
let socket: FakeSocket | undefined
const RealWebSocket = globalThis.WebSocket
beforeEach(() => {
  ;(globalThis as { WebSocket: unknown }).WebSocket = class {
    onmessage?: (event: { data: string }) => void
    constructor() { socket = this }
    close() {}
  }
})
afterEach(() => {
  ;(globalThis as { WebSocket: unknown }).WebSocket = RealWebSocket
  socket = undefined
})

const settle = () => new Promise(resolve => setImmediate(resolve))

/** One remote of a dev server: `own` is the hash of its own compilation. */
function remote(own: string, nextOwn = own) {
  let hash = own
  const calls = { check: 0, applied: 0, invalidated: 0 }
  const hot = {
    status: () => 'idle',
    addStatusHandler: () => {},
    removeStatusHandler: () => {},
    check: async () => { calls.check++; hash = nextOwn; return ['App'] },
  }
  const dispose = connectHot({ c: { App: { exports: {}, hot } }, h: () => hash },
    new URL('ws://workspace.test/preview/remotes/demo/ws'),
    () => { calls.applied++ }, () => { calls.invalidated++ })
  const build = async (serverHash: string) => {
    socket!.onmessage?.({ data: JSON.stringify({ type: 'hash', data: serverHash }) })
    socket!.onmessage?.({ data: JSON.stringify({ type: 'ok' }) })
    await settle()
  }
  return { calls, dispose, build }
}

test('one compiler: an unchanged hash equal to the remote\'s own does not check', async () => {
  const { calls, dispose, build } = remote('aaaa')
  await build('aaaa')
  assert.deepEqual(calls, { check: 0, applied: 0, invalidated: 0 })
  dispose()
})

test('two compilers: a rebuild of the other remote neither checks nor remounts this one', async () => {
  const { calls, dispose, build } = remote('aaaa')
  // MultiStats hash = child hashes joined in compiler order.
  await build('aaaa' + 'bbbb')
  await build('aaaa' + 'cccc')
  assert.deepEqual(calls, { check: 0, applied: 0, invalidated: 0 })
  dispose()
})

test('two compilers: a rebuild of this remote checks and applies its update', async () => {
  const { calls, dispose, build } = remote('aaaa', 'dddd')
  await build('dddd' + 'bbbb')
  assert.deepEqual(calls, { check: 1, applied: 1, invalidated: 0 })
  dispose()
})

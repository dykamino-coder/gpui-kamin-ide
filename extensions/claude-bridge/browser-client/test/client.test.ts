import assert from 'node:assert/strict'
import { test } from 'node:test'
import { createBridgeClient, type BridgeSocketLike } from '../src/index'

// A scripted socket peer: frames the client sends and frames the host pushes.
function fakeHost() {
  const sockets: (BridgeSocketLike & { sent: any[]; push(frame: unknown): void; open(): void; drop(code: number): void })[] = []
  const createSocket = (url: string) => {
    assert.equal(url, 'wss://studio.test/bridge')
    const socket = {
      readyState: 0, bufferedAmount: 0, sent: [] as any[],
      onopen: null as any, onclose: null as any, onmessage: null as any, onerror: null as any,
      send(text: string) { socket.sent.push(JSON.parse(text)) },
      close() { socket.readyState = 3 },
      open() { socket.readyState = 1; socket.onopen?.({}) },
      push(frame: unknown) { socket.onmessage?.({ data: JSON.stringify(frame) }) },
      drop(code: number) { socket.readyState = 3; socket.onclose?.({ code }) },
    }
    sockets.push(socket as never)
    return socket as never
  }
  return { sockets, createSocket }
}
const event = (channel: string, ...args: unknown[]) => ({ kind: 'event', channel, args })
const options = (host: ReturnType<typeof fakeHost>) => ({
  origin: 'https://studio.test', createSocket: host.createSocket,
  fileViewIdentity: { get: () => undefined, set: () => {} },
})

test('connect, sync snapshot, typed sends and event facade use the existing frames', async () => {
  const host = fakeHost()
  const client = createBridgeClient(options(host))
  const connected = client.connect()
  host.sockets[0].open()
  await connected
  assert.equal(client.connectionState, 'connected')

  const changes: unknown[] = []
  client.onTabsChanged((tabs, active) => changes.push([tabs.map(tab => tab.id), active]))
  const entries: unknown[] = []
  client.onJsonlEntries((tab, list) => entries.push([tab, list.length]))
  const synced = client.sync()
  assert.deepEqual(host.sockets[0].sent.at(-1), { kind: 'send', channel: 'workspace:sync', args: [] })
  host.sockets[0].push(event('workspace:snapshot-start', [{ id: 'tab-1' }], 'tab-1'))
  host.sockets[0].push(event('jsonl-entries', 'tab-1', [{ type: 'user' }, { type: 'assistant' }]))
  host.sockets[0].push(event('workspace:snapshot-end'))
  assert.deepEqual(await synced, { tabs: [{ id: 'tab-1' }], active: 'tab-1' })
  assert.deepEqual(entries, [['tab-1', 2]])
  assert.deepEqual(changes, [[['tab-1'], 'tab-1']])
  assert.equal(client.activeTabId(), 'tab-1')

  client.submitPrompt('tab-1', 'build a landing page')
  client.interrupt('tab-1')
  client.switchTab('tab-2')
  assert.deepEqual(host.sockets[0].sent.slice(-3), [
    { kind: 'send', channel: 'pty-submit-text', args: ['tab-1', 'build a landing page'] },
    { kind: 'send', channel: 'session:interrupt', args: ['tab-1'] },
    { kind: 'send', channel: 'tab:switch', args: ['tab-2'] },
  ])

  const created = client.createTab({ cwd: 'site' })
  const request = host.sockets[0].sent.find(frame => frame.kind === 'invoke' && frame.channel === 'tab:create')
  assert.deepEqual(request.args, [{ cwd: 'site' }])
  host.sockets[0].push({ kind: 'invoke-reply', id: request.id, generation: request.generation, ok: true, result: 'tab-3' })
  assert.equal(await created, 'tab-3')
  const resumed = client.resumeSession('conversation-1', { label: 'Landing' })
  const resume = host.sockets[0].sent.find(frame => frame.kind === 'invoke' && frame.channel === 'session:resume')
  assert.deepEqual(resume.args, [{ conversationId: 'conversation-1', sessionLabel: 'Landing' }])
  host.sockets[0].push({ kind: 'invoke-reply', id: resume.id, generation: resume.generation, ok: false, error: 'Unknown conversation' })
  await assert.rejects(resumed, /Unknown conversation/)
  client.disconnect()
  assert.equal(client.connectionState, 'closed')
  assert.throws(() => client.submitPrompt('tab-1', 'late'), /disconnected/)
})

test('preview state is pushed, replayed to late subscribers and survives reconnect with a fresh sync', async () => {
  const host = fakeHost()
  const client = createBridgeClient(options(host))
  const connected = client.connect()
  host.sockets[0].open()
  await connected
  const state = { descriptor: { name: 'demo', entry: '/preview/remotes/demo/remoteEntry.js', exposedModule: './Panel', props: {}, type: 'global', revision: 1 }, build: 'b1', available: true }
  host.sockets[0].push(event('workspace:preview-state', state))
  assert.deepEqual(client.preview.state(), state)
  const seen: unknown[] = []
  client.preview.onState(value => seen.push(value.build))
  assert.deepEqual(seen, ['b1'])

  // A pending invoke rejects on loss; the reconnect re-syncs snapshots, never replays commands.
  const pending = client.invoke('get-config')
  host.sockets[0].drop(1006)
  await assert.rejects(pending, /outcome may be unknown/)
  assert.equal(client.connectionState, 'offline')
  await new Promise(resolve => setTimeout(resolve, 300))
  host.sockets[1].open()
  assert.equal(host.sockets[1].sent.filter(frame => frame.kind === 'invoke').length, 0)
  assert.deepEqual(host.sockets[1].sent.filter(frame => frame.kind === 'send'), [{ kind: 'send', channel: 'workspace:sync', args: [] }])
  host.sockets[1].push(event('workspace:preview-state', { ...state, build: 'b2' }))
  assert.deepEqual(seen, ['b1', 'b2'])
  client.disconnect()
})

test('sign-in loss rejects connect waiters and the next connect opens a new socket', async () => {
  const host = fakeHost()
  const client = createBridgeClient(options(host))
  const first = client.connect()
  host.sockets[0].drop(4001)
  await assert.rejects(first, /sign-in required/)
  assert.equal(client.connectionState, 'authentication-required')
  const second = client.connect()
  assert.equal(host.sockets.length, 2)
  host.sockets[1].open()
  await second
  client.disconnect()
})

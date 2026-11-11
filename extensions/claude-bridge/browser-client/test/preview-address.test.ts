import assert from 'node:assert/strict'
import { afterEach, test } from 'node:test'
import { createBridgeClient } from '../src/client'
import { hostAddresses } from '../src/address'
import { createPreviewRenderer, type HostBase } from '../src/preview'
import type { RemoteRequire } from '../src/preview/hot'

/* eslint-disable @typescript-eslint/no-explicit-any -- a scripted page, remote runtime and federation host */
const globals = globalThis as any
const realFetch = globalThis.fetch
afterEach(() => {
  delete globals.webpackChunkdemo
  delete globals.location
  delete globals.WebSocket
  globalThis.fetch = realFetch
})

const descriptor = { name: 'demo', entry: '/preview/remotes/demo/remoteEntry.js', exposedModule: './Panel', props: {}, type: 'global' as const, revision: 3 }
const source = 'var chunks = self["webpackChunkdemo"] = self["webpackChunkdemo"] || [];'
const react = { React: { createElement: () => null, Component: class {} }, ReactDOM: {}, ReactClient: { createRoot: () => ({ render() {}, unmount() {} }) },
  JSXRuntime: {}, JSXDevRuntime: {}, Refresh: { injectIntoGlobalHook() {}, register() {}, performReactRefresh() {}, createSignatureFunctionForTransform() {} },
  version: '19.0.0' } as any

// Renders one remote the way a webpack container starts: the entry is fetched,
// registered, and its runtime announces itself through the chunk queue.
async function render(page: string, publicPath: string, base?: HostBase) {
  const http: string[] = [], sockets: string[] = []
  globals.location = new URL(page)
  globalThis.fetch = (async (input: RequestInfo | URL) => { http.push(String(input)); return new Response(source) }) as typeof fetch
  globals.WebSocket = class { constructor(url: URL) { sockets.push(String(url)) } close() {} }
  const hot = { status: () => 'idle', check: async () => [], addStatusHandler() {}, removeStatusHandler() {} }
  const require: RemoteRequire = { i: [], c: { App: { exports: {}, hot } }, h: () => 'hash', p: publicPath }
  let plugin: any
  const renderRemote = createPreviewRenderer({ react, base, federation: {
    init(options: any) { plugin = options.plugins[0] },
    registerRemotes(remotes) { http.push(remotes[0].entry) },
    async loadRemote() {
      const queue = globals.webpackChunkdemo ??= []
      queue.push = (chunk: [unknown, unknown, (runtime: RemoteRequire) => void]) => { chunk[2](require); return 0 }
      plugin.afterLoadEntry({ remoteInfo: { name: 'demo' } })
      return { mount: () => () => {} } as never
    },
  } })
  const mounted = await renderRemote({} as HTMLElement, descriptor, 'b1', () => {}, () => {}, () => {})
  assert.equal(mounted.hot, true)
  return { http, sockets, require, mounted }
}

test('without a base the preview addresses are the root of the page origin, as before', async () => {
  const plain = await render('http://page.test/app/index.html', '/preview/remotes/demo/')
  const entry = 'http://page.test/preview/remotes/demo/remoteEntry.js?build=b1&revision=3'
  assert.deepEqual(plain.http, [entry, entry])
  assert.deepEqual(plain.sockets, ['ws://page.test/preview/remotes/demo/ws'])
  // The remote's public path is not touched at all.
  assert.deepEqual(Object.getOwnPropertyDescriptor(plain.require, 'p'), { value: '/preview/remotes/demo/', writable: true, enumerable: true, configurable: true })
  plain.mounted.unmount()
  delete globals.webpackChunkdemo
  const secure = await render('https://page.test/', 'auto')
  assert.deepEqual(secure.sockets, ['wss://page.test/preview/remotes/demo/ws'])
  secure.mounted.unmount()
})

test('HTTP and WebSocket bases are independent and no preview address leaves them', async () => {
  const base = { http: '/gateway/files', ws: 'wss://live.test/gateway/sockets/' }
  const files = 'http://page.test/gateway/files/preview/remotes/demo/'
  // A remote that names the host path itself; publicPath:auto is covered below.
  const run = await render('http://page.test/app/index.html', '/preview/remotes/demo/', base)
  const entry = files + 'remoteEntry.js?build=b1&revision=3'
  assert.deepEqual(run.http, [entry, entry])
  assert.deepEqual(run.sockets, ['wss://live.test/gateway/sockets/preview/remotes/demo/ws'])
  // Chunks, assets and hot updates are `publicPath + file`.
  assert.equal(run.require.p, files)
  run.require.p = '/preview/remotes/demo/static/'
  assert.equal(run.require.p, files + 'static/')
  run.require.p = 'http://page.test/preview/remotes/demo/'
  assert.equal(run.require.p, files)
  for (const address of [...run.http, String(run.require.p)]) assert.ok(address.startsWith('http://page.test/gateway/files/'), address)
  for (const address of run.sockets) assert.ok(address.startsWith('wss://live.test/gateway/sockets/'), address)
  run.mounted.unmount()
  delete globals.webpackChunkdemo

  // publicPath:auto is the entry directory, already under the base: kept as is.
  const auto = await render('http://page.test/', files, { http: 'http://page.test/gateway/files/', ws: '/ws-prefix' })
  assert.equal(auto.require.p, files)
  assert.deepEqual(auto.sockets, ['ws://page.test/ws-prefix/preview/remotes/demo/ws'])
  auto.mounted.unmount()
})

test('a descriptor outside the remote directory is refused before any request', async () => {
  globals.location = new URL('http://page.test/')
  const requests: string[] = []
  globalThis.fetch = (async (input: RequestInfo | URL) => { requests.push(String(input)); return new Response('') }) as typeof fetch
  const renderRemote = createPreviewRenderer({ react, base: { http: '/gateway/files' },
    federation: { init() {}, registerRemotes() {}, loadRemote: async () => null } })
  for (const entry of ['/preview/remotes/other/remoteEntry.js', 'http://elsewhere.test/preview/remotes/demo/remoteEntry.js', '/preview/remotes/demo/../../secret.js']) {
    await assert.rejects(renderRemote({} as HTMLElement, { ...descriptor, entry }, 'b1', () => {}, () => {}, () => {}), /authenticated workspace proxy/)
  }
  assert.deepEqual(requests, [])
})

test('host addresses append the path to the base and never resolve it away', () => {
  const plain = hostAddresses({}, 'https://studio.test/app/')
  assert.equal(plain.http('/preview/state').href, 'https://studio.test/preview/state')
  assert.equal(plain.ws('/bridge').href, 'wss://studio.test/bridge')
  const moved = hostAddresses({ http: '/a/b?x=1#y', ws: 'http://sockets.test:8443/c' }, 'https://studio.test/app/')
  assert.equal(moved.http('/preview/state?x=1').href, 'https://studio.test/a/b/preview/state?x=1')
  assert.equal(moved.ws('/bridge').href, 'ws://sockets.test:8443/c/bridge')
  for (const path of ['bridge', '//evil.test/x', '/../x', '/..%2f..%2fx/../../..', '/\\evil.test/x', '']) {
    assert.throws(() => moved.http(path), /leaves its base/, path)
  }
  assert.throws(() => hostAddresses({ http: 'wss://studio.test/' }, 'https://studio.test/'), /Unsupported host base/)
  assert.throws(() => hostAddresses({ ws: 'ftp://studio.test/' }, 'https://studio.test/'), /Unsupported host base/)
})

test('the client uses the same bases for /bridge and the preview requests', async () => {
  const opened: string[] = [], requested: string[] = []
  const client = createBridgeClient({
    origin: 'https://studio.test', base: { http: '/gateway/files', ws: 'wss://live.test/gateway/sockets' },
    fileViewIdentity: { get: () => undefined, set: () => {} },
    createSocket: url => { opened.push(url); return { readyState: 0, bufferedAmount: 0, send() {}, close() {} } as never },
    fetch: (async (input: RequestInfo | URL) => { requested.push(String(input)); return new Response('{"descriptor":null,"build":null,"available":false}') }) as typeof fetch,
  })
  void client.connect().catch(() => {})
  await client.preview.fetchState()
  await client.preview.unmount()
  client.disconnect()
  assert.deepEqual(opened, ['wss://live.test/gateway/sockets/bridge'])
  assert.deepEqual(requested, ['https://studio.test/gateway/files/preview/state', 'https://studio.test/gateway/files/preview/unmount'])
})

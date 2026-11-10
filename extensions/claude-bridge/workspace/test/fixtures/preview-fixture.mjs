import assert from 'node:assert/strict'
import { fork } from 'node:child_process'
import { once, EventEmitter } from 'node:events'
import { createServer } from 'node:http'
import fs from 'node:fs/promises'
import { createRequire } from 'node:module'
import os from 'node:os'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import { WebSocketServer } from 'ws'

const require = createRequire(new URL('../../../webview/package.json', import.meta.url))
const webpack = require('webpack')
const DevServer = require('webpack-dev-server')
const cleanups = new Map()

export async function previewFixture(t, { conversations = false, holdSync = false, prepare } = {}) {
  const base = process.env.BRIDGE_PREVIEW_ROOT || os.tmpdir()
  const root = await fs.mkdtemp(path.join(base, 'bridgeweb-acc-preview-'))
  const cleanup = []
  cleanups.set(root, cleanup)
  t.after(async () => {
    const errors = []
    for (const dispose of cleanup.reverse()) { try { await dispose() } catch (error) { errors.push(error) } }
    cleanups.delete(root)
    if (errors.length) throw new AggregateError(errors, 'Preview fixture cleanup failed')
  })
  const home = path.join(root, 'home'), tmp = path.join(root, 'tmp')
  await Promise.all([fs.mkdir(home), fs.mkdir(tmp)])
  cleanup.push(async () => {
    assert.equal(path.dirname(root), path.resolve(base))
    assert.ok(path.basename(root).startsWith('bridgeweb-acc-preview-'))
    await fs.rm(root, { recursive: true })
  })
  await prepare?.({ root, home, tmp })
  const heldSync = []
  let syncArrived
  const syncPending = new Promise(resolve => { syncArrived = resolve })
  const reply = res => { res.setHeader('Content-Type', 'application/json'); res.end('{"ok":true}') }
  const http = createServer((req, res) => {
    req.resume()
    if (holdSync && /^\/api\/sync\/[a-f0-9]{16}\/user$/.test(req.url)) { heldSync.push(res); syncArrived(); return }
    reply(res)
  })
  const ws = new WebSocketServer({ server: http })
  await new Promise(resolve => http.listen(0, '127.0.0.1', resolve))
  let peer, id = 0
  let conversation = 0
  const peers = new Map()
  const frames = [], events = new EventEmitter()
  ws.on('connection', socket => {
    peer = socket
    socket.on('message', raw => {
      const frame = JSON.parse(raw.toString())
      frames.push(frame); events.emit('frame')
      if (['session:create', 'session:resume'].includes(frame.type)) {
        const conversationId = frame.conversationId || (conversations ? `00000000-0000-4000-8000-${String(++conversation).padStart(12, '0')}` : undefined)
        socket.send(JSON.stringify({ type: 'session:created', sessionId: conversationId || 'preview-session' }))
        if (conversations) {
          peers.set(conversationId, socket)
          socket.send(JSON.stringify({ type: 'session:conversation-id', conversationId }))
        }
      }
      if (frame.type === 'mcp:response') socket.send(JSON.stringify({ type: 'mcp:result-ack', sessionId: 'preview-session', requestId: frame.requestId, accepted: true }))
    })
  })
  cleanup.push(async () => {
    for (const client of ws.clients) client.terminate()
    for (const response of heldSync.splice(0)) reply(response)
    await new Promise(resolve => ws.close(() => resolve()))
    await new Promise(resolve => http.close(() => resolve()))
  })
  async function boot() {
    const child = fork(fileURLToPath(new URL('./preview-child.cjs', import.meta.url)), [root, `ws://127.0.0.1:${http.address().port}`], {
      cwd: tmp, execArgv: [], stdio: ['ignore', 'pipe', 'pipe', 'ipc'],
      env: { PATH: process.env.PATH, HOME: home, USERPROFILE: home, TMPDIR: tmp, TEMP: tmp, TMP: tmp,
        KAMIN_CACHE_DIR: path.join(root, 'cache'), KAMIN_DISABLE_EXTERNAL_MCP: '1' },
    })
    // Output is deliberately consumed without printing shared runtime payloads.
    child.stdout.resume(); child.stderr.resume()
    const exited = once(child, 'exit')
    const stop = async (crash = false) => {
      if (child.exitCode !== null || child.signalCode !== null) return
      if (crash) { child.kill('SIGKILL'); await exited; return }
      if (child.connected) child.send('shutdown')
      let forced = false
      try { await Promise.race([exited, new Promise((_, reject) => { const timer = setTimeout(() => reject(new Error('Preview child shutdown timeout')), 5000); timer.unref() })]) }
      catch { forced = true; child.kill(); await exited }
      const [code, signal] = await exited
      return { code, signal, forced }
    }
    cleanup.push(stop)
    const [ready] = await once(child, 'message', { signal: AbortSignal.timeout(15000) })
    return { origin: ready.origin, stop }
  }
  let runtime = await boot()
  async function wait(predicate) {
    const signal = AbortSignal.timeout(15000)
    for (;;) {
      const match = frames.find(predicate)
      if (match) return match
      await once(events, 'frame', { signal })
    }
  }
  async function tool(toolName, input, conversationId) {
    const requestId = `preview-${++id}`
    const socket = conversationId === undefined ? peer : peers.get(conversationId)
    assert.ok(socket, 'Synthetic upstream needs an established session')
    socket.send(JSON.stringify({ type: 'mcp:call', requestId, toolName, input }))
    const result = await wait(frame => frame.type === 'mcp:response' && frame.requestId === requestId)
    assert.equal(result.isError, undefined)
    assert.equal(result.result?.isError, undefined, 'Shared MCP handler must succeed')
    return result
  }
  async function waitForSync() {
    await Promise.race([syncPending, new Promise((_, reject) => {
      const timer = setTimeout(() => reject(new Error('Initial user sync did not arrive')), 15000); timer.unref()
    })])
  }
  return { root, get origin() { return runtime.origin }, wait, tool, waitForSync,
    async releaseSync() {
      await waitForSync()
      holdSync = false
      for (const response of heldSync.splice(0)) reply(response)
    },
    receivedFrames() { return structuredClone(frames) },
    send(conversationId, frame) { assert.ok(peers.has(conversationId)); peers.get(conversationId).send(JSON.stringify(frame)) },
    async restart(crash = false) { await runtime.stop(crash); runtime = await boot() },
    async shutdown() { return await runtime.stop() },
  }
}

export async function remoteFixture(t, root, { name = 'preview_fixture', requiredVersion = '19.3.1', hot = true } = {}) {
  const directory = path.join(root, name)
  await fs.mkdir(directory)
  const component = text => `import React from 'react'; export default function Panel(props) {
    const [count, setCount] = React.useState(0);
    if (props.fail) throw new Error('Fixture render failure');
    return React.createElement('button', {id:'remote-counter', onClick:()=>setCount(count+1)}, ${JSON.stringify(text)}+' '+props.label+' '+count);
  }`
  await fs.writeFile(path.join(directory, 'component.js'), component('Before'))
  await fs.writeFile(path.join(directory, 'panel.js'), `
    import React from 'react'; import {createRoot} from 'react-dom/client'; import Component from './component';
    ${hot ? `import ${JSON.stringify(require.resolve('webpack-dev-server/client/index.js') + '?url=%2Fpreview%2Fremotes%2F' + name + '%2Fws&apply=hmr-only&overlay=false&logging=none')};` : ''}
    export function mount(element, initial) {
      window.__previewMounts = (window.__previewMounts || 0) + 1;
      const root = createRoot(element); let props = initial; const render=()=>root.render(React.createElement(Component,props)); render();
      ${hot ? `module.hot.accept('./component', render);` : ''}
      return {update(next){props=next;render()}, unmount(){root.unmount()}};
    }
  `)
  const compiler = webpack({ mode: 'development', context: directory, entry: {}, devtool: false,
    output: { path: path.join(directory, 'dist'), publicPath: 'auto', uniqueName: name, chunkFilename: '[name].[contenthash].js' },
    resolve: { modules: [path.dirname(require.resolve('react/package.json')).replace(/[/\\]react$/, '')] },
    plugins: [new webpack.container.ModuleFederationPlugin({ name, filename: 'remoteEntry.js',
      exposes: { './Panel': './panel.js', './Component': './component.js' },
      shared: Object.fromEntries(['react', 'react-dom', 'react-dom/client'].map(pkg => [pkg, { singleton: true, strictVersion: true, requiredVersion, import: false }])) })],
    infrastructureLogging: { level: 'none' }, stats: 'errors-only' })
  const server = new DevServer({ host: '127.0.0.1', port: 0, hot: hot ? 'only' : false, liveReload: false,
    client: false, allowedHosts: 'all', webSocketServer: hot ? { type: 'ws', options: { path: `/preview/remotes/${name}/ws` } } : false }, compiler)
  await server.start()
  cleanups.get(root).push(async () => { await server.stop(); await new Promise(resolve => compiler.close(() => resolve())) })
  const port = server.server.address().port
  await new Promise((resolve, reject) => compiler.hooks.done.tap('fixture-ready', stats => stats.hasErrors() ? reject(new Error(stats.toString({ all: false, errors: true }))) : resolve()))
  return { name, entry: `http://127.0.0.1:${port}/remoteEntry.js`, exposedModule: './Panel',
    async edit(text) { await fs.writeFile(path.join(directory, 'component.js'), component(text)) },
  }
}

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

export async function previewFixture(t) {
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
  const http = createServer((req, res) => { req.resume(); res.setHeader('Content-Type', 'application/json'); res.end('{"ok":true}') })
  const ws = new WebSocketServer({ server: http })
  await new Promise(resolve => http.listen(0, '127.0.0.1', resolve))
  let peer, id = 0
  const frames = [], events = new EventEmitter()
  ws.on('connection', socket => {
    peer = socket
    socket.on('message', raw => {
      const frame = JSON.parse(raw.toString())
      frames.push(frame); events.emit('frame')
      if (frame.type === 'session:create') socket.send(JSON.stringify({ type: 'session:created', sessionId: 'preview-session' }))
      if (frame.type === 'mcp:response') socket.send(JSON.stringify({ type: 'mcp:result-ack', sessionId: 'preview-session', requestId: frame.requestId, accepted: true }))
    })
  })
  cleanup.push(async () => {
    for (const client of ws.clients) client.terminate()
    await new Promise(resolve => ws.close(() => resolve()))
    await new Promise(resolve => http.close(() => resolve()))
  })
  const child = fork(fileURLToPath(new URL('./preview-child.cjs', import.meta.url)), [root, `ws://127.0.0.1:${http.address().port}`], {
    cwd: tmp, execArgv: [], stdio: ['ignore', 'pipe', 'pipe', 'ipc'],
    env: { PATH: process.env.PATH, HOME: home, USERPROFILE: home, TMPDIR: tmp, TEMP: tmp, TMP: tmp,
      KAMIN_CACHE_DIR: path.join(root, 'cache'), KAMIN_DISABLE_EXTERNAL_MCP: '1' },
  })
  // Output is deliberately consumed without printing shared runtime payloads.
  child.stdout.resume(); child.stderr.resume()
  const exited = once(child, 'exit')
  cleanup.push(async () => {
    if (child.exitCode !== null || child.signalCode !== null) return
    if (child.connected) child.send('shutdown')
    try { await Promise.race([exited, new Promise((_, reject) => { const timer = setTimeout(() => reject(new Error('Preview child shutdown timeout')), 5000); timer.unref() })]) }
    catch { child.kill(); await exited }
  })
  const [ready] = await once(child, 'message', { signal: AbortSignal.timeout(15000) })
  async function wait(predicate) {
    const signal = AbortSignal.timeout(15000)
    for (;;) {
      const match = frames.find(predicate)
      if (match) return match
      await once(events, 'frame', { signal })
    }
  }
  async function tool(toolName, input) {
    const requestId = `preview-${++id}`
    peer.send(JSON.stringify({ type: 'mcp:call', requestId, toolName, input }))
    const result = await wait(frame => frame.type === 'mcp:response' && frame.requestId === requestId)
    assert.equal(result.isError, undefined)
    assert.equal(result.result?.isError, undefined, 'Shared MCP handler must succeed')
    return result
  }
  return { root, origin: ready.origin, wait, tool }
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
    client: false, allowedHosts: 'all', webSocketServer: hot ? { type: 'ws', options: { path: '/ws' } } : false }, compiler)
  await server.start()
  cleanups.get(root).push(async () => { await server.stop(); await new Promise(resolve => compiler.close(() => resolve())) })
  const port = server.server.address().port
  await new Promise((resolve, reject) => compiler.hooks.done.tap('fixture-ready', stats => stats.hasErrors() ? reject(new Error(stats.toString({ all: false, errors: true }))) : resolve()))
  return { name, entry: `http://127.0.0.1:${port}/remoteEntry.js`, exposedModule: './Panel',
    async edit(text) { await fs.writeFile(path.join(directory, 'component.js'), component(text)) },
  }
}

import assert from 'node:assert/strict'
import { fork, spawnSync } from 'node:child_process'
import { once, EventEmitter } from 'node:events'
import fs from 'node:fs'
import { createServer } from 'node:http'
import os from 'node:os'
import path from 'node:path'
import { test } from 'node:test'
import { fileURLToPath } from 'node:url'
import { WebSocket, WebSocketServer } from 'ws'

// Protocol peer only: the CLIENT, handlers, filesystem and socket routing are
// real. This is deliberately not live Claude/Podman/browser acceptance.
test('real BridgeHost pins identity, runs file tools, persists settings and shuts down naturally', async t => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'bridgeweb-runtime-'))
  const home = path.join(root, 'home')
  const tmp = path.join(root, 'tmp')
  fs.mkdirSync(home)
  fs.mkdirSync(tmp)
  const cleanup: Array<() => void | Promise<void>> = []
  t.after(async () => {
    for (const close of cleanup.reverse()) await close()
    assert.equal(path.dirname(root), fs.realpathSync(os.tmpdir()))
    assert.ok(path.basename(root).startsWith('bridgeweb-runtime-'))
    fs.rmSync(root, { recursive: true, force: true })
  })
  const http = createServer((request, response) => {
    request.resume()
    response.setHeader('Content-Type', 'application/json')
    response.end('{"ok":true}')
  })
  const upstream = new WebSocketServer({ server: http })
  await new Promise<void>(resolve => http.listen(0, '127.0.0.1', resolve))
  const address = http.address() as { port: number }
  const upstreamUrl = `ws://127.0.0.1:${address.port}`
  const upstreamEvents = new EventEmitter()
  const upstreamFrames: any[] = []
  let peer: WebSocket
  upstream.on('connection', socket => {
    peer = socket
    socket.on('message', raw => {
      const frame = JSON.parse(raw.toString())
      upstreamFrames.push(frame)
      upstreamEvents.emit('frame')
      if (frame.type === 'session:create') socket.send(JSON.stringify({ type: 'session:created', sessionId: 'fixture-session' }))
      if (frame.type === 'mcp:response') socket.send(JSON.stringify({ type: 'mcp:result-ack', sessionId: 'fixture-session', requestId: frame.requestId, accepted: true }))
    })
  })
  cleanup.push(async () => {
    for (const socket of upstream.clients) socket.terminate()
    await new Promise<void>(resolve => upstream.close(() => resolve()))
    await new Promise<void>(resolve => http.close(() => resolve()))
  })
  const child = fork(fileURLToPath(new URL('./fixtures/runtime-child.cjs', import.meta.url)), [root, upstreamUrl], {
    cwd: tmp, execArgv: [], stdio: ['ignore', 'pipe', 'pipe', 'ipc'],
    env: { PATH: process.env.PATH, SystemRoot: process.env.SystemRoot, ComSpec: process.env.ComSpec,
      HOME: home, USERPROFILE: home, TEMP: tmp, TMP: tmp, TMPDIR: tmp,
      KAMIN_CACHE_DIR: path.join(root, 'cache'), KAMIN_DISABLE_EXTERNAL_MCP: '1' },
  })
  // Keep output private; assertion messages never dump runtime logs/credentials.
  child.stdout!.resume()
  child.stderr!.resume()
  cleanup.push(async () => {
    if (child.exitCode === null && child.signalCode === null) {
      if (process.platform === 'win32') spawnSync('taskkill', ['/PID', String(child.pid), '/T', '/F'], { stdio: 'ignore', windowsHide: true })
      else child.kill()
      await once(child, 'exit', { signal: AbortSignal.timeout(5000) })
    }
  })
  const exited = once(child, 'exit')
  const [ready] = await once(child, 'message', { signal: AbortSignal.timeout(10000) }) as [{ origin: string }]
  assert.ok(ready.origin)
  const login = await fetch(`${ready.origin}/auth/login`, { method: 'POST', headers: { Origin: ready.origin, 'Content-Type': 'application/json' }, body: JSON.stringify({ token: 'synthetic-workspace-token' }) })
  const cookie = login.headers.get('set-cookie')!.split(';')[0]
  const connect = () => new WebSocket(ready.origin.replace('http:', 'ws:') + '/bridge', { headers: { Origin: ready.origin, Cookie: cookie } })
  const a = connect(), b = connect()
  const framesA: any[] = [], framesB: any[] = []
  const eventsA = new EventEmitter(), eventsB = new EventEmitter()
  a.on('message', raw => { framesA.push(JSON.parse(raw.toString())); eventsA.emit('frame') })
  b.on('message', raw => { framesB.push(JSON.parse(raw.toString())); eventsB.emit('frame') })
  await Promise.all([once(a, 'open'), once(b, 'open')])
  cleanup.push(() => { a.terminate(); b.terminate() })
  async function wait(frames: any[], events: EventEmitter, predicate: (value: any) => boolean) {
    const signal = AbortSignal.timeout(5000)
    while (true) {
      const result = frames.find(predicate)
      if (result) return result
      await once(events, 'frame', { signal })
    }
  }
  let id = 0
  async function invoke(channel: string, args: unknown[] = []) {
    const requestId = ++id
    a.send(JSON.stringify({ kind: 'invoke', id: requestId, generation: 'runtime-fixture', channel, args }))
    return wait(framesA, eventsA, frame => frame.kind === 'invoke-reply' && frame.id === requestId)
  }
  assert.equal((await invoke('get-version')).result, 'fixture')
  assert.equal((await invoke('get-cwd')).result, root)
  assert.equal((await invoke('get-config')).result.token, '[workspace credential]')
  assert.equal((await invoke('not:a:channel')).ok, false)
  assert.equal((await invoke('select-folder')).ok, false, 'missing browser services must reject explicitly')
  assert.equal((await invoke('bridge:server-fetch', ['http://attacker.invalid', '/api/tokens'])).ok, false)
  await invoke('write-clipboard', ['clipboard round trip'])
  assert.equal((await invoke('read-clipboard')).result, 'clipboard round trip')
  a.send(JSON.stringify({ kind: 'send', channel: 'layout:set', args: [{ filePanelVisible: true, sidebarWidth: 311 }] }))
  assert.equal((await invoke('layout:get')).result.sidebarWidth, 311)
  const created = await invoke('tab:create', [{ token: 'browser-cannot-replace-token', serverUrl: 'ws://attacker.invalid' }])
  assert.equal(created.ok, true)
  assert.equal(typeof created.result, 'string')
  const session = await wait(upstreamFrames, upstreamEvents, frame => frame.type === 'session:create')
  assert.equal(session.token, 'synthetic-workspace-token')
  assert.equal(session.cwd, root)
  await wait(framesB, eventsB, frame => frame.channel === 'tab:created')
  assert.equal(framesB.some(frame => frame.kind === 'invoke-reply'), false)
  const file = path.join(root, 'proof.txt')
  async function tool(toolName: string, input: Record<string, unknown>) {
    const requestId = `tool-${++id}`
    peer!.send(JSON.stringify({ type: 'mcp:call', requestId, toolName, input }))
    return wait(upstreamFrames, upstreamEvents, frame => frame.type === 'mcp:response' && frame.requestId === requestId)
  }
  await tool('Write', { file_path: file, content: 'before\n' })
  assert.equal(fs.readFileSync(file, 'utf8'), 'before\n')
  await tool('Edit', { file_path: file, old_string: 'before', new_string: 'after' })
  assert.equal(fs.readFileSync(file, 'utf8'), 'after\n')
  assert.match(JSON.stringify((await tool('Read', { file_path: file })).result), /after/)
  assert.match(JSON.stringify((await tool('Glob', { pattern: '*.txt', path: root })).result), /proof\.txt/)
  assert.equal(JSON.stringify(framesA).includes('synthetic-workspace-token'), false)
  assert.equal(JSON.stringify(framesB).includes('synthetic-workspace-token'), false)
  const persisted = JSON.parse(fs.readFileSync(path.join(root, 'storage', 'open-claude-bridge-config.json'), 'utf8'))
  assert.equal(persisted.layout.sidebarWidth, 311)
  assert.equal(persisted.config.serverUrl, upstreamUrl)
  child.send('shutdown')
  const [code, signal] = await Promise.race([
    exited,
    new Promise<never>((_resolve, reject) => {
      const timer = setTimeout(() => reject(new Error('Shared runtime leaked resources after shutdown')), 5000)
      timer.unref()
      void exited.finally(() => clearTimeout(timer))
    }),
  ])
  assert.equal(code, 0, 'shared runtime should exit naturally after disposal')
  assert.equal(signal, null)
})

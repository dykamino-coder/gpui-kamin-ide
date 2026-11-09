import assert from 'node:assert/strict'
import { once, EventEmitter } from 'node:events'
import fs from 'node:fs'
import { createServer } from 'node:http'
import os from 'node:os'
import path from 'node:path'
import { randomUUID } from 'node:crypto'
import { fileURLToPath, pathToFileURL } from 'node:url'
import { test } from 'node:test'
import { WebSocket, WebSocketServer } from 'ws'
import { assertProcessStopped } from './fixtures/external-mcp.js'
import { startRuntimeProcess } from './fixtures/runtime-process.js'
import { waitFrame } from './fixtures/runtime-protocol.js'

test('shared browser host discovers plugin LSP, routes four tools and owns project process lifetime', async t => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'bridgeweb-lsp-'))
  const home = path.join(root, 'home'), tmp = path.join(root, 'tmp')
  const plugin = path.join(home, '.claude', 'plugins', 'cache', 'fixture')
  for (const directory of [tmp, path.join(plugin, '.claude-plugin')]) fs.mkdirSync(directory, { recursive: true })
  const trace = path.join(root, 'lsp-trace.jsonl')
  fs.copyFileSync(fileURLToPath(new URL('./fixtures/lsp-server.cjs', import.meta.url)), path.join(plugin, 'server.cjs'))
  fs.writeFileSync(path.join(plugin, '.claude-plugin', 'plugin.json'), JSON.stringify({ name: 'fixture', lspServers: {
    fixture: { command: 'node', args: ['${CLAUDE_PLUGIN_ROOT}/server.cjs'], extensionToLanguage: { '.proof': 'plaintext' },
      env: { BRIDGEWEB_LSP_TRACE: trace }, restartOnCrash: false },
  } }))
  fs.writeFileSync(path.join(home, '.claude', 'plugins', 'installed_plugins.json'), JSON.stringify({ version: 2,
    plugins: { 'fixture@local': [{ installPath: plugin }] } }))
  fs.writeFileSync(path.join(home, '.claude', 'settings.json'), JSON.stringify({ enabledPlugins: { 'fixture@local': true } }))
  const cleanup: Array<() => void | Promise<void>> = []
  t.after(async () => {
    for (const close of cleanup.reverse()) await close()
    assert.equal(path.dirname(root), fs.realpathSync(os.tmpdir()))
    assert.ok(path.basename(root).startsWith('bridgeweb-lsp-'))
    fs.rmSync(root, { recursive: true, force: true })
  })
  const http = createServer((request, response) => { request.resume(); response.end('{"ok":true}') })
  const upstream = new WebSocketServer({ server: http })
  await new Promise<void>(resolve => http.listen(0, '127.0.0.1', resolve))
  const frames: any[] = [], events = new EventEmitter()
  const peers = new Map<string, WebSocket>()
  upstream.on('connection', socket => socket.on('message', raw => {
    const frame = JSON.parse(raw.toString())
    frames.push(frame); events.emit('frame')
    if (frame.type === 'session:create') {
      const sessionId = `lsp-session-${peers.size}`
      peers.set(frame.cwd + ':' + sessionId, socket)
      socket.send(JSON.stringify({ type: 'session:created', sessionId }))
      socket.send(JSON.stringify({ type: 'session:conversation-id', sessionId, conversationId: randomUUID() }))
    }
    if (frame.type === 'mcp:response') socket.send(JSON.stringify({ type: 'mcp:result-ack', requestId: frame.requestId, accepted: true }))
  }))
  cleanup.push(async () => {
    for (const socket of upstream.clients) socket.terminate()
    await new Promise<void>(resolve => upstream.close(() => resolve()))
    await new Promise<void>(resolve => http.close(() => resolve()))
  })
  const { child, exited, ready } = await startRuntimeProcess(root, tmp, home,
    `ws://127.0.0.1:${(http.address() as { port: number }).port}`, cleanup)
  const login = await fetch(`${ready.origin}/auth/login`, { method: 'POST', headers: { Origin: ready.origin, 'Content-Type': 'application/json' },
    body: JSON.stringify({ token: 'synthetic-workspace-token' }) })
  assert.equal(login.status, 200)
  const cookie = login.headers.get('set-cookie')!.split(';')[0]
  let page!: WebSocket
  const pageFrames: any[] = [], pageEvents = new EventEmitter()
  async function connect() {
    page = new WebSocket(ready.origin.replace('http:', 'ws:') + '/bridge', { headers: { Origin: ready.origin, Cookie: cookie } })
    page.on('message', raw => { pageFrames.push(JSON.parse(raw.toString())); pageEvents.emit('frame') })
    const socket = page
    cleanup.push(() => socket.terminate())
    await once(page, 'open')
  }
  await connect()
  let id = 0
  async function invoke(channel: string, args: unknown[] = []) {
    const requestId = ++id
    page.send(JSON.stringify({ kind: 'invoke', id: requestId, generation: 'lsp-fixture', channel, args }))
    const reply = await waitFrame(pageFrames, pageEvents, frame => frame.kind === 'invoke-reply' && frame.id === requestId)
    assert.equal(reply.ok, true, `Invoke ${channel} failed`)
    return reply.result
  }
  const definitions = await invoke('lsp:list')
  assert.deepEqual(definitions.map((item: any) => [item.id, item.initialized]), [['fixture@local/fixture', false]])
  async function session(cwd: string) {
    const start = frames.length
    const tab = await invoke('tab:create', [{ cwd }])
    await waitFrame(frames, events, frame => frames.indexOf(frame) >= start && frame.type === 'session:create' && frame.cwd === cwd)
    const socket = [...peers.entries()].reverse().find(([key]) => key.startsWith(cwd + ':'))![1]
    return { tab, socket }
  }
  async function tool(socket: WebSocket, toolName: string, file: string) {
    const requestId = `lsp-tool-${++id}`
    socket.send(JSON.stringify({ type: 'mcp:call', requestId, toolName, input: { file, line: 1, character: 2 } }))
    const response = await waitFrame(frames, events, frame => frame.type === 'mcp:response' && frame.requestId === requestId)
      .catch(error => { throw new Error(`Missing ${toolName} response for request ${requestId}`, { cause: error }) })
    return response.result.content.map((entry: any) => entry.text).join('\n')
  }
  const first = await session(root), shared = await session(root)
  const file = path.join(root, 'example.proof')
  fs.writeFileSync(file, 'original document')
  assert.match(await tool(first.socket, 'LspHover', file), /fixture hover: original document/)
  assert.equal((await invoke('lsp:list'))[0].initialized, true)
  assert.equal(await tool(first.socket, 'LspDefinition', file), `${pathToFileURL(file).href}:2:3`)
  assert.equal(await tool(first.socket, 'LspReferences', file), `${pathToFileURL(file).href}:2:3`)
  assert.match(await tool(first.socket, 'LspDiagnostics', file), /\[Warning\].*:2:3 original document \(workspace-fixture\) \[fixture-warning\]/)
  fs.writeFileSync(file, 'updated document')
  assert.match(await tool(shared.socket, 'LspHover', file), /fixture hover: updated document/)
  const records = () => fs.readFileSync(trace, 'utf8').trim().split('\n').map(line => JSON.parse(line))
  const initial = records()
  assert.equal(new Set(initial.map(row => row.pid)).size, 1, 'Same-project tabs must share the server')
  const firstPid = initial[0].pid
  assert.equal(initial[0].params.rootUri, pathToFileURL(root).href)
  assert.equal(initial[0].cwd, root)
  assert.ok(initial.some(row => row.method === 'textDocument/didChange' && row.params.textDocument.version === 2 && row.params.contentChanges[0].text === 'updated document'))
  assert.ok(initial.some(row => row.method === 'textDocument/references' && row.params.context.includeDeclaration === true))
  const closed = once(page, 'close'); page.close(); await closed
  // No browser sockets: real plugin routing and its process must remain alive.
  assert.match(await tool(shared.socket, 'LspHover', file), /updated document/)
  assert.doesNotThrow(() => process.kill(firstPid, 0))
  await connect()
  const otherRoot = path.join(root, 'other-project'); fs.mkdirSync(otherRoot)
  const otherFile = path.join(otherRoot, 'other.proof'); fs.writeFileSync(otherFile, 'isolated project')
  const other = await session(otherRoot)
  assert.match(await tool(other.socket, 'LspHover', otherFile), /isolated project/)
  const otherInit = records().find(row => row.method === 'initialize' && row.params.rootUri === pathToFileURL(otherRoot).href)
  assert.ok(otherInit)
  assert.notEqual(otherInit.pid, firstPid, 'Different projects need separate LSP processes')
  assert.equal(otherInit.cwd, otherRoot)
  page.send(JSON.stringify({ kind: 'send', channel: 'tab:close', args: [first.tab] }))
  await invoke('tab:list')
  assert.match(await tool(shared.socket, 'LspHover', file), /updated document/)
  page.send(JSON.stringify({ kind: 'send', channel: 'tab:close', args: [shared.tab] }))
  assert.equal((await invoke('tab:list')).some((tab: any) => tab.id === shared.tab), false)
  await assertProcessStopped(firstPid, 'last project LSP owner')
  assert.match(await tool(other.socket, 'LspHover', otherFile), /isolated project/)
  child.send('shutdown')
  const [code, signal] = await Promise.race([exited, new Promise<never>((_, reject) => {
    const timer = setTimeout(() => reject(new Error('LSP runtime did not dispose naturally')), 5000)
    timer.unref(); void exited.finally(() => clearTimeout(timer))
  })])
  assert.equal(code, 0); assert.equal(signal, null)
  await assertProcessStopped(otherInit.pid, 'runtime LSP shutdown')
})

import assert from 'node:assert/strict'
import { once, EventEmitter } from 'node:events'
import fs from 'node:fs'
import { createServer } from 'node:http'
import os from 'node:os'
import path from 'node:path'
import { test } from 'node:test'
import { WebSocket, WebSocketServer } from 'ws'
import { assertProcessStopped, deadline, externalMcp } from './fixtures/external-mcp.js'
import { startRuntimeProcess } from './fixtures/runtime-process.js'
import { assertTitleRegeneration, waitFrame as wait } from './fixtures/runtime-protocol.js'

// Protocol peer only: the CLIENT, handlers, filesystem and socket routing are
// real. This is deliberately not live Claude/Podman/browser acceptance.
for (const lifecycle of ['shutdown', 'disconnect then shutdown', 'tab close then shutdown', 'detach then shutdown']) {
test(`real BridgeHost pins identity, runs tools and ${lifecycle} cancels owned work`, async t => {
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
      if (['session:create', 'session:resume'].includes(frame.type)) socket.send(JSON.stringify({ type: 'session:created', sessionId: 'fixture-session' }))
      if (frame.type === 'mcp:response') socket.send(JSON.stringify({ type: 'mcp:result-ack', sessionId: 'fixture-session', requestId: frame.requestId, accepted: true }))
    })
  })
  cleanup.push(async () => {
    for (const socket of upstream.clients) socket.terminate()
    await new Promise<void>(resolve => upstream.close(() => resolve()))
    await new Promise<void>(resolve => http.close(() => resolve()))
  })
  const { child, exited, ready, runtimeOutput } = await startRuntimeProcess(root, tmp, home, upstreamUrl, cleanup)
  const login = await fetch(`${ready.origin}/auth/login`, { method: 'POST', headers: { Origin: ready.origin, 'Content-Type': 'application/json' }, body: JSON.stringify({ token: 'synthetic-workspace-token' }) })
  const cookie = login.headers.get('set-cookie')!.split(';')[0]
  const connect = () => new WebSocket(ready.origin.replace('http:', 'ws:') + '/bridge', { headers: { Origin: ready.origin, Cookie: cookie } })
  let a = connect()
  const b = connect()
  const framesA: any[] = [], framesB: any[] = []
  const eventsA = new EventEmitter(), eventsB = new EventEmitter()
  a.on('message', raw => { framesA.push(JSON.parse(raw.toString())); eventsA.emit('frame') })
  b.on('message', raw => { framesB.push(JSON.parse(raw.toString())); eventsB.emit('frame') })
  await Promise.all([once(a, 'open'), once(b, 'open')])
  cleanup.push(() => { a.terminate(); b.terminate() })
  let id = 0
  async function invoke(channel: string, args: unknown[] = []) {
    const requestId = ++id
    a.send(JSON.stringify({ kind: 'invoke', id: requestId, generation: 'runtime-fixture', channel, args }))
    return wait(framesA, eventsA, frame => frame.kind === 'invoke-reply' && frame.id === requestId)
  }
  assert.equal((await invoke('get-version')).result, 'fixture')
  assert.equal((await invoke('get-cwd')).result, root)
  assert.equal((await invoke('get-config')).result.token, '[workspace credential]')
  const configFile = path.join(root, 'storage', 'open-claude-bridge-config.json')
  function assertCredentialPrivate() {
    const stored = fs.readFileSync(configFile, 'utf8')
    assert.equal(JSON.parse(stored).config.token, '')
    assert.equal(stored.includes('synthetic-workspace-token'), false)
    assert.equal(runtimeOutput().includes('synthetic-workspace-token'), false, 'Runtime output must omit the credential')
  }
  assertCredentialPrivate()
  await invoke('set-config', [{ token: 'forged', serverUrl: 'ws://attacker.invalid' }])
  assertCredentialPrivate()
  assert.equal((await invoke('not:a:channel')).ok, false)
  assert.equal((await invoke('select-folder')).ok, false, 'missing browser services must reject explicitly')
  assert.equal((await invoke('bridge:server-fetch', ['http://attacker.invalid', '/api/tokens'])).ok, false)
  await invoke('write-clipboard', ['clipboard round trip'])
  assert.equal((await invoke('read-clipboard')).result, 'clipboard round trip')
  a.send(JSON.stringify({ kind: 'send', channel: 'layout:set', args: [{ filePanelVisible: true, sidebarWidth: 311 }] }))
  assert.equal((await invoke('layout:get')).result.sidebarWidth, 311)
  await invoke('base-prompt:set', ['Synthetic project instruction: preserve this.'])
  const created = await invoke('tab:create', [{ cwd: '', token: 'browser-cannot-replace-token', serverUrl: 'ws://attacker.invalid' }])
  assert.equal(created.ok, true)
  assert.equal(typeof created.result, 'string')
  const session = await wait(upstreamFrames, upstreamEvents, frame => frame.type === 'session:create')
  assert.equal(session.token, 'synthetic-workspace-token')
  assert.equal(session.cwd, root)
  assert.ok(session.basePrompt.includes('Synthetic project instruction: preserve this.'))
  assert.ok(session.basePrompt.includes('tool host is the Linux workspace container'))
  assert.ok(session.basePrompt.includes(JSON.stringify(root)))
  await wait(framesB, eventsB, frame => frame.channel === 'tab:created')
  assert.equal(framesB.some(frame => frame.kind === 'invoke-reply'), false)
  // The normal UI copies get-config into resume, including its redacted token.
  const browserConfig = (await invoke('get-config')).result
  const project = path.join(root, 'chosen-project')
  fs.mkdirSync(project)
  for (const [index, identity] of [{ ...browserConfig, cwd: 'chosen-project', token: 'forged', serverUrl: 'ws://attacker.invalid' }, { ...browserConfig, cwd: '   ' }].entries()) {
    const conversationId = `00000000-0000-4000-8000-00000000000${index}`
    assert.equal((await invoke('session:resume', [{ ...identity, conversationId }])).ok, true)
    const resumed = await wait(upstreamFrames, upstreamEvents, frame => frame.type === 'session:resume' && frame.conversationId === conversationId)
    assert.equal(resumed.token, 'synthetic-workspace-token')
    const expectedCwd = index === 0 ? project : root
    assert.equal(resumed.cwd, expectedCwd)
    assert.ok(resumed.basePrompt.includes(JSON.stringify(expectedCwd)))
    assert.ok(resumed.basePrompt.includes('Synthetic project instruction: preserve this.'))
    const persisted = JSON.parse(fs.readFileSync(path.join(root, 'storage', 'open-claude-bridge-config.json'), 'utf8'))
    assert.equal(persisted.config.token, '')
    assert.equal(persisted.config.serverUrl, upstreamUrl)
    assertCredentialPrivate()
  }
  assert.equal((await invoke('tab:create', [{ cwd: path.join(root, 'missing-project') }])).ok, false)
  assert.equal((await invoke('tab:create', [{ cwd: 42 }])).ok, false)
  await assertTitleRegeneration(a, invoke, upstreamFrames, upstreamEvents)
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
  const external = await externalMcp()
  cleanup.push(() => external.close())
  const added = await invoke('mcp:add-server', [{ name: 'shutdown-fixture', type: 'http', url: external.url, enabled: true }])
  assert.equal(added.ok, true)
  await wait(framesA, eventsA, frame => frame.channel === 'mcp-servers-changed' &&
    frame.args[0].some((server: any) => server.id === added.result.id && server.status === 'connected'))
  peer!.send(JSON.stringify({ type: 'mcp:call', requestId: 'external-wait', toolName: 'fixture_wait', input: {} }))
  await deadline(external.started, 'External MCP call did not start')
  // Both shells would keep running for a minute if final teardown only detached.
  const longCommand = (file: string) => `node -e 'require("node:fs").writeFileSync("${file}",String(process.pid)); setTimeout(()=>{},60000)'`
  const bg = await tool('Bash', { command: longCommand('background.pid'), run_in_background: true })
  assert.match(JSON.stringify(bg.result), /Background process started/)
  peer!.send(JSON.stringify({ type: 'mcp:call', requestId: 'shell-wait', toolName: 'Bash',
    input: { command: longCommand('foreground.pid') } }))
  const pidFiles = ['foreground.pid', 'background.pid'].map(name => path.join(root, name))
  for (let attempt = 0; attempt < 200; attempt++) {
    if (pidFiles.every(file => fs.existsSync(file) && fs.readFileSync(file, 'utf8').trim())) break
    await new Promise(resolve => setTimeout(resolve, 25))
  }
  assert.ok(pidFiles.every(file => fs.existsSync(file)), 'Long-running Bash commands did not start')
  const shellPids = pidFiles.map(file => Number(fs.readFileSync(file, 'utf8').trim()))
  assert.ok(shellPids.every(pid => Number.isSafeInteger(pid) && pid > 0))
  if (lifecycle === 'detach then shutdown') {
    const activeTab = (await invoke('tab:get-active')).result
    const send = (channel: string, args: unknown[]) => a.send(JSON.stringify({ kind: 'send', channel, args }))
    send('pty-submit-text', [activeTab, 'message once', 'stable-submission'])
    await wait(upstreamFrames, upstreamEvents, frame => frame.type === 'session:submitText' && frame.submissionId === 'stable-submission')
    peer!.send(JSON.stringify({ type: 'session:submission', id: 'stable-submission', state: 'native', uuid: 'native-history' }))
    for (const [identity, text] of [['fallback-accepted', 'fallback accepted with zero views'], ['fallback-pending', 'fallback still waiting']]) {
      send('pty-submit-text', [activeTab, text, identity])
      await wait(upstreamFrames, upstreamEvents, frame => frame.type === 'session:submitText' && frame.submissionId === identity)
      peer!.send(JSON.stringify({ type: 'session:submission', id: identity, state: 'fallback' }))
      await wait(framesA, eventsA, frame => frame.channel === 'session:submission' && frame.args[1].id === identity && frame.args[1].state === 'fallback')
    }
    send('change-permission-mode', [activeTab, 'acceptEdits'])
    await invoke('tab:list') // ingress barrier before the tool asks for permission
    peer!.send(JSON.stringify({ type: 'mcp:call', requestId: 'reopen-permission', toolName: 'Bash', input: { command: 'echo permission-after-reopen' } }))
    await wait(framesA, eventsA, frame => frame.channel === 'permission-request' && frame.args[1].requestId === 'reopen-permission')
    peer!.send(JSON.stringify({ type: 'streaming:entry', entry: { type: 'assistant', uuid: 'stream-turn', __streaming: true,
      message: { id: 'stream-turn', role: 'assistant', content: [{ type: 'text', text: 'before' }] } } }))
    await wait(framesA, eventsA, frame => frame.channel === 'streaming-entry')
    let cancelled = false
    void external.cancelled.then(() => { cancelled = true })
    const closed = Promise.all([once(a, 'close'), once(b, 'close')])
    a.close()
    b.close()
    await closed
    // Zero browser sockets: neither sessions nor their work belong to the page.
    await new Promise(resolve => setTimeout(resolve, 250))
    assert.equal(cancelled, false, 'Socket loss must not abort an external MCP call')
    for (const pid of shellPids) assert.doesNotThrow(() => process.kill(pid, 0))
    assert.equal(peer!.readyState, WebSocket.OPEN, 'Socket loss must keep the upstream connection')
    assert.match(JSON.stringify((await tool('Read', { file_path: file })).result), /after/)
    peer!.send(JSON.stringify({ type: 'streaming:delta', msgId: 'stream-turn', blockIdx: 0, appendText: ' during absence', kind: 'text' }))
    peer!.send(JSON.stringify({ type: 'jsonl:entries', entries: [
      { type: 'user', uuid: 'native-history', _ord: 0, message: { role: 'user', content: 'message once' } },
      { type: 'user', uuid: 'fallback-history', _ord: 1, message: { role: 'user', content: 'fallback accepted with zero views' } },
      { type: 'user', uuid: 'closed-history', _ord: 2, message: { role: 'user', content: 'recorded with zero views' } },
    ] }))
    peer!.send(JSON.stringify({ type: 'jsonl:status', status: 'watching', replayComplete: true }))
    // The subsequent tool result is an upstream ordering barrier.
    await tool('Read', { file_path: file })
    a = connect()
    a.on('message', raw => { framesA.push(JSON.parse(raw.toString())); eventsA.emit('frame') })
    await once(a, 'open')
    assert.equal((await invoke('tab:list')).result.length, 3)
    const start = framesA.length
    send('workspace:sync', [])
    await wait(framesA, eventsA, frame => framesA.indexOf(frame) >= start && frame.channel === 'workspace:snapshot-end')
    const snapshot = framesA.slice(start)
    assert.ok(snapshot.some(frame => frame.channel === 'workspace:snapshot-end'))
    assert.equal(snapshot.filter(frame => frame.channel === 'jsonl-entries').flatMap(frame => frame.args[1]).filter(row => row.uuid === 'closed-history').length, 1)
    assert.equal(snapshot.find(frame => frame.channel === 'streaming-entry').args[1].message.content[0].text, 'before during absence')
    assert.ok(snapshot.some(frame => frame.channel === 'permission-request' && frame.args[1].requestId === 'reopen-permission'))
    assert.ok(snapshot.some(frame => frame.channel === 'session:submission' && frame.args[1].text === 'message once' && frame.args[1].state === 'native'))
    assert.equal(snapshot.find(frame => frame.channel === 'session:submission' && frame.args[1].id === 'stable-submission').args[1].committed, true)
    const fallback = (frames: any[], id: string) => frames.find(frame => frame.channel === 'session:submission' && frame.args[1].id === id).args[1]
    assert.equal(fallback(snapshot, 'fallback-accepted').fallbackQueue.consumed, true, 'Host must reconcile fallback receipts while no browser exists')
    assert.equal(fallback(snapshot, 'fallback-pending').fallbackQueue.consumed, undefined)
    send('pty-submit-text', [activeTab, 'message once', 'stable-submission'])
    send('permission-response', ['reopen-permission', 'allow'])
    const permitted = await wait(upstreamFrames, upstreamEvents, frame => frame.type === 'mcp:response' && frame.requestId === 'reopen-permission')
    assert.match(JSON.stringify(permitted.result), /permission-after-reopen/)
    assert.equal(upstreamFrames.filter(frame => frame.type === 'session:submitText' && frame.submissionId === 'stable-submission').length, 1)
    const c = connect()
    const framesC: any[] = [], eventsC = new EventEmitter()
    c.on('message', raw => { framesC.push(JSON.parse(raw.toString())); eventsC.emit('frame') })
    cleanup.push(() => c.terminate())
    await once(c, 'open')
    const aLength = framesA.length
    c.send(JSON.stringify({ kind: 'send', channel: 'workspace:sync', args: [] }))
    await wait(framesC, eventsC, frame => frame.channel === 'workspace:snapshot-end')
    assert.equal(fallback(framesC, 'fallback-accepted').fallbackQueue.consumed, true, 'A second page must retain the consumed marker')
    assert.equal(fallback(framesC, 'fallback-pending').text, 'fallback still waiting')
    assert.equal(framesA.slice(aLength).some(frame => frame.channel === 'workspace:snapshot-start' || frame.channel === 'jsonl-entries'), false,
      'A second page must not replay/reset the first page')
    assert.equal(framesC.some(frame => frame.channel === 'permission-request'), false, 'Resolved prompts must not return in a later page')
    assert.equal(cancelled, false)
  }
  if (lifecycle === 'disconnect then shutdown' || lifecycle === 'tab close then shutdown') {
    // This is the public browser message, not a direct ConnectionManager call.
    const channel = lifecycle === 'tab close then shutdown' ? 'tab:close' : 'disconnect'
    const active = (await invoke('tab:get-active')).result
    a.send(JSON.stringify({ kind: 'send', channel, args: channel === 'tab:close' ? [active] : [] }))
    assert.equal((await invoke('tab:list')).result.some((tab: any) => tab.id === active), false)
    await deadline(external.cancelled, 'External MCP request survived explicit browser disconnect')
    for (const [index, pid] of shellPids.entries()) await assertProcessStopped(pid, index === 0 ? 'foreground' : 'background')
  }
  child.send('shutdown')
  await deadline(external.cancelled, 'External MCP request survived terminal shutdown')
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
  for (const [index, pid] of shellPids.entries()) await assertProcessStopped(pid, index === 0 ? 'foreground' : 'background')
  assert.equal(upstreamFrames.some(frame => frame.type === 'session:end'), lifecycle === 'tab close then shutdown')
  assertCredentialPrivate()
  for (const entry of fs.readdirSync(root, { recursive: true, withFileTypes: true })) {
    if (!entry.isFile()) continue
    const content = fs.readFileSync(path.join(entry.parentPath, entry.name))
    assert.equal(content.includes(Buffer.from('synthetic-workspace-token')), false,
      'Workspace config, cache, temporary files and diagnostic logs must omit the credential')
  }
})
}

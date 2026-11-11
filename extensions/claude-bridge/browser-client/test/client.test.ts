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

test('chat controls, prompts, subagents and monitors map onto the host channels', async () => {
  const host = fakeHost()
  const client = createBridgeClient(options(host))
  const connected = client.connect()
  host.sockets[0].open()
  await connected
  const socket = host.sockets[0]
  const seen: unknown[] = []
  client.onStreamingDelta((tab, delta) => seen.push(['delta', tab, delta.appendText]))
  client.onActivity((tab, activity) => seen.push(['activity', tab, activity.isWorking]))
  client.onUsage((tab, usage) => seen.push(['usage', tab, usage.model]))
  client.onPermissionRequest((tab, request) => { seen.push(['permission', tab, request.toolName]); client.respondPermission(request.requestId, 'allow_once') })
  client.onQuestion((tab, question) => { seen.push(['question', tab, question.options]); client.answerQuestion(question.requestId, 'beta') })
  client.onElicitation((tab, request) => { seen.push(['elicitation', tab, request.message]); client.respondElicitation(request.requestId, 'accept', { response: 'ok' }) })
  client.onSubagentEntries(batch => seen.push(['subagent', batch.tab, batch.agentName, batch.agentId, batch.entries.length]))
  const monitorEvents: unknown[] = []
  const stopMonitors = client.monitors.onEvent(event => monitorEvents.push(event.type))

  socket.push(event('streaming-delta', 'tab-1', { msgId: 'm', blockIdx: 0, appendText: 'Hel', kind: 'text' }))
  socket.push(event('session:activity', 'tab-1', { rawTitle: '', isWorking: true }))
  socket.push(event('session:usage', 'tab-1', { windows: [], contextPercent: 5, model: 'claude-haiku-5-5' }))
  socket.push(event('permission-request', 'tab-1', { requestId: 'p1', toolName: 'Bash', input: {}, category: 'shell' }))
  socket.push(event('ask-user-question', 'tab-1', { requestId: 'q1', question: 'Pick', options: ['alpha', 'beta'] }))
  socket.push(event('elicitation-request', 'tab-1', { requestId: 'e1', toolName: 'mcp', message: 'Confirm?' }))
  socket.push(event('jsonl-subagent-entries', 'tab-1', 'general-purpose', [{ type: 'assistant' }], 'agent-1'))
  socket.push(event('monitor:started', { id: 'tab-1/p@m/tick', tabId: 'tab-1', pluginId: 'p@m', pluginName: 'p', monitorName: 'tick', description: '', command: 'echo' }))
  socket.push(event('monitor:output', { id: 'tab-1/p@m/tick', entry: { ts: 1, stream: 'stdout', text: 'tick 1' } }))
  socket.push(event('monitor:status', { id: 'tab-1/p@m/tick', status: 'exited', exitCode: 0 }))
  socket.push(event('monitor:stopped', { id: 'tab-1/p@m/tick' }))
  socket.push(event('monitor:error', { tabId: 'tab-1', pluginId: 'p@m', monitorName: 'bad', error: 'refused' }))
  stopMonitors()
  socket.push(event('monitor:stopped', { id: 'late' }))
  assert.deepEqual(seen, [
    ['delta', 'tab-1', 'Hel'], ['activity', 'tab-1', true], ['usage', 'tab-1', 'claude-haiku-5-5'],
    ['permission', 'tab-1', 'Bash'], ['question', 'tab-1', ['alpha', 'beta']], ['elicitation', 'tab-1', 'Confirm?'],
    ['subagent', 'tab-1', 'general-purpose', 'agent-1', 1],
  ])
  assert.deepEqual(monitorEvents, ['started', 'output', 'status', 'stopped', 'error'])

  client.setModel('tab-1', 'haiku')
  client.setEffort('tab-1', 'low')
  client.setToolPermissionMode('tab-1', 'ask')
  client.selectSubagent('tab-1', 'agent-1')
  client.selectSubagent('tab-1', null)
  const sends = socket.sent.filter(frame => frame.kind === 'send').map(frame => [frame.channel, ...frame.args])
  assert.deepEqual(sends, [
    ['permission-response', 'p1', 'allow_once'], ['ask-user-response', 'q1', 'beta'], ['elicitation-response', 'e1', 'accept', { response: 'ok' }],
    ['change-model', 'tab-1', 'haiku'], ['change-effort', 'tab-1', 'low'], ['change-permission-mode', 'tab-1', 'ask'],
    ['jsonl:subagent-select', 'tab-1', 'agent-1'], ['jsonl:subagent-select', 'tab-1', null],
  ])

  const reply = (channel: string, result: unknown) => {
    const frame = socket.sent.filter(sent => sent.kind === 'invoke' && sent.channel === channel).at(-1)
    socket.push({ kind: 'invoke-reply', id: frame.id, generation: frame.generation, ok: true, result })
    return frame.args
  }
  const image = client.saveImage('AAAA', 'image/png')
  assert.deepEqual(reply('save-clipboard-image', '/workspace/.bridge-uploads/a/pasted-image.png'), ['AAAA', 'image/png'])
  assert.equal(await image, '/workspace/.bridge-uploads/a/pasted-image.png')
  const file = client.saveAttachment('BBBB', 'notes.txt')
  assert.deepEqual(reply('save-dropped-file', '/workspace/.bridge-uploads/b/notes.txt'), ['BBBB', 'notes.txt'])
  assert.equal(await file, '/workspace/.bridge-uploads/b/notes.txt')
  const saved = client.savedSessions()
  reply('session:get-saved', [{ conversationId: 'c1', cwd: '/workspace', label: 'Landing' }])
  assert.equal((await saved)[0].conversationId, 'c1')
  const list = client.monitors.list()
  reply('monitors:list', [{ id: 'tab-1/p@m/tick', status: 'running' }])
  assert.equal((await list)[0].status, 'running')
  const log = client.monitors.log('tab-1/p@m/tick')
  assert.deepEqual(reply('monitors:get-log', [{ ts: 1, stream: 'stdout', text: 'tick 1' }]), ['tab-1/p@m/tick'])
  assert.equal((await log)[0].text, 'tick 1')
  client.disconnect()
})

test('MCP, skills, agents, plugins and hooks map onto the host channels', async () => {
  const host = fakeHost()
  const client = createBridgeClient(options(host))
  const connected = client.connect()
  host.sockets[0].open()
  await connected
  const socket = host.sockets[0]
  // Each call is answered in order; the frame's channel and args are the contract.
  const call = async <T>(pending: Promise<T>, channel: string, args: unknown[], result: unknown): Promise<T> => {
    const frame = socket.sent.filter(sent => sent.kind === 'invoke').at(-1)
    assert.equal(frame.channel, channel)
    assert.deepEqual(frame.args, args)
    socket.push({ kind: 'invoke-reply', id: frame.id, generation: frame.generation, ok: true, result })
    return pending
  }
  const server = { id: 's1', name: 'files', type: 'stdio', enabled: true, status: 'connected', tools: [] }
  const draft = { name: 'files', type: 'stdio' as const, command: 'node', args: ['server.js'], enabled: true }

  assert.deepEqual(await call(client.mcp.servers(), 'mcp:get-servers', [], [server]), [server])
  assert.equal(await call(client.mcp.ready(), 'mcp:status', [], true), true)
  assert.deepEqual(await call(client.mcp.addServer(draft), 'mcp:add-server', [draft], server), server)
  assert.equal(await call(client.mcp.updateServer('s1', { name: 'docs' }), 'mcp:update-server', ['s1', { name: 'docs' }], null), null)
  await call(client.mcp.setServerEnabled('s1', false), 'mcp:toggle-server', ['s1', false], undefined)
  assert.deepEqual(await call(client.mcp.testServer('s1'), 'mcp:test-server', ['s1'], { ok: true, tools: ['read'] }), { ok: true, tools: ['read'] })
  assert.deepEqual(await call(client.mcp.oauthStatus('s1'), 'mcp:oauth-status', ['s1'], { connected: false }), { connected: false })
  await call(client.mcp.oauthDisconnect('s1'), 'mcp:oauth-disconnect', ['s1'], { ok: true })
  await call(client.mcp.resources('s1'), 'mcp:get-resources', ['s1'], [])
  await call(client.mcp.readResource('s1', 'file:///a'), 'mcp:read-resource', ['s1', 'file:///a'], { ok: true, result: {} })
  await call(client.mcp.prompts('s1'), 'mcp:get-prompts', ['s1'], [])
  await call(client.mcp.getPrompt('s1', 'greet'), 'mcp:get-prompt', ['s1', 'greet'], { ok: false, error: 'no' })
  await call(client.mcp.getPrompt('s1', 'greet', { who: 'x' }), 'mcp:get-prompt', ['s1', 'greet', { who: 'x' }], { ok: true, result: {} })
  await call(client.mcp.registry(), 'mcp:registry-list', [], { ok: true, servers: [] })
  await call(client.mcp.removeServer('s1'), 'mcp:remove-server', ['s1'], undefined)

  assert.equal((await call(client.skills.list(), 'skills:list', [], [{ name: 'deploy', source: 'project' }]))[0].name, 'deploy')
  await call(client.skills.create('Deploy', '# body'), 'skills:create', ['Deploy', '# body'], { name: 'deploy', fileName: 'SKILL.md', path: '/workspace/.claude/skills/deploy/SKILL.md' })
  assert.equal(await call(client.skills.read('/p/SKILL.md'), 'skills:read', ['/p/SKILL.md'], '# body'), '# body')
  await call(client.skills.remove('/p/SKILL.md'), 'skills:delete', ['/p/SKILL.md'], undefined)
  await call(client.agents.list(), 'agents:list', [], [])

  await call(client.plugins.installed(), 'plugins:list-installed', [], [])
  await call(client.plugins.contents('/plugins/p'), 'plugins:get-contents', ['/plugins/p'], null)
  await call(client.plugins.install('p', 'm', '/market/p'), 'plugins:install', ['p', 'm', '/market/p'], undefined)
  await call(client.plugins.uninstall('p', 'm'), 'plugins:uninstall', ['p', 'm'], undefined)
  assert.deepEqual(await call(client.plugins.setEnabled('p@m', true), 'plugins:set-enabled', ['p@m', true], { ok: true, restartRequired: true }), { ok: true, restartRequired: true })
  await call(client.plugins.options('p@m'), 'plugins:get-options-schema', ['p@m'], { schema: {}, values: {}, sensitiveKeys: [] })
  await call(client.plugins.saveOptions('p@m', { key: 'v' }), 'plugins:save-options', ['p@m', { key: 'v' }], { ok: true, restartRequired: true })
  await call(client.plugins.marketplaces(), 'plugins:list-marketplaces', [], [])
  await call(client.plugins.addMarketplace('m', 'owner/repo'), 'plugins:add-marketplace', ['m', 'owner/repo'], { name: 'm' })
  await call(client.plugins.addMarketplace('m', 'https://git.test/r.git', { token: 't' }), 'plugins:add-marketplace', ['m', 'https://git.test/r.git', { token: 't' }], { name: 'm' })
  await call(client.plugins.updateMarketplace('m'), 'plugins:update-marketplace', ['m'], { ok: true, changed: false })
  await call(client.plugins.refreshMarketplaces(), 'plugins:refresh-all-marketplaces', [], { ok: true, results: [] })
  await call(client.plugins.browse('m'), 'plugins:browse-marketplace', ['m'], [])
  await call(client.plugins.removeMarketplace('m'), 'plugins:remove-marketplace', ['m'], undefined)

  const handler = { type: 'command', command: 'echo hi' }
  // The hook lists arrive wrapped; the facade returns the lists themselves.
  assert.deepEqual(await call(client.hooks.list(), 'hooks:list', [], { hooks: { Stop: [{ hooks: [handler] }] } }), { Stop: [{ hooks: [handler] }] })
  assert.deepEqual(await call(client.hooks.listAll(), 'hooks:list-all', [], { hooks: [] }), [])
  await call(client.hooks.listAll('/workspace/site'), 'hooks:list-all', ['/workspace/site'], { hooks: [] })
  assert.deepEqual(await call(client.hooks.library(), 'hooks:get-library', [], { templates: [{ id: 't1' }] }), [{ id: 't1' }])
  await call(client.hooks.installTemplate('t1'), 'hooks:install-template', ['t1'], { ok: true, restartRequired: true })
  await call(client.hooks.save({ event: 'Stop', handler }), 'hooks:save', [{ event: 'Stop', handler }], { ok: true })
  await call(client.hooks.test({ event: 'Stop', handler, mockPayload: {} }), 'hooks:test', [{ event: 'Stop', handler, mockPayload: {} }], { ok: true, effectiveHost: 'local', result: {} })
  await call(client.hooks.remove('Stop', 0, 0), 'hooks:delete', ['Stop', 0, 0], { ok: true })
  await call(client.hooks.pendingPluginApprovals(), 'hooks:list-pending-plugin-approvals', [], [])
  await call(client.hooks.pluginApproval('p@m'), 'hooks:get-plugin-approval', ['p@m'], { approved: false, hashes: [] })
  await call(client.hooks.setPluginApproval('p@m', ['h1']), 'hooks:set-plugin-approval', ['p@m', ['h1']], { ok: true })

  const seen: unknown[] = []
  const stops = [
    client.mcp.onServersChanged(servers => seen.push(['servers', servers.length])),
    client.mcp.onActivity((tab, activity) => seen.push(['activity', tab, activity.toolName, activity.status])),
    client.plugins.onMarketplaceUpdated(update => seen.push(['marketplace', update.name, update.ok])),
    client.plugins.onBrowseProgress(progress => seen.push(['browse', progress.done, progress.total])),
    client.plugins.onHooksAwaitingApproval(pending => seen.push(['approval', pending.pluginId, pending.hooks.length])),
  ]
  socket.push(event('mcp-servers-changed', [server]))
  socket.push(event('mcp-activity', 'tab-1', { requestId: 'r1', toolName: 'Bash', input: {}, timestamp: 1, status: 'pending' }))
  socket.push(event('marketplaces:updated', { name: 'm', ok: true }))
  socket.push(event('plugins:browse-progress', { marketplace: 'm', done: 1, total: 4, current: 'p' }))
  socket.push(event('plugin-hooks:awaiting-approval', { pluginId: 'p@m', hooks: [{ event: 'Stop', handler, hash: 'h1' }], approvedHashes: [] }))
  for (const stop of stops) stop()
  socket.push(event('mcp-servers-changed', []))
  assert.deepEqual(seen, [
    ['servers', 1], ['activity', 'tab-1', 'Bash', 'pending'], ['marketplace', 'm', true], ['browse', 1, 4], ['approval', 'p@m', 1],
  ])
  client.disconnect()
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

// Checkout-only deterministic CLI: PTY input, JSONL, hook relay and MCP HTTP.
import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import { randomUUID } from 'node:crypto'
import { records, projectSlug, usageLayouts } from './fixtures.mjs'

if (!['development', 'test'].includes(process.env.NODE_ENV) || process.env.BRIDGE_DEV_FAKE_CLI !== '1') {
  throw new Error('Fake CLI is dev/test only')
}
const home = process.env.BRIDGE_FAKE_CLI_HOME
if (!home || !path.isAbsolute(home)) throw new Error('Isolated fake CLI home required')
const args = process.argv.slice(2)
if (args.includes('--version')) {
  process.stdout.write('0.0.0-fake-cli\n')
  process.exit(0)
}
if (args.includes('auth') && args.includes('status')) {
  process.stdout.write(
    JSON.stringify({
      loggedIn: true,
      authMethod: 'fake-cli',
      apiProvider: 'synthetic',
      email: 'fixture@example.invalid',
      plan: 'synthetic',
    }) + '\n',
  )
  process.exit(0)
}
const stream = args.includes('--output-format') && args.includes('stream-json')
const resumed = args.indexOf('--resume')
const conversation = resumed >= 0 ? args[resumed + 1] : randomUUID()
if (!/^[a-zA-Z0-9-]+$/.test(conversation ?? '')) throw new Error('Invalid conversation ID')
const slugDir = path.join(home, '.claude', 'projects', projectSlug(process.cwd()))
const file = path.join(slugDir, `${conversation}.jsonl`)
fs.mkdirSync(slugDir, { recursive: true })
if (resumed >= 0 && !fs.existsSync(file)) throw new Error('No conversation found')
const prior = fs.existsSync(file) ? fs.readFileSync(file, 'utf8').trim().split('\n').filter(Boolean).length : 0
const rec = records(conversation, prior)
const emit = (...entries) => {
  fs.appendFileSync(file, entries.map((e) => JSON.stringify(e) + '\n').join(''))
  if (stream)
    for (const e of entries)
      process.stdout.write(
        JSON.stringify({
          type: e.type,
          message: e.message,
          session_id: conversation,
          uuid: e.uuid,
          parent_tool_use_id: null,
          ...(e.subtype ? { subtype: e.subtype, compact_metadata: e.compactMetadata } : {}),
        }) + '\n',
      )
}
const screen = (text) =>
  process.stdout.write(stream ? '' : `\x1b[2J\x1b[H[FAKE CLAUDE - ${conversation}]\r\n${text}\r\n\u276f `)
const settings = fs.existsSync('.claude/settings.json')
  ? JSON.parse(fs.readFileSync('.claude/settings.json', 'utf8'))
  : {}
const mcp = fs.existsSync('.mcp.json')
  ? JSON.parse(fs.readFileSync('.mcp.json', 'utf8')).mcpServers['user-tools']
  : undefined
// Never execute commands from settings, plugins or input. Only extract the
// Bridge relay's loopback URL and reproduce its HTTP request.
async function localPost(url, body, headers = {}) {
  const target = new URL(url)
  if (target.protocol !== 'http:' || target.hostname !== '127.0.0.1')
    throw new Error('Only loopback Bridge HTTP is allowed')
  const response = await fetch(target, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json', Accept: 'application/json, text/event-stream', ...headers },
    body: JSON.stringify(body),
    signal: AbortSignal.timeout(120000),
  })
  if (!response.ok) throw new Error(`Bridge HTTP ${response.status}`)
  const text = await response.text()
  if (!text) return undefined
  if (
    response.headers.get('content-type')?.includes('text/event-stream') ||
    text.startsWith(':') ||
    text.startsWith('event:') ||
    text.startsWith('data:')
  ) {
    const data = text.split('\n').find((line) => line.startsWith('data:'))
    if (!data) throw new Error('MCP stream ended without a response')
    return JSON.parse(data.slice(5))
  }
  return JSON.parse(text)
}
async function hook(event, extra = {}) {
  for (const matcher of settings.hooks?.[event] ?? []) {
    for (const h of matcher.hooks ?? []) {
      const relay = (h.args ?? []).join(' ').match(/http:\/\/127\.0\.0\.1:\d+\/api\/hooks\/[^"\s]+/)
      if (relay)
        await localPost(
          relay[0],
          { hook_event_name: event, session_id: conversation, cwd: process.cwd(), transcript_path: file, ...extra },
          {
            Authorization: `Bearer ${process.env.CLAUDE_BRIDGE_HOOK_TOKEN}`,
            'X-Bridge-Hook-Relay': 'command',
          },
        )
    }
  }
}
let rpcSeq = 0
const rpc = (method, params) => {
  if (!mcp) throw new Error('Bridge .mcp.json required')
  return localPost(
    mcp.url,
    { jsonrpc: '2.0', ...(method.startsWith('notifications/') ? {} : { id: ++rpcSeq }), method, params },
    mcp.headers,
  )
}
let childFile
let childRec
let agentName = 'worker'
const appendChild = (text) => {
  const e = childRec.assistant([{ type: 'text', text }], 2000, { isSidechain: true })
  fs.appendFileSync(childFile, JSON.stringify(e) + '\n')
}
async function agents(count = 800, team) {
  const agentId = `agent-${conversation}-worker`
  const dir = path.join(slugDir, conversation, 'subagents')
  fs.mkdirSync(dir, { recursive: true })
  childFile = path.join(dir, `${agentId}.jsonl`)
  childRec = records(
    `${conversation}-child`,
    fs.existsSync(childFile) ? fs.readFileSync(childFile, 'utf8').trim().split('\n').length : 0,
  )
  fs.writeFileSync(childFile.replace('.jsonl', '.meta.json'), JSON.stringify({ agentType: agentName }))
  const call = rec.tool('Agent', {
    name: agentName,
    description: 'Synthetic worker',
    subagent_type: 'general-purpose',
    prompt: 'Synthetic task',
    ...(team ? { team_name: team } : {}),
  })
  emit(
    call,
    rec.result(call.message.content[0].id, `Spawned successfully.\nagent_id: ${agentId}\nname: ${agentName}\nrunning.`),
  )
  for (let i = 0; i < count; i++) appendChild(`${conversation} child row ${i}`)
  await hook('SubagentStart', { agent_id: agentId, agent_type: agentName })
}
async function execute(command) {
  if (command === '/exit') {
    await hook('SessionEnd')
    process.exit(0)
  }
  if (command === '/usage') {
    process.stdout.write(
      `\x1b[2J\x1b[H${usageLayouts[process.env.BRIDGE_FAKE_USAGE_LAYOUT ?? 'fable'] ?? usageLayouts.fable}`,
    )
    return
  }
  await hook('UserPromptSubmit', { prompt: command })
  // Input content itself is synthetic control, never user prompt capture.
  emit(rec.user('Synthetic harness command'))
  const [action, parameter] = command.split(/\s+/)
  switch (action) {
    case '/fake-agents':
    case '/fake-team': {
      const count = Number(parameter ?? 800)
      if (!Number.isInteger(count) || count < 1 || count > 10000) throw new Error('Row count must be 1..10000')
      await agents(count, action === '/fake-team' ? 'synthetic-team' : undefined)
      break
    }
    case '/fake-idle':
      emit(rec.idle(agentName))
      await hook('SubagentStop')
      break
    case '/fake-follow-up': {
      if (!childFile) throw new Error('Spawn a worker first')
      const call = rec.tool('SendMessage', { type: 'message', recipient: agentName, content: 'Synthetic follow-up' })
      emit(call, rec.result(call.message.content[0].id, 'Message sent successfully'))
      // Stale idle emitted before the follow-up but delivered after its ACK.
      emit(rec.idle(agentName, '2026-01-01T00:00:01.000Z'))
      appendChild('Synthetic follow-up running')
      break
    }
    case '/fake-report':
      appendChild('Synthetic follow-up complete')
      emit(rec.report(agentName, 'Synthetic report'), rec.idle(agentName))
      break
    case '/fake-pressure':
      emit(rec.assistant([{ type: 'text', text: 'Synthetic pre-compact occupancy' }], 978010))
      break
    case '/compact':
    case '/fake-compact-missing':
      emit(
        rec.compact(action === '/fake-compact-missing' ? null : 15098),
        rec.user('Synthetic compact summary', { isCompactSummary: true }),
      )
      await hook('SessionStart', { source: 'compact' })
      break
    case '/fake-next-usage':
      emit(rec.assistant([{ type: 'text', text: 'Synthetic post-compact usage' }], 17000))
      break
    case '/fake-replay':
      for (let i = 0; i < 3000; i++)
        emit(rec.assistant([{ type: 'text', text: `Synthetic replay ${i} ${'x'.repeat(1024)}` }]))
      break
    case '/fake-console':
      process.stdout.write(
        Array.from(
          { length: 80 },
          (_, i) =>
            `\x1b[36m${conversation} console ${i} \u041a\u0438\u0440\u0438\u043b\u043b\u0438\u0446\u0430 \u65e5\u672c\u8a9e\x1b[0m\r\n`,
        ).join(''),
      )
      break
    case '/fake-mcp': {
      await rpc('initialize', {
        protocolVersion: '2024-11-05',
        capabilities: {},
        clientInfo: { name: 'fake-claude', version: '0.0.0' },
      })
      await rpc('notifications/initialized', {})
      await rpc('tools/list', {})
      const toolScript = fileURLToPath(new URL('./tool-barrier.mjs', import.meta.url))
      const command = `"${process.execPath}" "${toolScript}" "${home}"`
      const call = rec.tool('mcp__user-tools__Bash', { command, description: 'Synthetic tool delivery probe' })
      emit(call)
      screen('MCP pending; interrupt the CLIENT transport now, then complete the tool.')
      const response = await rpc('tools/call', { name: 'Bash', arguments: call.message.content[0].input })
      emit(
        rec.result(
          call.message.content[0].id,
          JSON.stringify(response.result ?? response.error),
          Boolean(response.error || response.result?.isError),
        ),
      )
      break
    }
    default:
      emit(
        rec.assistant([{ type: 'text', text: 'Synthetic response. Use /fake-agents, /fake-mcp, /compact or /usage.' }]),
      )
  }
  await hook('Stop', { last_assistant_message: 'Synthetic turn complete' })
  if (stream)
    process.stdout.write(
      JSON.stringify({
        type: 'result',
        subtype: 'success',
        is_error: false,
        result: 'Synthetic turn complete',
        session_id: conversation,
        uuid: randomUUID(),
        num_turns: 1,
        duration_ms: 0,
        duration_api_ms: 0,
        total_cost_usd: 0,
      }) + '\n',
    )
  // Preserve console scrollback on ordinary input: only startup and /usage redraw.
  process.stdout.write(stream ? '' : '\r\n[FAKE] turn complete\r\n\u276f ')
}
let buffer = ''
let escape = ''
let queue = Promise.resolve()
process.stdin.setEncoding('utf8')
if (process.stdin.isTTY) process.stdin.setRawMode(true)
process.stdin.on('data', (chunk) => {
  for (const ch of chunk) {
    // The actual submitText coordinator wraps input in bracketed-paste CSI.
    // Parse incrementally so split PTY chunks do not turn markers into prompt text.
    if (escape) {
      escape += ch
      if (escape === '\x1b[') continue
      if (!escape.startsWith('\x1b[') || /[@-~]/.test(ch)) escape = ''
      continue
    }
    if (ch === '\x1b') {
      escape = ch
      continue
    }
    if (ch === '\x15') {
      buffer = ''
      continue
    }

    if (ch === '\x03') {
      process.exit(130)
    }
    if (ch === '\r' || ch === '\n') {
      const line = buffer
      buffer = ''
      if (!line) continue
      const command = stream && line.startsWith('{') ? JSON.parse(line).message?.content : line
      queue = queue
        .then(() => execute(String(command)))
        .catch((error) => {
          process.stderr.write(`[FAKE] ${error.message}\n`)
          screen('Synthetic scenario failed')
        })
    } else if (ch === '\x7f' || ch === '\b') buffer = buffer.slice(0, -1)
    else if (ch >= ' ') buffer += ch
  }
})
if (args.includes('/usage')) await execute('/usage')
else {
  if (stream)
    process.stdout.write(
      JSON.stringify({
        type: 'system',
        subtype: 'init',
        session_id: conversation,
        cwd: process.cwd(),
        tools: ['Agent', 'SendMessage', 'mcp__user-tools__Bash'],
        model: 'claude-opus-5-5',
        mcp_servers: [{ name: 'user-tools', status: 'connected' }],
        uuid: randomUUID(),
      }) + '\n',
    )
  screen('Credential-free dev harness')
  await hook('SessionStart', { source: resumed >= 0 ? 'resume' : 'startup' })
}

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
// Opt-in parity with the interactive provider's turn lifecycle. SessionStart
// does not report a selected mode; later hooks report this process's launch mode.
const permissionReceipts = process.env.BRIDGE_FAKE_CLI_PERMISSION_RECEIPTS === '1'
const modeFlag = args.indexOf('--permission-mode')
const explicitMode = modeFlag >= 0 ? args[modeFlag + 1] : undefined
const launchMode = args.includes('--dangerously-skip-permissions')
  ? 'bypassPermissions'
  : ['default', 'acceptEdits', 'plan'].includes(explicitMode)
    ? explicitMode
    : undefined
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
// Continue after the highest record number already used: the transcript a resume
// reads can hold fewer lines than earlier processes numbered, and a reused UUID
// makes the client drop the new row as a duplicate.
const prior = fs.existsSync(file)
  ? fs
      .readFileSync(file, 'utf8')
      .split('\n')
      .filter(Boolean)
      .reduce((max, line, i) => {
        const seq = Number(line.split(`"uuid":"${conversation}-`)[1]?.match(/^(\d+)"/)?.[1] ?? 0)
        return Math.max(max, i + 1, seq)
      }, 0)
  : 0
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
// Bridge passes its hooks (status relay included) in a --settings file.
const settingsArg = args.indexOf('--settings')
if (settingsArg >= 0 && fs.existsSync(args[settingsArg + 1] ?? '')) {
  const extra = JSON.parse(fs.readFileSync(args[settingsArg + 1], 'utf8'))
  settings.hooks = { ...(settings.hooks ?? {}), ...(extra.hooks ?? {}) }
}
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
  const rows = Array.from({ length: count }, (_, i) =>
    childRec.assistant([{ type: 'text', text: `${conversation} child row ${i}` }], 2000, { isSidechain: true }),
  )
  fs.appendFileSync(childFile, rows.map((row) => JSON.stringify(row) + '\n').join(''))
  await hook('SubagentStart', { agent_id: agentId, agent_type: agentName })
}
// Load mode (BRIDGE_FAKE_LOAD_K > 0): an ordinary prompt runs a turn shaped
// like a real coding turn — K tool calls through the Bridge MCP endpoint
// (Read, Grep, Bash in turn) with model "think" time between them, transcript
// records and console output, framed by the CLI's OSC 7501 status. Used to
// load-test the server's main thread with many chats without paying ~200 MB
// per real CLI. No API traffic, no prompt content: synthetic only.
const LOAD_K = Number(process.env.BRIDGE_FAKE_LOAD_K ?? 0)
const LOAD_THINK_MS = Number(process.env.BRIDGE_FAKE_LOAD_THINK_MS ?? 2000)
// CPU a real CLI spends per step (rendering, parsing the stream); calibrated
// so the container's CPU contention matches real CLIs at the same load.
const LOAD_BURN_MS = Number(process.env.BRIDGE_FAKE_LOAD_BURN_MS ?? 0)
const burn = () => {
  const end = Date.now() + LOAD_BURN_MS
  while (Date.now() < end);
}
const LOAD_TOOLS = [
  ['Read', { file_path: process.env.BRIDGE_FAKE_LOAD_FILE ?? '/tmp/work/f1.txt' }],
  ['Grep', { pattern: 'needle', path: process.env.BRIDGE_FAKE_LOAD_DIR ?? '/tmp/work' }],
  ['Bash', { command: 'ls | head -5', description: 'List files' }],
]
const osc = (state) => process.stdout.write(`\x1b]7501;app=claude-code:state=${state}\x07`)
const pause = (ms) => new Promise((resolve) => setTimeout(resolve, ms))
let loadMcpReady = false
async function loadTurn() {
  osc('working')
  if (!loadMcpReady) {
    await rpc('initialize', {
      protocolVersion: '2024-11-05',
      capabilities: {},
      clientInfo: { name: 'fake-claude', version: '0.0.0' },
    })
    await rpc('notifications/initialized', {})
    loadMcpReady = true
  }
  for (let i = 0; i < LOAD_K; i++) {
    await pause(LOAD_THINK_MS)
    const [name, input] = LOAD_TOOLS[i % LOAD_TOOLS.length]
    const call = rec.tool(`mcp__user-tools__${name}`, input)
    emit(call)
    process.stdout.write(`\r\n\x1b[32m\u25cf ${name}\x1b[0m(${JSON.stringify(input)})\r\n`)
    const response = await rpc('tools/call', { name, arguments: input })
    const text = String(response?.result?.content?.[0]?.text ?? '')
    emit(rec.result(call.message.content[0].id, text.slice(0, 4000)))
    process.stdout.write(text.slice(0, 1500).replace(/\n/g, '\r\n'))
    burn()
  }
  await pause(LOAD_THINK_MS)
  emit(rec.assistant([{ type: 'text', text: 'Synthetic load turn complete' }]))
  osc('idle')
}
/** Messages this process received (in memory only, for /fake-recall). */
const received = []
// `/fake-stream <blocks> <intervalMs> <tag>`: a measured load turn. Timed
// assistant blocks around one client tool round trip; each block carries its
// tag, sequence and emit time, so the client measures first-block latency,
// gaps, loss and duplicates without trusting server-side bookkeeping.
async function streamTurn([blocksArg = '20', intervalArg = '50', tag = 'turn']) {
  const blocks = Number(blocksArg)
  const interval = Number(intervalArg)
  if (!Number.isInteger(blocks) || blocks < 1 || blocks > 500) throw new Error('Block count must be 1..500')
  if (!Number.isInteger(interval) || interval < 0 || interval > 5000) throw new Error('Interval must be 0..5000ms')
  if (!/^[a-zA-Z0-9-]{1,64}$/.test(tag)) throw new Error('Invalid stream tag')
  osc('working')
  if (!loadMcpReady) {
    await rpc('initialize', {
      protocolVersion: '2024-11-05',
      capabilities: {},
      clientInfo: { name: 'fake-claude', version: '0.0.0' },
    })
    await rpc('notifications/initialized', {})
    loadMcpReady = true
  }
  const filler = 'synthetic streamed text '.repeat(16)
  for (let seq = 0; seq < blocks; seq++) {
    if (seq === Math.floor(blocks / 2)) {
      const input = { command: `echo ${tag}`, description: 'Synthetic load tool' }
      const call = rec.tool('mcp__user-tools__Bash', input)
      emit(call)
      const response = await rpc('tools/call', { name: 'Bash', arguments: input })
      emit(rec.result(call.message.content[0].id, JSON.stringify(response?.result ?? response?.error ?? null)))
    }
    emit(rec.assistant([{ type: 'text', text: `[load ${tag} seq=${seq} t=${Date.now()}] ${filler}` }]))
    process.stdout.write(`\r\n${tag} block ${seq}`)
    if (interval) await pause(interval)
  }
  osc('idle')
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
  const receipt = permissionReceipts && launchMode ? { permission_mode: launchMode } : {}
  if (permissionReceipts && !stream) osc('working')
  await hook('UserPromptSubmit', { prompt: command, ...receipt })
  // Input content itself is synthetic control, never user prompt capture. A
  // queue acceptance run opts in to the real CLI's behaviour: the transcript
  // records what was typed, so the client can match its pending item to it.
  emit(rec.user(process.env.BRIDGE_FAKE_CLI_RECORD_INPUT === '1' ? command : 'Synthetic harness command'))
  received.push(command)
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
    case '/fake-note': {
      // A synthetic fact the answer carries (user input itself is never captured).
      const row = rec.assistant([{ type: 'text', text: `noted: ${parameter ?? ''}` }])
      if (permissionReceipts) {
        // Native resume repair keeps provider message IDs with the msg_ prefix.
        row.message.id = 'msg_' + row.uuid
        row.timestamp = new Date().toISOString()
        row.message.stop_reason = 'end_turn'
      }
      emit(row)
      break
    }
    case '/fake-recall': {
      // Lines of the appended system prompt holding the word (engine-switch handoff).
      // What this session knows: its system prompt, the messages it received
      // (a hidden context block included) and its own transcript.
      const prompt = args.includes('--append-system-prompt') ? args[args.indexOf('--append-system-prompt') + 1] : ''
      const own = fs.existsSync(file) ? fs.readFileSync(file, 'utf8') : ''
      const said = own
        .split('\n')
        .filter(Boolean)
        .flatMap((line) => (JSON.parse(line).message?.content ?? []).map?.((b) => b.text ?? '') ?? [])
        .filter((text) => !text.startsWith('recall:'))
      // The current message counts without its own command line.
      const inputs = received.map((text, i) =>
        i === received.length - 1 ? text.split('\n').slice(1).join('\n') : text,
      )
      const known = [prompt ?? '', ...inputs, ...said].join('\n')
      const hits = known.split('\n').filter((line) => parameter && line.includes(parameter))
      emit(rec.assistant([{ type: 'text', text: `recall: ${hits.join(' | ')}` }]))
      break
    }
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
    case '/fake-tool': {
      // `/fake-tool <name> <json-args>`: one call of a Bridge MCP tool, its
      // result in the transcript and as the answer (how a model reads it).
      const args = command.slice(action.length + parameter.length + 2).trim()
      await rpc('initialize', {
        protocolVersion: '2024-11-05',
        capabilities: {},
        clientInfo: { name: 'fake-claude', version: '0.0.0' },
      })
      await rpc('notifications/initialized', {})
      const input = args ? JSON.parse(args) : {}
      const call = rec.tool(`mcp__user-tools__${parameter}`, input)
      emit(call)
      const response = await rpc('tools/call', { name: parameter, arguments: input })
      const text = (response.result?.content ?? []).map((c) => c.text ?? '').join('\n')
      emit(
        rec.result(call.message.content[0].id, text || JSON.stringify(response.error), Boolean(response.error)),
        rec.assistant([{ type: 'text', text: `tool said: ${text}` }]),
      )
      break
    }
    case '/fake-stream':
      await streamTurn(command.split(/\s+/).slice(1))
      break
    default:
      if (LOAD_K > 0) {
        await loadTurn()
        break
      }
      emit(
        rec.assistant([{ type: 'text', text: 'Synthetic response. Use /fake-agents, /fake-mcp, /compact or /usage.' }]),
      )
  }
  await hook('Stop', { last_assistant_message: 'Synthetic turn complete', ...receipt })
  if (permissionReceipts && !stream) osc('idle')
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
// Inside a bracketed paste a newline belongs to the message (one submission).
let pasting = false
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
      if (!escape.startsWith('\x1b[') || /[@-~]/.test(ch)) {
        if (escape === '\x1b[200~') pasting = true
        if (escape === '\x1b[201~') pasting = false
        escape = ''
      }
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
    if (pasting && (ch === '\r' || ch === '\n')) {
      buffer += '\n'
      continue
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
    else if (ch >= ' ') {
      buffer += ch
      // A TUI echoes typed/pasted text; the server's terminal submit path waits
      // for that echo before Enter, so a silent CLI would stall it to its cap.
      if (!stream) process.stdout.write(ch)
    }
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
  if (LOAD_K > 0) osc('idle')
  await hook('SessionStart', { source: resumed >= 0 ? 'resume' : 'startup' })
}

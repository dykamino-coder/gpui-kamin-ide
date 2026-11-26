// Real Claude Code native tools versus the shipping Bridge handlers over MCP.
// A deterministic loopback provider emits tool calls; no model/API is contacted.
// Run ONLY in an isolated, network-disabled container with a disposable HOME.
import assert from 'node:assert/strict'
import { spawn, execFileSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import fs from 'node:fs'
import http from 'node:http'
import path from 'node:path'
import { performance } from 'node:perf_hooks'
import readline from 'node:readline'
import { fileURLToPath } from 'node:url'

assert.equal(process.env.BENCH_ISOLATED, '1', 'Use a disposable, network-disabled container')
assert.notEqual(process.platform, 'win32', 'Run CLI/server checks in Podman')
const dir = path.dirname(fileURLToPath(import.meta.url))
const out = process.argv[2] || '/bench/results'
const samples = Number(process.env.BENCH_SAMPLES || 5)
const warmCalls = Number(process.env.BENCH_WARM_CALLS || 5)
assert(samples >= 5 && warmCalls >= 5, 'At least five independent cold and five warm observations are required')
const tools = (process.env.BENCH_TOOLS || 'Read,Write,Edit,Glob,Grep,Bash,NotebookEdit').split(',')
fs.mkdirSync(out, { recursive: true })
const rows = []
const root = '/tmp/bridge-native-tools/corpus'
const hash = bytes => createHash('sha256').update(bytes).digest('hex')
const source = Array.from({ length: 150 }, (_, i) => `line ${i}: текст = ${i};`).join('\n') + '\n'
const notebook = { nbformat: 4, nbformat_minor: 5, metadata: {}, cells: [
  { id: 'fixture-cell', cell_type: 'code', source: ['print("before")'], metadata: {}, outputs: [], execution_count: null },
] }

function fixtures() {
  fs.mkdirSync(root, { recursive: true })
  for (let i = 0; i <= warmCalls; i++) fs.rmSync(path.join(root, `write-${i}.txt`), { force: true })
  fs.writeFileSync(path.join(root, 'read.txt'), source)
  fs.writeFileSync(path.join(root, 'edit.txt'), source)
  fs.writeFileSync(path.join(root, 'notebook.ipynb'), JSON.stringify(notebook))
  fs.mkdirSync(path.join(root, 'tree'), { recursive: true })
  for (let i = 0; i < 64; i++) {
    const file = path.join(root, 'tree', `item-${String(i).padStart(3, '0')}.ts`)
    fs.writeFileSync(file, i % 16 === 0 ? `export const needle = ${i}\n` : `export const other = ${i}\n`)
    fs.utimesSync(file, 1_700_000_000 + i, 1_700_000_000 + i)
  }
}

function input(tool, i) {
  switch (tool) {
    case 'Read': return { file_path: path.join(root, 'read.txt'), offset: 10, limit: 50 }
    case 'Write': return { file_path: path.join(root, `write-${i}.txt`), content: source }
    case 'Edit': return { file_path: path.join(root, 'edit.txt'), old_string: `line ${i}:`, new_string: `changed ${i}:` }
    case 'Glob': return { pattern: '**/*.ts', path: path.join(root, 'tree') }
    case 'Grep': return { pattern: 'needle', path: path.join(root, 'tree'), output_mode: 'files_with_matches' }
    case 'Bash': return { command: "printf 'bridge-bench\\nтекст\\n'", description: 'Print deterministic fixture' }
    case 'NotebookEdit': return { notebook_path: path.join(root, 'notebook.ipynb'), cell_id: 'fixture-cell', new_source: `print(${i})`, edit_mode: 'replace' }
    default: throw new Error(`Missing fixture: ${tool}`)
  }
}

function canonical(tool, result, i, structured) {
  assert(!result.is_error && !result.isError, `Tool returned error: ${JSON.stringify(result)}`)
  const text = typeof result.content === 'string' ? result.content :
    (result.content || []).filter(c => c.type === 'text').map(c => c.text).join('\n')
  if (tool === 'Write') {
    const bytes = fs.readFileSync(path.join(root, `write-${i}.txt`))
    assert.equal(bytes.toString(), source)
    return hash(bytes)
  }
  if (tool === 'Edit') {
    const bytes = fs.readFileSync(path.join(root, 'edit.txt'))
    let expected = source
    for (let n = 0; n <= i; n++) expected = expected.replace(`line ${n}:`, `changed ${n}:`)
    assert.equal(bytes.toString(), expected)
    return hash(bytes)
  }
  if (tool === 'NotebookEdit') {
    const nb = JSON.parse(fs.readFileSync(path.join(root, 'notebook.ipynb'), 'utf8'))
    for (const cell of nb.cells) if (Array.isArray(cell.source)) cell.source = cell.source.join('')
    assert.equal(nb.cells[0].source, `print(${i})`)
    assert.deepEqual(nb.cells[0].outputs, [])
    assert.equal(nb.cells[0].execution_count, null)
    return hash(JSON.stringify(nb))
  }
  if (tool === 'Read') {
    const lines = text.split('\n').filter(line => /^\s*\d+\t/.test(line)).map(line => line.replace(/^\s*\d+\t/, ''))
    const expected = source.split('\n').slice(9, 59)
    assert.deepEqual(lines, expected, `Read result: ${text}`)
    return hash(JSON.stringify(lines))
  }
  if (tool === 'Glob' || tool === 'Grep') {
    // Ignore presentation headers; preserve the actual result set. Glob order
    // is checked separately because both tools promise newest-first results.
    const paths = text.split('\n').filter(line => /^(?:tree\/|\/tmp\/bridge-native-tools\/corpus\/tree\/)item-\d+\.ts$/.test(line))
      .map(file => path.resolve(root, file))
    const expected = Array.from({ length: 64 }, (_, n) => path.join(root, 'tree', `item-${String(n).padStart(3, '0')}.ts`))
      .filter((_, n) => tool === 'Glob' || n % 16 === 0)
    assert.deepEqual([...paths].sort(), expected.sort(), `Listing result: ${text}; structured=${JSON.stringify(structured)}`)
    if (tool === 'Glob' && !structured) assert.deepEqual(paths, [...expected].sort().reverse())
    return hash(JSON.stringify([...paths].sort()))
  }
  if (tool === 'Bash') {
    const stdout = structured?.stdout ?? text
    assert.equal(stdout.trim(), 'bridge-bench\nтекст', `Shell result: ${text}`)
    return hash(stdout.trim())
  }
  throw new Error(`No correctness oracle: ${tool}`)
}

async function listen(server) {
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve))
  return `http://127.0.0.1:${server.address().port}`
}

async function run(tool, route, sample) {
  fixtures()
  const home = `/tmp/bridge-native-tools/home-${route}-${tool}-${sample}`
  fs.mkdirSync(home, { recursive: true })
  const env = { ...process.env, HOME: home, CLAUDE_CONFIG_DIR: home + '/config',
    CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC: '1', BENCH_PROFILE: home + '/bridge-profile' }
  const raw = { tool, route, sample, inputs: [], results: [], handler: [], stderr: '', requests: [] }
  let worker, rpcId = 0
  const pending = new Map()
  async function rpc(request) {
    const id = ++rpcId
    const promise = new Promise((resolve, reject) => pending.set(id, { resolve, reject }))
    worker.stdin.write(JSON.stringify({ id, ...request }) + '\n')
    return promise
  }
  const mcp = http.createServer(async (req, res) => {
    if (req.method !== 'POST') { res.writeHead(405).end(); return }
    try {
      let data = ''; for await (const c of req) data += c
      const body = JSON.parse(data)
      if (body.id === undefined) { res.writeHead(202).end(); return }
      let result
      if (body.method === 'initialize') result = { protocolVersion: '2025-03-26', capabilities: { tools: {} }, serverInfo: { name: 'bridgebench', version: '1' } }
      else if (body.method === 'tools/list') result = { tools: (await rpc({ method: 'tools/list' })).tools }
      else if (body.method === 'tools/call') {
        const response = await rpc({ name: body.params.name, input: body.params.arguments, cwd: root })
        raw.handler.push({ name: body.params.name, ms: response.handlerMs })
        result = response.result
      } else throw new Error(`Unhandled MCP method ${body.method}`)
      res.writeHead(200, { 'content-type': 'application/json' }).end(JSON.stringify({ jsonrpc: '2.0', id: body.id, result }))
    } catch (error) { res.writeHead(500).end(String(error)) }
  })
  if (route === 'bridge') {
    worker = spawn(process.execPath, [path.join(dir, 'native-tool-worker.cjs')], { cwd: root, env, stdio: ['pipe', 'pipe', 'pipe'] })
    readline.createInterface({ input: worker.stdout }).on('line', line => {
      const response = JSON.parse(line)
      const request = pending.get(response.id)
      pending.delete(response.id)
      if (response.error) request?.reject(new Error(response.error)); else request?.resolve(response)
    })
    worker.stderr.on('data', c => { raw.stderr += c })
    worker.on('exit', code => { for (const p of pending.values()) p.reject(new Error(`Worker exit ${code}`)) })
  }
  const mcpUrl = await listen(mcp)
  const prep = ['Edit', 'NotebookEdit'].includes(tool)
  const count = 1 + warmCalls
  let step = 0, active, failure
  const provider = http.createServer(async (req, res) => {
    let data = ''; for await (const c of req) data += c
    const body = JSON.parse(data)
    if (req.url.includes('count_tokens')) { res.end(JSON.stringify({ input_tokens: 0 })); return }
    raw.requests.push({ tools: (body.tools || []).map(t => t.name), model: body.model })
    const isPrep = prep && step === 0
    const index = step - (prep ? 1 : 0)
    const done = index >= count
    const name = route === 'bridge' ? `mcp__bridgebench__${isPrep ? 'Read' : tool}` : isPrep ? 'Read' : tool
    const args = isPrep ? { file_path: path.join(root, tool === 'Edit' ? 'edit.txt' : 'notebook.ipynb') } : done ? {} : input(tool, index)
    const id = `toolu_bench_${step}`
    if (!done) {
      active = { id, i: index, prep: isPrep, start: performance.now() }
      raw.inputs.push({ id, name, input: args })
    }
    res.writeHead(200, { 'content-type': 'text/event-stream' })
    const event = (type, fields) => res.write(`event: ${type}\ndata: ${JSON.stringify({ type, ...fields })}\n\n`)
    event('message_start', { message: { id: `msg_bench_${step}`, type: 'message', role: 'assistant', content: [], model: body.model, stop_reason: null, stop_sequence: null, usage: { input_tokens: 0, output_tokens: 0 } } })
    event('content_block_start', { index: 0, content_block: done ? { type: 'text', text: '' } : { type: 'tool_use', id, name, input: {} } })
    event('content_block_delta', { index: 0, delta: done ? { type: 'text_delta', text: 'done' } : { type: 'input_json_delta', partial_json: JSON.stringify(args) } })
    event('content_block_stop', { index: 0 })
    event('message_delta', { delta: { stop_reason: done ? 'end_turn' : 'tool_use', stop_sequence: null }, usage: { output_tokens: 0 } })
    event('message_stop', {})
    if (!done) active.start = performance.now()
    res.end(); step++
  })
  env.ANTHROPIC_BASE_URL = await listen(provider)
  env.ANTHROPIC_API_KEY = 'local-fixture-not-a-credential'
  const args = ['--print', 'Execute the deterministic benchmark tool calls.', '--model', 'claude-sonnet-4-6',
    '--output-format', 'stream-json', '--verbose', '--dangerously-skip-permissions', '--no-session-persistence',
    '--setting-sources', '', '--strict-mcp-config', '--mcp-config', JSON.stringify({ mcpServers: route === 'bridge' ? { bridgebench: { type: 'http', url: mcpUrl + '/mcp' } } : {} }),
    '--tools', route === 'native' ? [...new Set([tool, ...(prep ? ['Read'] : [])])].join(',') : '']
  const start = performance.now()
  const child = spawn('claude', args, { cwd: root, env, stdio: ['ignore', 'pipe', 'pipe'] })
  const timer = setTimeout(() => { failure ||= new Error('CLI run exceeded 60 seconds'); child.kill('SIGKILL') }, 60_000)
  readline.createInterface({ input: child.stdout }).on('line', line => {
    try {
      const message = JSON.parse(line)
      if (message.type !== 'user') return
      for (const result of message.message?.content || []) {
        if (result.type !== 'tool_result' || result.tool_use_id !== active?.id) continue
        const ms = performance.now() - active.start
        const record = { ...active, ms, result, structured: message.tool_use_result }
        delete record.start
        raw.results.push(record)
        if (!active.prep) record.digest = canonical(tool, result, active.i, message.tool_use_result)
      }
    } catch (error) { failure ||= error; child.kill('SIGKILL') }
  })
  child.stderr.on('data', c => { raw.stderr += c })
  try {
    const code = await new Promise((resolve, reject) => { child.on('exit', resolve); child.on('error', reject) })
    raw.processMs = performance.now() - start
    if (failure) throw failure
    assert.equal(code, 0, `CLI exit ${code}; ${raw.stderr}`)
    const measured = raw.results.filter(r => !r.prep)
    assert.equal(measured.length, count, 'Every scheduled tool call must return a checked result')
    for (const r of measured) rows.push({ tool, route, sample, state: r.i === 0 ? 'cold' : 'warm', i: r.i, ms: r.ms, digest: r.digest })
  } finally {
    clearTimeout(timer)
    worker?.stdin.end()
    worker?.kill()
    provider.closeAllConnections(); provider.close()
    mcp.closeAllConnections(); mcp.close()
    fs.writeFileSync(path.join(out, `${tool}-${route}-${sample}.json`), JSON.stringify(raw, null, 2) + '\n')
  }
}

const metadata = { node: process.version, cli: execFileSync('claude', ['--version'], { encoding: 'utf8' }).trim(),
  ripgrepOverride: process.env.KAMIN_RG_PATH || null,
  ripgrepVersion: process.env.KAMIN_RG_PATH ? execFileSync(process.env.KAMIN_RG_PATH, ['--version'], { encoding: 'utf8' }).trim() : null,
  platform: process.platform, arch: process.arch, samples, warmCalls, workerSha256: hash(fs.readFileSync(path.join(dir, 'native-tool-worker.cjs'))),
  methodology: 'process-cold first handler call (Edit/NotebookEdit require a prior Read); warm repeats in same CLI/handler process; OS filesystem cache is not flushed; loopback model, network-disabled container; MCP HTTP + actual shipping handlers, excludes Bridge WS/PTY/IDE; tool time is local SSE tool-call completion to CLI tool-result event, includes common CLI scheduling and MCP overhead',
  fixtureSha256: hash(source) }
fs.writeFileSync(path.join(out, 'metadata.json'), JSON.stringify(metadata, null, 2) + '\n')
for (const tool of tools) {
  for (let sample = 0; sample < samples; sample++) {
    // Alternate route order to limit systematic cache/order bias.
    for (const route of sample % 2 ? ['bridge', 'native'] : ['native', 'bridge']) {
      await run(tool, route, sample)
      console.log(`${tool} ${route} ${sample + 1}/${samples} checked`)
    }
  }
}
for (const tool of tools) for (let sample = 0; sample < samples; sample++) for (let i = 0; i <= warmCalls; i++) {
  const pair = rows.filter(r => r.tool === tool && r.sample === sample && r.i === i)
  assert.equal(pair.length, 2)
  assert.equal(pair[0].digest, pair[1].digest, `${tool} result mismatch`)
}
fs.writeFileSync(path.join(out, 'samples.json'), JSON.stringify(rows, null, 2) + '\n')
const median = values => [...values].sort((a, b) => a - b)[Math.floor(values.length / 2)]
const summary = tools.map(tool => ({ tool, ...Object.fromEntries(['cold', 'warm'].flatMap(state => ['native', 'bridge'].map(route => {
  const values = rows.filter(r => r.tool === tool && r.state === state && r.route === route).map(r => r.ms)
  return [`${route}_${state}`, { n: values.length, medianMs: median(values) }]
}))), correctness: 'PASS: every result matches fixture oracle and paired native result' }))
fs.writeFileSync(path.join(out, 'summary.json'), JSON.stringify(summary, null, 2) + '\n')
console.log(JSON.stringify(summary, null, 2))

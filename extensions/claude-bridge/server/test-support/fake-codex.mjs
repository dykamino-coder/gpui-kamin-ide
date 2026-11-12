#!/usr/bin/env node
// Fake `codex` CLI for Bridge tests (the codex twin of fake-claude.mjs).
//
// Speaks the subset of the codex-cli 0.160.1 app-server protocol Bridge uses,
// with no network and no real account:
//   codex debug models --bundled   → a two-model catalog
//   codex app-server [-c k=v …]    → newline-delimited JSON-RPC on stdio
//
// State lives under $CODEX_HOME/fake-state (account + threads), so a restarted
// fake (or a restarted Bridge) resumes threads like the real binary does.
// Selected with BRIDGE_CODEX_BIN=<this file>.
//
// Turn input commands (everything else is echoed back as "echo: <text>"):
//   /fake-mcp <Tool> <json-args>  call the thread's Bridge MCP endpoint (real HTTP + Bearer)
//   /fake-slow                    stream until interrupted
//   /fake-fail <message>          fail the turn with <message>
//   /fake-reroute <model>         emit model/rerouted, then echo
//   /fake-crash                   start the turn, then the app-server process dies
//   /fake-hang                    start the turn, then stay silent until interrupted
//
// Knobs: FAKE_CODEX_LOGGED_IN=1 (start signed in), FAKE_CODEX_LOGIN_DELAY_MS,
// FAKE_CODEX_LOGIN_FAIL=<error>, FAKE_CODEX_INIT_FAIL=<error> (stderr + exit 2),
// FAKE_CODEX_START_DELAY_MS (app-server answers nothing for that long).
import fs from 'node:fs'
import path from 'node:path'
import readline from 'node:readline'
import { randomUUID } from 'node:crypto'

const args = process.argv.slice(2)
// `codex --version` (the server's availability probe) needs no CODEX_HOME.
if (args[0] === '--version') {
  process.stdout.write('codex-cli 0.160.1-fake\n')
  process.exit(0)
}
const home = process.env.CODEX_HOME
if (!home) {
  process.stderr.write('fake-codex: CODEX_HOME is required\n')
  process.exit(2)
}
const stateDir = path.join(home, 'fake-state')
const threadsDir = path.join(stateDir, 'threads')
fs.mkdirSync(threadsDir, { recursive: true })
const accountFile = path.join(stateDir, 'account.json')

const MODELS = [
  { slug: 'gpt-fake', display_name: 'GPT Fake', description: 'Fake default model', is_default: true },
  { slug: 'gpt-fake-mini', display_name: 'GPT Fake Mini', description: 'Fake small model', is_default: false },
]

if (args[0] === 'debug' && args[1] === 'models') {
  process.stdout.write(
    'WARNING: fake catalog\n' +
      JSON.stringify({
        models: MODELS.map((m) => ({
          slug: m.slug,
          display_name: m.display_name,
          apply_patch_tool_type: 'freeform',
          shell_type: 'unified_exec',
          experimental_supported_tools: [],
          multi_agent_version: null,
        })),
      }),
  )
  process.exit(0)
}
if (args[0] !== 'app-server') {
  process.stderr.write(`fake-codex: unsupported command ${args.join(' ')}\n`)
  process.exit(2)
}
// The lockdown arrives as -c overrides; keep them for tests to inspect.
const overrides = []
for (let i = 1; i < args.length; i++) if (args[i] === '-c') overrides.push(args[++i])
fs.writeFileSync(path.join(stateDir, 'last-overrides.json'), JSON.stringify(overrides))
if (process.env.FAKE_CODEX_INIT_FAIL) {
  process.stderr.write(`${process.env.FAKE_CODEX_INIT_FAIL}\n`)
  process.exit(2)
}
if (process.env.FAKE_CODEX_LOGGED_IN === '1' && !fs.existsSync(accountFile)) {
  fs.writeFileSync(accountFile, JSON.stringify({ email: 'fake@example.test', planType: 'plus' }))
}

const send = (msg) => process.stdout.write(JSON.stringify({ ...msg, emittedAtMs: Date.now() }) + '\n')
const notify = (method, params) => send({ method, params })
const readAccount = () => {
  try {
    return JSON.parse(fs.readFileSync(accountFile, 'utf8'))
  } catch {
    return null
  }
}
const threadFile = (id) => path.join(threadsDir, `${id}.json`)
const loadThread = (id) => {
  try {
    return JSON.parse(fs.readFileSync(threadFile(id), 'utf8'))
  } catch {
    return null
  }
}
const saveThread = (t) => fs.writeFileSync(threadFile(t.id), JSON.stringify({ ...t, live: undefined }))
const threads = new Map() // id → { id, model, mcp, turns, active, live }
let pendingLogin = null

function mcpFromConfig(config) {
  const bridge = config?.mcp_servers?.bridge
  return bridge ? { url: bridge.url, headers: bridge.http_headers ?? {} } : null
}

class RpcError extends Error {
  constructor(code, message) {
    super(message)
    this.code = code
  }
}

const handlers = {
  initialize: () => ({ userAgent: 'fake-codex/0.160.1', codexHome: home, platformFamily: 'unix', platformOs: 'linux' }),
  'account/read': () => {
    const a = readAccount()
    return {
      account: a ? { type: 'chatgpt', email: a.email, planType: a.planType } : null,
      requiresOpenaiAuth: true,
      workspaceRouting: null,
    }
  },
  'account/login/start': (p) => {
    if (p?.type !== 'chatgptDeviceCode') throw new RpcError(-32602, 'fake supports chatgptDeviceCode only')
    const loginId = randomUUID()
    pendingLogin = loginId
    const delay = Number(process.env.FAKE_CODEX_LOGIN_DELAY_MS ?? 50)
    setTimeout(() => {
      if (pendingLogin !== loginId) return
      pendingLogin = null
      const failure = process.env.FAKE_CODEX_LOGIN_FAIL
      if (!failure) fs.writeFileSync(accountFile, JSON.stringify({ email: 'fake@example.test', planType: 'plus' }))
      notify('account/login/completed', {
        loginId,
        success: !failure,
        error: failure || null,
        onboardingEntrypoint: null,
      })
      if (!failure) notify('account/updated', { authMode: 'chatgpt', planType: 'plus' })
    }, delay)
    return {
      type: 'chatgptDeviceCode',
      loginId,
      verificationUrl: 'https://auth.example.test/codex/device',
      userCode: 'FAKE-1234',
    }
  },
  'account/login/cancel': (p) => {
    if (pendingLogin === p?.loginId) pendingLogin = null
    return { status: 'canceled' }
  },
  'account/logout': () => {
    fs.rmSync(accountFile, { force: true })
    notify('account/updated', { authMode: null, planType: null })
    return {}
  },
  'model/list': () => ({
    data: MODELS.map((m) => ({
      id: m.slug,
      model: m.slug,
      upgrade: null,
      upgradeInfo: null,
      availabilityNux: null,
      displayName: m.display_name,
      description: m.description,
      modelSpecialty: null,
      hidden: false,
      supportedReasoningEfforts: [
        { reasoningEffort: 'low', description: 'low' },
        { reasoningEffort: 'medium', description: 'medium' },
        { reasoningEffort: 'high', description: 'high' },
      ],
      defaultReasoningEffort: 'medium',
      inputModalities: ['text'],
      supportsPersonality: false,
      multiAgentVersion: null,
      additionalSpeedTiers: [],
      serviceTiers: [],
      defaultServiceTier: null,
      availableAccessPrograms: null,
      isDefault: m.is_default,
    })),
    nextCursor: null,
  }),
  'thread/start': (p) => {
    const t = { id: randomUUID(), model: p?.model ?? 'gpt-fake', cwd: p?.cwd, mcp: mcpFromConfig(p?.config), turns: 0 }
    threads.set(t.id, t)
    saveThread(t)
    queueMicrotask(() => notify('thread/started', { thread: { id: t.id } }))
    return { thread: { id: t.id, model: t.model, cwd: t.cwd, path: null }, model: t.model }
  },
  'thread/resume': (p) => {
    const loaded = threads.get(p?.threadId)
    const t = loaded ?? loadThread(p?.threadId)
    if (!t) throw new RpcError(-32600, `thread not found: ${p?.threadId}`)
    // A turn of a previous (dead) process is gone with it.
    if (!loaded) t.active = null
    // The Bearer is per Bridge session: a resume brings a new one.
    t.mcp = mcpFromConfig(p?.config) ?? t.mcp
    if (p?.model) t.model = p.model
    threads.set(t.id, t)
    saveThread(t)
    return { thread: { id: t.id, model: t.model, cwd: t.cwd, path: null }, model: t.model }
  },
  'thread/unsubscribe': (p) => {
    threads.delete(p?.threadId)
    return { status: 'unsubscribed' }
  },
  'turn/start': (p) => {
    const t = threads.get(p?.threadId)
    if (!t) throw new RpcError(-32600, 'thread not loaded')
    if (t.active) throw new RpcError(-32600, 'turn already running')
    if (p.model) t.model = p.model
    const turnId = `turn-${++t.turns}`
    t.active = { id: turnId, interrupted: false, effort: p.effort ?? null, model: t.model }
    saveThread(t)
    const text = (p.input ?? []).map((i) => i.text ?? '').join('\n')
    queueMicrotask(() => void runTurn(t, turnId, text))
    return { turn: { id: turnId, status: 'inProgress', error: null, items: [] } }
  },
  'turn/steer': (p) => {
    const t = threads.get(p?.threadId)
    if (!t?.active || t.active.id !== p.expectedTurnId) throw new RpcError(-32600, 'no active turn')
    const text = (p.input ?? []).map((i) => i.text ?? '').join('\n')
    userMessage(t, t.active.id, text)
    return { turnId: t.active.id }
  },
  'turn/interrupt': (p) => {
    const t = threads.get(p?.threadId)
    if (t?.active && t.active.id === p.turnId) t.active.interrupted = true
    return {}
  },
}

function item(t, turnId, it, started = true) {
  if (started) notify('item/started', { threadId: t.id, turnId, item: it, startedAtMs: Date.now() })
  notify('item/completed', { threadId: t.id, turnId, item: it, completedAtMs: Date.now() })
}

function userMessage(t, turnId, text) {
  item(
    t,
    turnId,
    { type: 'userMessage', id: randomUUID(), content: [{ type: 'text', text, text_elements: [] }] },
    false,
  )
}

function agentMessage(t, turnId, text) {
  const id = randomUUID()
  notify('item/started', { threadId: t.id, turnId, item: { type: 'agentMessage', id, text: '' } })
  for (const chunk of text.match(/.{1,6}/gs) ?? []) {
    notify('item/agentMessage/delta', { threadId: t.id, turnId, itemId: id, delta: chunk })
  }
  notify('item/completed', { threadId: t.id, turnId, item: { type: 'agentMessage', id, text } })
}

async function callMcp(t, tool, argsJson) {
  if (!t.mcp) return { error: 'no Bridge MCP server configured for this thread' }
  const res = await fetch(t.mcp.url, {
    method: 'POST',
    headers: {
      ...t.mcp.headers,
      'Content-Type': 'application/json',
      Accept: 'application/json, text/event-stream',
    },
    body: JSON.stringify({ jsonrpc: '2.0', id: 1, method: 'tools/call', params: { name: tool, arguments: argsJson } }),
  })
  const body = await res.text()
  if (!res.ok) return { error: `MCP HTTP ${res.status}: ${body.slice(0, 200)}` }
  const json = res.headers.get('content-type')?.includes('text/event-stream')
    ? JSON.parse(
        body
          .split('\n')
          .filter((l) => l.startsWith('data:'))
          .map((l) => l.slice(5).trim())
          .filter(Boolean)
          .at(-1) ?? 'null',
      )
    : JSON.parse(body)
  if (json?.error) return { error: json.error.message }
  return { result: json?.result ?? null }
}

async function runTurn(t, turnId, text) {
  const started = Date.now()
  notify('turn/started', { threadId: t.id, turn: { id: turnId, status: 'inProgress', error: null, items: [] } })
  userMessage(t, turnId, text)
  let status = 'completed'
  let error = null
  try {
    if (text.startsWith('/fake-mcp ')) {
      const [, tool, ...rest] = text.split(' ')
      const argsJson = rest.length ? JSON.parse(rest.join(' ')) : {}
      const id = randomUUID()
      const base = { type: 'mcpToolCall', id, server: 'bridge', tool, arguments: argsJson }
      notify('item/started', {
        threadId: t.id,
        turnId,
        item: { ...base, status: 'inProgress', result: null, error: null },
      })
      const out = await callMcp(t, tool, argsJson)
      notify('item/completed', {
        threadId: t.id,
        turnId,
        item: out.error
          ? { ...base, status: 'failed', result: null, error: { message: out.error } }
          : { ...base, status: 'completed', result: out.result, error: null },
      })
      agentMessage(t, turnId, out.error ? `tool failed: ${out.error}` : 'tool done')
    } else if (text === '/fake-slow') {
      const id = randomUUID()
      notify('item/started', { threadId: t.id, turnId, item: { type: 'agentMessage', id, text: '' } })
      while (!t.active?.interrupted) {
        notify('item/agentMessage/delta', { threadId: t.id, turnId, itemId: id, delta: '.' })
        await new Promise((r) => setTimeout(r, 20))
      }
      status = 'interrupted'
    } else if (text === '/fake-crash') {
      agentMessage(t, turnId, 'about to crash')
      setTimeout(() => process.exit(9), 20)
      return
    } else if (text === '/fake-hang') {
      while (!t.active?.interrupted) await new Promise((r) => setTimeout(r, 20))
      status = 'interrupted'
    } else if (text.startsWith('/fake-fail')) {
      status = 'failed'
      error = {
        message: text.slice('/fake-fail'.length).trim() || 'fake failure',
        codexErrorInfo: null,
        additionalDetails: null,
      }
      notify('error', { threadId: t.id, turnId, error, willRetry: false })
    } else {
      if (text.startsWith('/fake-reroute ')) {
        const to = text.split(' ')[1]
        notify('model/rerouted', {
          threadId: t.id,
          turnId,
          fromModel: t.model,
          toModel: to,
          reason: 'highRiskCyberActivity',
        })
        t.model = to
      }
      agentMessage(
        t,
        turnId,
        `echo: ${text} [model=${t.active?.model ?? t.model} effort=${t.active?.effort ?? 'default'}]`,
      )
    }
  } catch (e) {
    status = 'failed'
    error = { message: String(e?.message ?? e), codexErrorInfo: null, additionalDetails: null }
  }
  const usage = { totalTokens: 30, inputTokens: 20, cachedInputTokens: 5, outputTokens: 10, reasoningOutputTokens: 0 }
  notify('thread/tokenUsage/updated', {
    threadId: t.id,
    turnId,
    tokenUsage: { total: usage, last: usage, modelContextWindow: 200000 },
  })
  t.active = null
  saveThread(t)
  notify('turn/completed', {
    threadId: t.id,
    turn: { id: turnId, status, error, items: [], durationMs: Date.now() - started },
  })
}

// A slow app-server start (requests wait in the stdin buffer meanwhile).
const startDelay = Number(process.env.FAKE_CODEX_START_DELAY_MS ?? 0)
if (startDelay > 0) await new Promise((r) => setTimeout(r, startDelay))
const rl = readline.createInterface({ input: process.stdin })
rl.on('line', (line) => {
  if (!line.trim()) return
  let msg
  try {
    msg = JSON.parse(line)
  } catch {
    return
  }
  if (msg.id === undefined) return // notifications (initialized) and responses
  if (!msg.method) return
  const handler = handlers[msg.method]
  if (!handler) {
    send({ id: msg.id, error: { code: -32601, message: `fake-codex: ${msg.method} not implemented` } })
    return
  }
  try {
    send({ id: msg.id, result: handler(msg.params) })
  } catch (e) {
    send({ id: msg.id, error: { code: e.code ?? -32603, message: e.message } })
  }
})
rl.on('close', () => process.exit(0))

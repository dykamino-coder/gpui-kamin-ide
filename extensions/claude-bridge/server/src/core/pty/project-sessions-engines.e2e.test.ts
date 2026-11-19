// D1 end-to-end: the project session tools for both engines through the real
// Bridge session WebSocket, the real MCP HTTP handler, fake Claude CLI and fake
// codex. Harness copied from session-codex.e2e.test.ts. Original header:
// End-to-end: a codex chat through the real Bridge session WebSocket, the real
// MCP HTTP handler (Bearer + tool-call pipeline to the client) and the real
// codex runtime, with test-support/fake-codex.mjs in place of the codex binary.
// Only identity (token lookup) and DuckDB are stubbed; HOME is a temp dir.
import 'reflect-metadata'
import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import type { AddressInfo } from 'node:net'
import { afterAll, afterEach, beforeAll, beforeEach, describe, expect, it, vi } from 'vitest'
import WebSocket from 'ws'

const state = vi.hoisted(() => {
  const nodeOs = require('node:os') as typeof import('node:os')
  const nodePath = require('node:path') as typeof import('node:path')
  const nodeFs = require('node:fs') as typeof import('node:fs')
  const home = nodeFs.mkdtempSync(nodePath.join(nodeOs.tmpdir(), 'codex-e2e-'))
  return {
    home,
    agentSettings: new Map<string, string>(),
    sessionTokens: [] as unknown[][],
  }
})

vi.mock('os', async (original) => {
  const real = await original<typeof import('os')>()
  return { ...real, default: { ...real, homedir: () => state.home }, homedir: () => state.home }
})
vi.mock('node:os', async (original) => {
  const real = await original<typeof import('node:os')>()
  return { ...real, default: { ...real, homedir: () => state.home }, homedir: () => state.home }
})
vi.mock('../auth/tokens', () => ({
  resolveToken: async (token: string) =>
    token === 'tok-a'
      ? { userName: 'alice', tokenId: 'tid-a' }
      : token === 'tok-b'
        ? { userName: 'bob', tokenId: 'tid-b' }
        : null,
}))
vi.mock('../stats/database/lifecycle', () => ({
  getDataDir: () => require('node:path').join(state.home, 'data'),
  getDb: async () => ({
    run: async (sql: string, params: unknown[] = []) => {
      if (sql.includes('INSERT INTO user_agent_settings')) state.agentSettings.set(String(params[0]), String(params[1]))
      if (sql.includes('INSERT INTO session_tokens')) state.sessionTokens.push(params)
    },
    runAndReadAll: async (sql: string, params: unknown[] = []) => ({
      getRowObjects: () =>
        sql.includes('user_agent_settings') && state.agentSettings.has(String(params[0]))
          ? [{ default_engine: state.agentSettings.get(String(params[0])) }]
          : [],
    }),
  }),
}))
vi.mock('../stats/database/write-lock', () => ({ withStatsWrite: async (fn: () => Promise<unknown>) => fn() }))

const FAKE_CODEX = fileURLToPath(new URL('../../../test-support/fake-codex.mjs', import.meta.url))

type Frame = Record<string, any> & { type: string }

class Client {
  readonly frames: Frame[] = []
  private waiters: Array<{ pred: (f: Frame) => boolean; resolve: (f: Frame) => void }> = []
  /** Answers every mcp:call like the KaminIDE host would. */
  mcpReply: (frame: Frame) => unknown = (f) => ({ content: [{ type: 'text', text: `host ran ${f.toolName}` }] })
  readonly mcpCalls: Frame[] = []

  private constructor(readonly ws: WebSocket) {
    ws.on('message', (raw) => {
      const frame = JSON.parse(String(raw)) as Frame
      this.frames.push(frame)
      if (frame.type === 'mcp:call') {
        this.mcpCalls.push(frame)
        this.send({
          type: 'mcp:response',
          requestId: frame.requestId,
          ...(frame.taskOwner ? { taskOwner: frame.taskOwner, sessionId: frame.sessionId } : {}),
          result: this.mcpReply(frame),
        })
      }
      for (const w of [...this.waiters]) {
        if (w.pred(frame)) {
          this.waiters.splice(this.waiters.indexOf(w), 1)
          w.resolve(frame)
        }
      }
    })
  }

  static async open(port: number): Promise<Client> {
    const ws = new WebSocket(`ws://127.0.0.1:${port}/ws/session`)
    await new Promise((resolve, reject) => ws.once('open', resolve).once('error', reject))
    return new Client(ws)
  }

  send(msg: Frame): void {
    this.ws.send(JSON.stringify(msg))
  }

  next(pred: (f: Frame) => boolean, timeoutMs = 10_000, from = 0): Promise<Frame> {
    const seen = this.frames.slice(from).find(pred)
    if (seen) return Promise.resolve(seen)
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => reject(new Error(`timeout waiting for frame; got ${this.types()}`)), timeoutMs)
      this.waiters.push({ pred, resolve: (f) => (clearTimeout(timer), resolve(f)) })
    })
  }

  types(): string {
    return this.frames.map((f) => f.type).join(',')
  }

  entries(): Array<Record<string, any>> {
    return this.frames.filter((f) => f.type === 'jsonl:entries').flatMap((f) => f.entries)
  }

  async entry(pred: (e: Record<string, any>) => boolean, timeoutMs = 10_000): Promise<Record<string, any>> {
    const deadline = Date.now() + timeoutMs
    for (;;) {
      const hit = this.entries().find(pred)
      if (hit) return hit
      if (Date.now() > deadline)
        throw new Error(`entry not seen; have ${JSON.stringify(this.entries()).slice(0, 2000)}`)
      await new Promise((r) => setTimeout(r, 25))
    }
  }

  close(): void {
    this.ws.close()
  }
}

const text = (e: Record<string, any>): string =>
  typeof e.message?.content === 'string'
    ? e.message.content
    : (e.message?.content ?? []).map((b: any) => b.text ?? b.thinking ?? JSON.stringify(b.content ?? '')).join('')

let port = 0
let server: import('node:http').Server
let runtimeMod: typeof import('../codex/codex-runtime')
let configMod: typeof import('../config')
let app: import('hono').Hono
// Scripted `claude auth status`: tests here never spawn the real Claude CLI.
let claudeMonitor: import('../engine/engine-auth').ClaudeAuthMonitor

function newRuntime(
  extraEnv: Record<string, string> = {},
  restartPolicy?: import('../codex/codex-host').RestartPolicy,
) {
  return new runtimeMod.CodexRuntime({
    restartPolicy,
    home: path.join(state.home, 'codex-home'),
    command: { command: process.execPath, args: [FAKE_CODEX] },
    env: { PATH: process.env.PATH ?? '', SystemRoot: process.env.SystemRoot ?? '' },
    extraEnv: { FAKE_CODEX_LOGGED_IN: '1', FAKE_CODEX_LOGIN_DELAY_MS: '50', ...extraEnv },
  })
}

async function createChat(client: Client, extra: Frame | Record<string, unknown> = {}) {
  client.send({ type: 'session:create', token: 'tok-a', protocolVersion: 1, cwd: '/work/project', ...extra })
  return client.next((f) => f.type === 'session:created' || f.type === 'session:error')
}

async function say(client: Client, message: string, submissionId: string) {
  const from = client.frames.length
  client.send({ type: 'session:submitText', data: message, submissionId })
  const receipt = await client.next((f) => f.type === 'session:submission' && f.id === submissionId, 10_000, from)
  return receipt
}

beforeAll(async () => {
  configMod = await import('../config')
  runtimeMod = await import('../codex/codex-runtime')
  runtimeMod.setCodexRuntimeForTests(newRuntime())
  const engineAuthMod = await import('../engine/engine-auth')
  const { eventBus } = await import('../events/bus')
  claudeMonitor = new engineAuthMod.ClaudeAuthMonitor({
    probe: async () => ({ loggedIn: false }),
    onChange: (loggedIn) => eventBus.emit('claude:account-updated', { engine: 'claude', loggedIn }),
  })
  claudeMonitor.record({ loggedIn: false })
  engineAuthMod.setClaudeAuthForTests(claudeMonitor)
  const { Hono } = await import('hono')
  const { serve } = await import('@hono/node-server')
  const { handleMcpRequest, handleMcpGet, handleMcpDelete } = await import('./mcp-http-handler')
  const { attachSessionWebSocket } = await import('./session-ws')
  const { installUpgradeHandler } = await import('../server/ws')
  const { registerCodexAccountRoutes } = await import('../dashboard/controllers/codex-account')
  app = new Hono()
  app.post('/mcp/:sessionId', handleMcpRequest)
  app.get('/mcp/:sessionId', handleMcpGet)
  app.delete('/mcp/:sessionId', handleMcpDelete)
  registerCodexAccountRoutes(app, () => runtimeMod.codexRuntime())
  // CLI lifecycle hooks (prompt readiness of the fake Claude), as in production.
  app.route('/', (await import('../hooks/routes')).createHooksRoutes())
  server = await new Promise((resolve) => {
    const s = serve({ fetch: app.fetch, port: 0, hostname: '127.0.0.1' }, () => resolve(s as never))
  })
  port = (server.address() as AddressInfo).port
  configMod.config.port = port
  attachSessionWebSocket(server as never)
  installUpgradeHandler(server as never)
}, 30_000)

afterAll(async () => {
  const { shutdownAll } = await import('./session-core')
  shutdownAll()
  await runtimeMod.shutdownCodexRuntime()
  await new Promise((r) => server.close(r))
  // Windows: a just-stopped CLI (fake Claude under ConPTY, the app-server) can
  // hold its directory past the retries; a leftover temp dir is not a failure.
  try {
    fs.rmSync(state.home, { recursive: true, force: true, maxRetries: 50, retryDelay: 100 })
  } catch (err) {
    if (process.platform !== 'win32') throw err
  }
}, 30_000)


// ── D1: chats of this folder and token, readable from both engines ─────────
describe('project session tools for Claude and ChatGPT chats', () => {
  beforeEach(() => {
    vi.stubEnv('NODE_ENV', 'test')
    vi.stubEnv('BRIDGE_DEV_FAKE_CLI', '1')
    vi.stubEnv('BRIDGE_FAKE_CLI_HOME', state.home)
    claudeMonitor.record({ loggedIn: true })
  })
  afterEach(() => {
    vi.unstubAllEnvs()
    claudeMonitor.record({ loggedIn: false })
  })

  async function chat(engine: 'claude' | 'codex', opts: { token?: string; cwd?: string } = {}) {
    const client = await Client.open(port)
    client.send({ type: 'session:create', token: opts.token ?? 'tok-a', protocolVersion: 1, cwd: opts.cwd ?? '/work/project', engine })
    const created = await client.next((f) => f.type === 'session:created' || f.type === 'session:error')
    expect(created.type).toBe('session:created')
    return client
  }
  async function conversationOf(client: Client): Promise<string> {
    return (await client.next((f) => f.type === 'session:conversation-id', 15_000)).conversationId
  }
  async function sessionOf(conversationId: string) {
    const { getAllSessions } = await import('./session-manager')
    const s = getAllSessions().find((x) => x.cliConversationId === conversationId || (x.conversationAliases ?? []).includes(conversationId))
    if (!s?.mcpEndpoint) throw new Error(`no session for ${conversationId}`)
    return s
  }
  async function call(conversationId: string, tool: string, args: Record<string, unknown> = {}) {
    const s = await sessionOf(conversationId)
    const res = await fetch(`http://127.0.0.1:${port}/mcp/${s.mcpEndpoint!.id}`, {
      method: 'POST',
      headers: { 'content-type': 'application/json', accept: 'application/json, text/event-stream', authorization: `Bearer ${s.mcpEndpoint!.token}` },
      body: JSON.stringify({ jsonrpc: '2.0', id: 1, method: 'tools/call', params: { name: tool, arguments: args } }),
    })
    const raw = await res.text()
    const data = JSON.parse(raw.includes('data: ') ? raw.split('data: ').pop()! : raw)
    const out = (data.result?.content ?? []).map((c: { text?: string }) => c.text ?? '').join('\n')
    return { isError: !!data.result?.isError, text: out }
  }
  const dump = (...a: unknown[]) => fs.appendFileSync(process.env.K_D1_DUMP ?? path.join(state.home, 'd1.log'), a.map(String).join(' ') + '\n')
  const listOf = (t: string) => JSON.parse(t.slice(t.indexOf('{'), t.lastIndexOf('}') + 1)) as { conversations: Array<Record<string, any>> }

  const footerOf = (t: string) => JSON.parse(t.slice(t.lastIndexOf('\n{') + 1)) as { results: number; transcripts: number }
  const hitIds = (t: string) => [...t.matchAll(/^([0-9a-f-]{36}) #\d+/gm)].map((m) => m[1])

  it('both engines see the same chats of their token and folder, a mixed chat once', async () => {
    const c = await chat('claude')
    await say(c, '/fake-note 4217', 'c1')
    await c.entry((e) => e.type === 'assistant' && text(e) === 'noted: 4217', 15_000)
    const cId = await conversationOf(c)
    const x = await chat('codex')
    await say(x, 'codex fact 5591', 'x1')
    await x.entry((e) => text(e).startsWith('echo: codex fact 5591'), 15_000)
    const xId = await conversationOf(x)
    const y = await chat('codex', { token: 'tok-b' })
    await say(y, 'foreign token 7777', 'y1')
    await y.entry((e) => text(e).startsWith('echo: foreign token 7777'), 15_000)
    const yId = await conversationOf(y)
    const z = await chat('codex', { cwd: '/work/other' })
    await say(z, 'other folder 8888', 'z1')
    await z.entry((e) => text(e).startsWith('echo: other folder 8888'), 15_000)
    const zId = await conversationOf(z)

    for (const [caller, self] of [
      [xId, xId],
      [cId, cId],
    ] as const) {
      const list = listOf((await call(caller, 'project_sessions_list')).text)
      const byId = new Map(list.conversations.map((r) => [r.session_id, r]))
      expect([...byId.keys()].sort()).toEqual([cId, xId].sort())
      expect(byId.get(cId)).toMatchObject({ engine: 'claude' })
      expect(byId.get(xId)).toMatchObject({ engine: 'codex', title: 'codex fact 5591' })
      expect(byId.get(self)!.self).toBe(true)
    }
    // Claude reads the ChatGPT chat and back.
    const fromCodex = await call(xId, 'project_session_search', { query: '4217' })
    expect(hitIds(fromCodex.text)).toEqual([cId])
    const fromClaude = await call(cId, 'project_session_search', { query: '5591' })
    expect(new Set(hitIds(fromClaude.text))).toEqual(new Set([xId]))
    const read = await call(cId, 'project_session_read', { session_id: xId })
    expect(read.text).toContain('codex fact 5591')
    expect(read.text).toMatch(/assistant engine=codex/)
    // Other tokens and other folders stay invisible, by search and by id.
    for (const q of ['7777', '8888']) expect(footerOf((await call(xId, 'project_session_search', { query: q })).text).results).toBe(0)
    for (const id of [yId, zId]) {
      const r = await call(cId, 'project_session_read', { session_id: id })
      expect(r.isError).toBe(true)
      expect(r.text).toContain('Conversation not found in this project')
    }

    // Mixed chat: Claude, then ChatGPT in the same chat.
    const m = await chat('claude')
    await say(m, '/fake-note 9931', 'm1')
    await m.entry((e) => e.type === 'assistant' && text(e) === 'noted: 9931', 15_000)
    const mFirst = await conversationOf(m)
    const from = m.frames.length
    m.send({ type: 'session:switch-engine', engine: 'codex', model: 'gpt-fake' })
    await m.next((f) => f.type === 'session:restarted', 15_000, from)
    const mId = (await m.next((f) => f.type === 'session:conversation-id' && f.conversationId !== mFirst, 15_000, from))
      .conversationId
    await say(m, 'mixed gpt 3141', 'm2')
    await m.entry((e) => text(e).startsWith('echo: mixed gpt 3141'), 15_000)

    const list = listOf((await call(xId, 'project_sessions_list')).text)
    expect(list.conversations.map((r) => r.session_id).sort()).toEqual([cId, xId, mId].sort())
    const mixed = list.conversations.find((r) => r.session_id === mId)!
    expect(mixed).toMatchObject({ engine: 'codex', model: 'gpt-fake' })
    expect(mixed.segments.map((s: { engine: string }) => s.engine)).toEqual(['claude', 'codex'])
    expect(mixed.segments[1].model).toBe('gpt-fake')
    for (const r of list.conversations) expect(String(r.title)).not.toContain('bridge-context')
    // One fact, one hit: segment transcripts are not searched beside the chat.
    const fact = await call(xId, 'project_session_search', { query: '9931' })
    expect(hitIds(fact.text)).toEqual([mId])
    const after = await call(cId, 'project_session_search', { query: '3141' })
    expect(new Set(hitIds(after.text))).toEqual(new Set([mId]))
    expect(after.text).not.toContain('bridge-context')
    expect(footerOf((await call(cId, 'project_session_search', { query: 'Bridge note' })).text).results).toBe(0)
    // The chat's earlier (Claude) id still opens the whole conversation.
    const byOldId = await call(cId, 'project_session_read', { session_id: mFirst })
    expect(byOldId.text).toContain('noted: 9931')
    expect(byOldId.text).toContain('mixed gpt 3141')
    expect(byOldId.text).not.toContain('bridge-context')

    // Resume by a known id from another token is refused for both engines.
    for (const id of [cId, xId]) {
      const b = await Client.open(port)
      b.send({ type: 'session:resume', token: 'tok-b', protocolVersion: 1, cwd: '/work/project', conversationId: id, noSpawnOnNotFound: true })
      const reply = await b.next((f) => ['session:resume-not-found', 'session:created', 'session:error'].includes(f.type), 15_000)
      expect(reply.type).toBe('session:resume-not-found')
      b.close()
    }
    for (const cl of [c, x, y, z, m]) {
      cl.send({ type: 'session:end' })
      cl.close()
    }
  }, 90_000)
})
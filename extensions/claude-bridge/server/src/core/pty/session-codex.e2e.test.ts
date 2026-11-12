// End-to-end: a codex chat through the real Bridge session WebSocket, the real
// MCP HTTP handler (Bearer + tool-call pipeline to the client) and the real
// codex runtime, with test-support/fake-codex.mjs in place of the codex binary.
// Only identity (token lookup) and DuckDB are stubbed; HOME is a temp dir.
import 'reflect-metadata'
import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import type { AddressInfo } from 'node:net'
import { afterAll, beforeAll, describe, expect, it, vi } from 'vitest'
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

function newRuntime(extraEnv: Record<string, string> = {}) {
  return new runtimeMod.CodexRuntime({
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
  fs.rmSync(state.home, { recursive: true, force: true })
}, 30_000)

describe('codex chat over the Bridge session WebSocket', () => {
  let threadId = ''

  it('creates a codex chat, streams the answer and writes the normalized transcript', async () => {
    const client = await Client.open(port)
    const created = await createChat(client, { engine: 'codex' })
    expect(created).toMatchObject({
      type: 'session:created',
      engine: 'codex',
      model: 'gpt-fake',
    })
    expect(await client.next((f) => f.type === 'agent:preferences')).toMatchObject({ defaultEngine: 'claude' })

    expect(await say(client, 'hello codex', 'sub-1')).toMatchObject({ state: 'read' })
    const answer = await client.entry((e) => e.type === 'assistant' && text(e).startsWith('echo: hello codex'))
    expect(answer).toMatchObject({ engine: 'codex', message: { model: 'gpt-fake' } })
    expect(text(answer)).toBe('echo: hello codex [model=gpt-fake effort=default]')
    const user = await client.entry((e) => e.type === 'user' && text(e) === 'hello codex')
    expect(answer.parentUuid).toBe(user.uuid)

    const conv = await client.next((f) => f.type === 'session:conversation-id')
    threadId = conv.conversationId
    const stub = await client.next((f) => f.type === 'streaming:entry')
    expect(stub.entry).toMatchObject({ __streaming: true, message: { id: answer.message.id } })
    expect(client.frames.some((f) => f.type === 'streaming:delta' && f.msgId === answer.message.id)).toBe(true)
    const activity = client.frames.filter((f) => f.type === 'session:activity').map((f) => f.isWorking)
    expect(activity).toContain(true)
    expect(activity.at(-1)).toBe(false)

    // The transcript lives where the watcher (and resume lookup) expects it.
    const slugDirs = fs.readdirSync(path.join(state.home, '.claude', 'projects'))
    const file = slugDirs
      .map((d) => path.join(state.home, '.claude', 'projects', d, `${threadId}.jsonl`))
      .find((f) => fs.existsSync(f))!
    const lines = fs
      .readFileSync(file, 'utf8')
      .trim()
      .split('\n')
      .map((l) => JSON.parse(l))
    expect(lines.every((l) => l.engine === 'codex' && l.sessionId === threadId)).toBe(true)
    await vi.waitFor(() => expect(state.sessionTokens.some((p) => p[0] === threadId && p[6] === 'codex')).toBe(true))

    // The lockdown reached the shared app-server.
    const overrides = JSON.parse(
      fs.readFileSync(path.join(state.home, 'codex-home', 'fake-state', 'last-overrides.json'), 'utf8'),
    ) as string[]
    expect(overrides).toContain('features.shell_tool=false')
    expect(overrides).toContain('web_search="live"')
    client.send({ type: 'session:end' })
    client.close()
  }, 30_000)

  it('runs Bridge MCP tools through the real handler, per-session Bearer and host round trip', async () => {
    const a = await Client.open(port)
    const b = await Client.open(port)
    await createChat(a, { engine: 'codex' })
    b.send({ type: 'session:create', token: 'tok-b', engine: 'codex', protocolVersion: 1 })
    await b.next((f) => f.type === 'session:created')
    a.mcpReply = () => ({ content: [{ type: 'text', text: 'body of /work/project/a.txt' }] })
    b.mcpReply = () => ({ content: [{ type: 'text', text: 'B only' }] })

    await Promise.all([
      say(a, '/fake-mcp Read {"file_path":"/work/project/a.txt"}', 'mcp-a'),
      say(b, '/fake-mcp Read {"file_path":"/b.txt"}', 'mcp-b'),
    ])
    const result = await a.entry(
      (e) => e.type === 'user' && Array.isArray(e.message?.content) && e.message.content[0]?.type === 'tool_result',
    )
    expect(JSON.stringify(result)).toContain('body of /work/project/a.txt')
    const call = await a.entry((e) => e.type === 'assistant' && e.message?.content?.[0]?.type === 'tool_use')
    expect(call.message.content[0]).toMatchObject({
      name: 'mcp__bridge__Read',
      input: { file_path: '/work/project/a.txt' },
    })
    await b.entry((e) => JSON.stringify(e).includes('B only'))

    // Each chat's tool call reached only its own client.
    expect(a.mcpCalls.map((f) => f.input)).toEqual([{ file_path: '/work/project/a.txt' }])
    expect(b.mcpCalls.map((f) => f.input)).toEqual([{ file_path: '/b.txt' }])
    expect(JSON.stringify(a.entries())).not.toContain('B only')
    for (const c of [a, b]) {
      c.send({ type: 'session:end' })
      c.close()
    }
  }, 30_000)

  it('switches model and effort for the next turn, interrupts, and explains failures', async () => {
    const client = await Client.open(port)
    await createChat(client, { engine: 'codex' })
    client.send({ type: 'session:change-model', model: 'gpt-fake-mini' })
    expect(await client.next((f) => f.type === 'session:model-changed')).toMatchObject({
      model: 'gpt-fake-mini',
      inPlace: true,
    })
    client.send({ type: 'session:change-effort', effort: 'max' })
    await client.next((f) => f.type === 'session:model-changed' && f.effort === 'max')
    await say(client, 'after switch', 's1')
    await client.entry((e) => text(e) === 'echo: after switch [model=gpt-fake-mini effort=xhigh]')

    // A Claude id cannot move a codex chat to another engine.
    const before = client.frames.length
    client.send({ type: 'session:change-model', model: 'claude-opus-5-5' })
    expect(await client.next((f) => f.type === 'session:model-changed', 5_000, before)).toMatchObject({
      model: 'gpt-fake-mini',
    })

    await say(client, '/fake-slow', 's2')
    await client.next((f) => f.type === 'streaming:entry' && f.entry.message.content[0].text.length > 0)
    client.send({ type: 'session:interrupt' })
    await client.entry((e) => e.type === 'system' && e.interruptedByUser === true)

    await say(client, '/fake-fail stream error: unexpected status 407 Proxy Authentication Required', 's3')
    const failure = await client.entry((e) => e.type === 'system' && e.level === 'error')
    expect(failure.content).toContain('corporate proxy requires authentication')
    client.send({ type: 'session:end' })
    client.close()
  }, 30_000)

  it('gates the chat until an administrator signs in with the device code', async () => {
    const runtime = runtimeMod.codexRuntime()
    await runtime.logout()
    const client = await Client.open(port)
    await createChat(client, { engine: 'codex' })
    const receipt = await say(client, 'are you there?', 'gate-1')
    expect(receipt).toMatchObject({ state: 'uncertain' })
    expect(receipt.warning).toContain('not signed in')
    expect(await client.next((f) => f.type === 'codex:auth')).toMatchObject({ loggedIn: false })

    const status0 = await (await app.request('/api/dashboard/codex/account?force=true')).json()
    expect(status0).toMatchObject({ loggedIn: false, pendingLogin: null })
    const login = await (await app.request('/api/dashboard/codex/login', { method: 'POST' })).json()
    expect(login).toMatchObject({ verificationUrl: 'https://auth.example.test/codex/device', userCode: 'FAKE-1234' })
    await vi.waitFor(
      async () => {
        const s = await (await app.request('/api/dashboard/codex/account')).json()
        expect(s).toMatchObject({ loggedIn: true, email: 'fake@example.test', pendingLogin: null })
      },
      { timeout: 5_000, interval: 50 },
    )
    expect(await say(client, 'now?', 'gate-2')).toMatchObject({ state: 'read' })
    await client.entry((e) => text(e).startsWith('echo: now?'))
    client.send({ type: 'session:end' })
    client.close()
  }, 30_000)

  it('resumes a codex chat after the app-server and the session are gone', async () => {
    const client = await Client.open(port)
    await createChat(client, { engine: 'codex' })
    await say(client, 'remember 42', 'r1')
    const first = await client.entry((e) => text(e).startsWith('echo: remember 42'))
    const conv = (await client.next((f) => f.type === 'session:conversation-id')).conversationId
    client.send({ type: 'session:end' })
    client.close()
    // Process restart: the shared app-server goes away, a new runtime starts it again.
    await runtimeMod.codexRuntime().close()
    runtimeMod.setCodexRuntimeForTests(newRuntime())

    const again = await Client.open(port)
    // No engine hint: the transcript decides.
    again.send({
      type: 'session:resume',
      token: 'tok-a',
      conversationId: conv,
      protocolVersion: 1,
      cwd: '/work/project',
    })
    expect(await again.next((f) => f.type === 'session:created')).toMatchObject({ engine: 'codex' })
    await again.entry((e) => e.uuid === first.uuid) // history replayed
    await say(again, 'still there?', 'r2')
    const next = await again.entry((e) => text(e).startsWith('echo: still there?'))
    expect(next.sessionId).toBe(conv)
    const user = await again.entry((e) => e.type === 'user' && text(e) === 'still there?')
    expect(user.parentUuid).toBe(first.uuid) // one chain across the restart
    again.send({ type: 'session:end' })
    again.close()
  }, 30_000)

  it('keeps the per-user main agent and serves the GPT model list', async () => {
    const client = await Client.open(port)
    client.send({ type: 'codex:models', token: 'tok-a' })
    const models = await client.next((f) => f.type === 'codex:models')
    expect(models.models.map((m: { id: string }) => m.id)).toEqual(['gpt-fake', 'gpt-fake-mini'])
    expect(models.account).toMatchObject({ loggedIn: true })

    client.send({ type: 'agent:set-default-engine', engine: 'codex' })
    expect(await client.next((f) => f.type === 'agent:preferences' && 'error' in f)).toMatchObject({
      error: expect.stringContaining('Not authenticated'),
    })

    // (A Claude chat would spawn the real CLI; any authenticated socket will do.)
    expect(await createChat(client, { engine: 'codex' })).toMatchObject({ engine: 'codex' })
    const from = client.frames.length
    client.send({ type: 'agent:set-default-engine', engine: 'codex' })
    expect(
      await client.next((f) => f.type === 'agent:preferences' && f.defaultEngine === 'codex', 5_000, from),
    ).toBeTruthy()
    client.send({ type: 'session:end' })
    client.close()

    // The next connection learns the main agent and opens the chat with it.
    const later = await Client.open(port)
    expect(await createChat(later, { engine: 'codex' })).toMatchObject({ engine: 'codex' })
    expect(await later.next((f) => f.type === 'agent:preferences')).toMatchObject({ defaultEngine: 'codex' })
    later.send({ type: 'session:end' })
    later.close()
  }, 30_000)

  it('without a codex binary marks ChatGPT unavailable and refuses the chat with the reason', async () => {
    const working = runtimeMod.codexRuntime()
    runtimeMod.setCodexRuntimeForTests(
      new runtimeMod.CodexRuntime({
        home: path.join(state.home, 'codex-home-missing'),
        command: { command: path.join(state.home, 'no-such-codex'), args: [] },
        env: { PATH: process.env.PATH ?? '' },
      }),
    )
    try {
      const client = await Client.open(port)
      client.send({ type: 'codex:models', token: 'tok-a' })
      expect(await client.next((f) => f.type === 'codex:models')).toMatchObject({
        models: [],
        unavailable: 'codex is not installed on this Bridge server',
      })
      expect(await createChat(client, { engine: 'codex' })).toMatchObject({
        type: 'session:error',
        error: expect.stringContaining('ChatGPT is unavailable: codex is not installed on this Bridge server'),
      })
      client.close()
    } finally {
      runtimeMod.setCodexRuntimeForTests(working)
    }
  }, 30_000)

  it('reports the codex version Bridge runs and the image pin on the status page', async () => {
    const runtime = runtimeMod.codexRuntime()
    expect(await runtimeMod.codexStatus(() => runtime, { BRIDGE_CODEX_VERSION: '0.160.1' })).toEqual({
      available: true,
      version: '0.160.1-fake',
      reason: null,
      pinned: '0.160.1',
    })
  }, 30_000)
})

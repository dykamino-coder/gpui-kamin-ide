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
    expect(text(answer)).toBe('echo: hello codex [model=gpt-fake effort=medium]')
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

  it('feeds Bridge Console a live log of the thread and replays it to a reattached client', async () => {
    const consoleText = (c: Client) =>
      c.frames
        .filter((f) => f.type === 'session:output')
        .map((f) => String(f.data))
        .join('')
    const client = await Client.open(port)
    expect(await createChat(client, { engine: 'codex' })).toMatchObject({ engine: 'codex' })
    // First frame: without any session:output the console stays on "Connecting to session…".
    await client.next((f) => f.type === 'session:output' && String(f.data).includes('ChatGPT (codex)'))
    expect(consoleText(client)).toContain('model gpt-fake')

    await say(client, 'hello console', 'sub-console')
    await client.entry((e) => e.type === 'assistant' && text(e).startsWith('echo: hello console'))
    await vi.waitFor(() => expect(consoleText(client)).toContain('turn completed'), { timeout: 5_000 })
    const log = consoleText(client)
    expect(log).toContain('hello console')
    expect(log).toContain('echo: hello console')
    expect(log).not.toMatch(/[^\r]\n/)

    // A second window on the same live chat gets the console back at once.
    const conv = await client.next((f) => f.type === 'session:conversation-id')
    const again = await Client.open(port)
    again.send({ type: 'session:resume', token: 'tok-a', protocolVersion: 1, conversationId: conv.conversationId })
    const replay = await again.next((f) => f.type === 'session:output')
    // Byte-exact: the reply streamed mid-line, and the replay must not split it.
    expect(replay.data).toBe(consoleText(client))
    again.send({ type: 'session:end' })
    again.close()
    client.close()
  }, 30_000)

  it('tells a reattached client that the turn which ended meanwhile is over', async () => {
    const client = await Client.open(port)
    await createChat(client, { engine: 'codex' })
    await say(client, 'turn while away', 'sub-away')
    await client.entry((e) => e.type === 'assistant' && text(e).startsWith('echo: turn while away'))
    const conv = await client.next((f) => f.type === 'session:conversation-id')
    // The window that saw `working` is gone; the edge back to idle went to it only.
    const again = await Client.open(port)
    again.send({ type: 'session:resume', token: 'tok-a', protocolVersion: 1, conversationId: conv.conversationId })
    const state = await again.next((f) => f.type === 'session:activity')
    expect(state).toMatchObject({ isWorking: false, promptReady: true, hookDriven: true })
    again.send({ type: 'session:end' })
    again.close()
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

  it('/compact (typed or the Compact button) compacts the codex thread instead of sending a model turn', async () => {
    const client = await Client.open(port)
    await createChat(client, { engine: 'codex' })
    await say(client, 'first message', 'k1')
    await client.entry((e) => text(e).startsWith('echo: first message'), 15_000)
    const from = client.frames.length
    // What the Compact button sends: `/compact <hand-off instructions>`.
    const receipt = await say(client, '/compact Summarise HANDOFF-K9 for a fresh agent', 'k2')
    expect(receipt).toMatchObject({ state: 'read' })
    const boundary = await client.entry(
      (e) =>
        e.subtype === 'compact_boundary' &&
        client.frames.findIndex((f) => f.type === 'jsonl:entries' && f.entries.includes(e)) >= from,
      15_000,
    )
    expect(boundary).toMatchObject({ type: 'system', content: 'Conversation compacted' })
    // The thread was never told the command as text, and nothing answered it.
    const mark = client.frames.length
    await say(client, '/fake-recall HANDOFF-K9', 'k3')
    const recall = await client.entry(
      (e) =>
        e.type === 'assistant' &&
        text(e).startsWith('recall:') &&
        client.frames.findIndex((f) => f.type === 'jsonl:entries' && f.entries.includes(e)) >= mark,
      15_000,
    )
    expect(text(recall)).toBe('recall: nothing about HANDOFF-K9')
    const echoed = client.frames
      .slice(from)
      .flatMap((f) => (f.type === 'jsonl:entries' ? f.entries : []))
      .some((e: Record<string, any>) => text(e).includes('echo: /compact'))
    expect(echoed).toBe(false)
    client.send({ type: 'session:end' })
    client.close()
  })

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
    const switched = await client.entry((e) => text(e) === 'echo: after switch [model=gpt-fake-mini effort=xhigh]')
    // The reply is labelled with the model that produced it, not the chat's first one.
    expect(switched.message.model).toBe('gpt-fake-mini')

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

  it('lets an idle ChatGPT chat sleep instead of ending it, and wakes it with the next message', async () => {
    const { sweepIdleSessions } = await import('./session-hibernation')
    const client = await Client.open(port)
    await createChat(client, { engine: 'codex', basePrompt: 'Standing rule BP-SLEEP-3301: answer tersely.' })
    await say(client, 'before sleep', 'z1')
    const first = await client.entry((e) => text(e).startsWith('echo: before sleep'))
    const conv = (await client.next((f) => f.type === 'session:conversation-id')).conversationId

    // An hour later: the sweep puts the chat to sleep; the client is told it
    // sleeps, not that the session ended (which made it resume at once).
    const mark = client.frames.length
    const config = { idleMs: 15 * 60_000, maxAwake: 0, wakeConcurrency: 2, evictIdleMs: 60_000 }
    expect(sweepIdleSessions(Date.now() + 60 * 60_000, config)).toBeGreaterThanOrEqual(1)
    await client.next((f) => f.type === 'session:activity' && f.hibernation === 'asleep', 15_000, mark)

    await say(client, '/fake-recall BP-SLEEP-3301', 'z2')
    const recall = await client.entry(
      (e) =>
        e.type === 'assistant' &&
        text(e).startsWith('recall:') &&
        client.frames.findIndex((f) => f.type === 'jsonl:entries' && f.entries.includes(e)) >= mark,
      15_000,
    )
    // Same thread, the chat's standing instructions still there.
    expect(recall.sessionId).toBe(conv)
    expect(text(recall)).toContain('Standing rule BP-SLEEP-3301')
    const after = client.frames.slice(mark)
    expect(after.some((f) => f.type === 'session:activity' && f.hibernation === 'awake')).toBe(true)
    expect(after.some((f) => f.type === 'session:exit')).toBe(false)
    const user = await client.entry((e) => e.type === 'user' && text(e) === '/fake-recall BP-SLEEP-3301')
    expect(user.parentUuid).toBe(first.uuid)
    client.send({ type: 'session:end' })
    client.close()
  }, 30_000)

  it('keeps a ChatGPT chat awake while its turn runs', async () => {
    const { sweepIdleSessions } = await import('./session-hibernation')
    const client = await Client.open(port)
    await createChat(client, { engine: 'codex' })
    await say(client, '/fake-slow', 'w1')
    await client.next((f) => f.type === 'session:activity' && f.isWorking === true, 15_000)
    const mark = client.frames.length
    const config = { idleMs: 15 * 60_000, maxAwake: 0, wakeConcurrency: 2, evictIdleMs: 60_000 }
    sweepIdleSessions(Date.now() + 60 * 60_000, config)
    await new Promise((r) => setTimeout(r, 300))
    expect(client.frames.slice(mark).some((f) => f.type === 'session:activity' && f.hibernation === 'asleep')).toBe(
      false,
    )
    client.send({ type: 'session:end' })
    client.close()
  }, 30_000)

  it('resumes a codex chat on the model and effort chosen after its last turn', async () => {
    const client = await Client.open(port)
    await createChat(client, { engine: 'codex' })
    await say(client, 'first turn', 'k1')
    await client.entry((e) => text(e).startsWith('echo: first turn [model=gpt-fake effort=medium]'))
    const conv = (await client.next((f) => f.type === 'session:conversation-id')).conversationId
    // Chosen in the chat, no turn afterwards: codex's rollout does not know it yet.
    client.send({ type: 'session:change-model', model: 'gpt-fake-mini' })
    await client.next((f) => f.type === 'session:model-changed' && f.model === 'gpt-fake-mini')
    client.send({ type: 'session:change-effort', effort: 'low' })
    await client.next((f) => f.type === 'session:model-changed' && f.effort === 'low')
    client.send({ type: 'session:end' })
    client.close()
    await runtimeMod.codexRuntime().close()
    runtimeMod.setCodexRuntimeForTests(newRuntime())

    const again = await Client.open(port)
    again.send({
      type: 'session:resume',
      token: 'tok-a',
      conversationId: conv,
      protocolVersion: 1,
      cwd: '/work/project',
    })
    expect(await again.next((f) => f.type === 'session:created')).toMatchObject({
      engine: 'codex',
      model: 'gpt-fake-mini',
      effort: 'low',
    })
    await say(again, 'after restart', 'k2')
    await again.entry((e) => text(e) === 'echo: after restart [model=gpt-fake-mini effort=low]')
    again.send({ type: 'session:end' })
    again.close()
  }, 30_000)

  it('brings a base prompt saved after the first turn and the exact model to the resumed thread, once', async () => {
    // Owner's case: the thread ran turns before the prompt was saved; after a
    // restart codex resumes it, and resume instructions never reach its context.
    // Answers already given (a resumed chat replays them) are not the new one.
    const answered = new Set<string>()
    const recall = async (c: Client, word: string, id: string) => {
      await say(c, `/fake-recall ${word}`, id)
      const hit = await c.entry((e) => e.type === 'assistant' && text(e).startsWith('recall:') && !answered.has(e.uuid))
      answered.add(hit.uuid)
      return text(hit)
    }
    const client = await Client.open(port)
    await createChat(client, { engine: 'codex' })
    expect(await recall(client, 'BP-RES-7310', 'p1')).toBe('recall: nothing about BP-RES-7310')
    // The model line arrives with the first turn (the start named no model).
    expect(await recall(client, 'runs on the model', 'p1m')).toContain('`gpt-fake`')
    const conv = (await client.next((f) => f.type === 'session:conversation-id')).conversationId
    client.send({ type: 'session:end' })
    client.close()
    await runtimeMod.codexRuntime().close()
    runtimeMod.setCodexRuntimeForTests(newRuntime())

    const again = await Client.open(port)
    again.send({
      type: 'session:resume',
      token: 'tok-a',
      conversationId: conv,
      protocolVersion: 1,
      cwd: '/work/project',
      basePrompt: 'Saved later BP-RES-7310: reply in Russian.',
    })
    expect(await again.next((f) => f.type === 'session:created')).toMatchObject({ engine: 'codex' })
    const seen = await recall(again, 'BP-RES-7310', 'p2')
    expect(seen.split('Saved later BP-RES-7310')).toHaveLength(2)
    // The chat shows the message, not the context sent in front of it.
    const bubble = await again.entry((e) => e.type === 'user' && text(e).includes('/fake-recall BP-RES-7310'))
    expect(text(bubble)).toBe('/fake-recall BP-RES-7310')
    // Delivered once: the next turns do not repeat it.
    expect((await recall(again, 'BP-RES-7310', 'p3')).split('Saved later BP-RES-7310')).toHaveLength(2)

    // A model and effort switch updates the model line once, without the prompt again.
    again.send({ type: 'session:change-model', model: 'gpt-fake-mini' })
    await again.next((f) => f.type === 'session:model-changed' && f.model === 'gpt-fake-mini')
    again.send({ type: 'session:change-effort', effort: 'low' })
    await again.next((f) => f.type === 'session:model-changed' && f.effort === 'low')
    const lines = await recall(again, 'runs on the model', 'p4')
    expect(lines).toContain('This chat runs on the model `gpt-fake-mini` with reasoning effort `low`.')
    expect(lines.split('`gpt-fake-mini`')).toHaveLength(2)
    expect((await recall(again, 'BP-RES-7310', 'p5')).split('Saved later BP-RES-7310')).toHaveLength(2)
    expect((await recall(again, 'gpt-fake-mini', 'p6')).split('runs on the model `gpt-fake-mini`')).toHaveLength(2)
    again.send({ type: 'session:end' })
    again.close()
  }, 40_000)

  it('continues a chat that never ran a turn on a new thread after a restart, with its choice', async () => {
    // codex 0.160.1 writes a rollout only with the first turn (acceptance
    // 2026-11-12: such a chat came back as "Conversation not found").
    await runtimeMod.codexRuntime().close()
    runtimeMod.setCodexRuntimeForTests(newRuntime({ FAKE_CODEX_ROLLOUT_ON_TURN: '1' }))
    const { codexTranscriptPath } = await import('./session-codex')
    const client = await Client.open(port)
    const created = await createChat(client, { engine: 'codex' })
    client.send({ type: 'session:change-model', model: 'gpt-fake-mini' })
    await client.next((f) => f.type === 'session:model-changed' && f.model === 'gpt-fake-mini')
    client.send({ type: 'session:change-effort', effort: 'low' })
    await client.next((f) => f.type === 'session:model-changed' && f.effort === 'low')
    const settingsDir = created.settingsDir as string
    const choice = fs.readdirSync(settingsDir).find((f) => f.startsWith('codex-choice-'))!
    const threadId = choice.slice('codex-choice-'.length, -'.json'.length)
    // The chat's transcript holds only a notice (e.g. a blocked first message).
    const transcript = codexTranscriptPath(settingsDir, threadId)
    fs.mkdirSync(path.dirname(transcript), { recursive: true })
    const notice = { type: 'system', uuid: 'n1', parentUuid: null, timestamp: new Date().toISOString() }
    fs.writeFileSync(
      transcript,
      JSON.stringify({ ...notice, sessionId: threadId, engine: 'codex', content: 'x' }) + '\n',
    )
    client.send({ type: 'session:end' })
    client.close()
    await runtimeMod.codexRuntime().close()
    runtimeMod.setCodexRuntimeForTests(newRuntime({ FAKE_CODEX_ROLLOUT_ON_TURN: '1' }))

    const again = await Client.open(port)
    again.send({
      type: 'session:resume',
      noSpawnOnNotFound: true,
      token: 'tok-a',
      conversationId: threadId,
      protocolVersion: 1,
      cwd: '/work/project',
    })
    const resumed = await again.next(
      (f) => f.type === 'session:created' || f.type === 'session:resume-not-found' || f.type === 'session:error',
    )
    expect(resumed).toMatchObject({ type: 'session:created', engine: 'codex', model: 'gpt-fake-mini', effort: 'low' })
    expect(resumed.resumeNotFound).toBeUndefined()
    // Nothing to replay; the new thread's id reaches the client with its transcript.
    await again.next((f) => f.type === 'jsonl:status' && f.replayComplete === true)
    await say(again, 'first turn', 'k1')
    await again.entry((e) => text(e) === 'echo: first turn [model=gpt-fake-mini effort=low]')
    const next = (await again.next((f) => f.type === 'session:conversation-id')).conversationId
    expect(next).not.toBe(threadId)
    again.send({ type: 'session:end' })
    again.close()
    await runtimeMod.codexRuntime().close()
    runtimeMod.setCodexRuntimeForTests(newRuntime())
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

// The selector disables an engine the server is not signed in to; sign-in and
// sign-out reach open sockets live.
describe('engine sign-in in the model selector', () => {
  const prefs = (client: Client, pred: (f: Frame) => boolean, from: number) =>
    client.next((f) => f.type === 'agent:preferences' && pred(f), 10_000, from)

  it('opens the chat on the signed-in engine, refuses a signed-out main agent and pushes sign-in changes', async () => {
    // Earlier cases swap the runtime; the dashboard routes announce sign-ins
    // of the runtime they were registered with (one per server in production).
    const { Hono } = await import('hono')
    const { registerCodexAccountRoutes } = await import('../dashboard/controllers/codex-account')
    const dashboard = new Hono()
    registerCodexAccountRoutes(dashboard, () => runtimeMod.codexRuntime())
    claudeMonitor.record({ loggedIn: false })
    const client = await Client.open(port)
    // Main agent Claude, Claude signed out, ChatGPT signed in → a ChatGPT chat, with the reason.
    const created = await createChat(client, { engine: 'claude', token: 'tok-b' })
    expect(created).toMatchObject({ type: 'session:created', engine: 'codex' })
    expect(created.engineNotice).toContain(
      'Claude is not signed in on this Bridge server. This chat runs ChatGPT instead',
    )
    expect(await prefs(client, () => true, 0)).toMatchObject({
      defaultEngine: 'claude',
      auth: { claude: { loggedIn: false, reason: 'Claude is not signed in on this Bridge server.' } },
    })

    let from = client.frames.length
    client.send({ type: 'agent:set-default-engine', engine: 'codex' })
    expect(await prefs(client, (f) => f.defaultEngine === 'codex', from)).not.toHaveProperty('error')
    from = client.frames.length
    client.send({ type: 'agent:set-default-engine', engine: 'claude' })
    expect(await prefs(client, (f) => 'error' in f, from)).toMatchObject({
      error: 'Claude is not signed in on this Bridge server.',
      defaultEngine: 'codex',
    })

    // `claude login` on the server: the selector learns it without a reconnect.
    from = client.frames.length
    claudeMonitor.record({ loggedIn: true })
    expect(await prefs(client, (f) => f.auth?.claude?.loggedIn === true, from)).toMatchObject({
      auth: { claude: { loggedIn: true, reason: null }, codex: { loggedIn: true } },
    })

    // ChatGPT sign-out from the dashboard, then nothing signed in at all.
    from = client.frames.length
    await dashboard.request('/api/dashboard/codex/logout', { method: 'POST' })
    expect(await prefs(client, (f) => f.auth?.codex?.loggedIn === false, from)).toMatchObject({
      auth: { claude: { loggedIn: true }, codex: { reason: 'ChatGPT is not signed in on this Bridge server.' } },
    })
    from = client.frames.length
    claudeMonitor.record({ loggedIn: false })
    expect(await prefs(client, (f) => f.auth?.claude?.loggedIn === false, from)).toMatchObject({
      auth: { claude: { loggedIn: false }, codex: { loggedIn: false } },
    })

    // The administrator signs ChatGPT in again: pushed as well.
    from = client.frames.length
    await dashboard.request('/api/dashboard/codex/login', { method: 'POST' })
    expect(await prefs(client, (f) => f.auth?.codex?.loggedIn === true, from)).toMatchObject({
      auth: { claude: { loggedIn: false }, codex: { loggedIn: true, reason: null } },
    })
    client.send({ type: 'session:end' })
    client.close()
  }, 30_000)
})

// Field lessons of the Codex agent driver (same app-server protocol), end to end.
describe('codex chat survives the app-server and reports a stalled turn', () => {
  const notice = (needle: string) => (e: Record<string, any>) =>
    e.type === 'system' && typeof e.content === 'string' && e.content.includes(needle)

  // Each case gets a fresh crash history; the storm case keeps the defaults.
  async function freshRuntime(policy?: import('../codex/codex-host').RestartPolicy) {
    await runtimeMod.codexRuntime().close()
    runtimeMod.setCodexRuntimeForTests(newRuntime({}, policy))
  }
  beforeEach(() => freshRuntime({ stormCrashes: 10, maxCrashes: 10 }))

  it('keeps the app-server after a completed turn, and resumes the thread on a restarted one after a crash', async () => {
    const client = await Client.open(port)
    await createChat(client, { engine: 'codex' })
    const runtime = runtimeMod.codexRuntime()
    const host = await runtime.host()
    await say(client, 'first', 'c1')
    await client.entry((e) => text(e).startsWith('echo: first'))
    // A finished turn is not a finished process: nothing restarts.
    await new Promise((r) => setTimeout(r, 100))
    expect(await runtime.host()).toBe(host)
    expect(host.alive).toBe(true)

    await say(client, '/fake-crash', 'c2')
    await client.entry(notice('Restarting it and resuming this conversation'))
    await client.entry(notice('Resumed the conversation on a restarted ChatGPT app-server'))
    const restarted = await runtime.host()
    expect(restarted).not.toBe(host)
    expect(host.crashed).toBe(true)

    // Same Bridge session, same thread, new app-server.
    expect(await say(client, 'after crash', 'c3')).toMatchObject({ state: 'read' })
    await client.entry((e) => text(e).startsWith('echo: after crash'))
    expect(client.frames.some((f) => f.type === 'session:exit')).toBe(false)
    client.send({ type: 'session:end' })
    client.close()
  }, 30_000)

  it('queues a message sent during the restart and delivers it to the resumed thread', async () => {
    // Every app-server start takes 800 ms, so the restart is still running below.
    await runtimeMod.codexRuntime().close()
    runtimeMod.setCodexRuntimeForTests(newRuntime({ FAKE_CODEX_START_DELAY_MS: '800' }, { stormCrashes: 10 }))
    const client = await Client.open(port)
    await createChat(client, { engine: 'codex' })
    await say(client, '/fake-crash', 'q1')
    await client.entry(notice('Restarting it and resuming this conversation'))
    // Sent during the restart: waits for the resume instead of failing.
    client.send({ type: 'session:submitText', data: 'queued', submissionId: 'q2' })
    expect(await client.next((f) => f.type === 'session:submission' && f.id === 'q2', 15_000)).toMatchObject({
      state: 'read',
    })
    await client.entry((e) => text(e).startsWith('echo: queued'))
    client.send({ type: 'session:end' })
    client.close()
  }, 30_000)

  it('reports a turn that goes silent and clears the report when it ends', async () => {
    process.env.BRIDGE_CODEX_STALL_MS = '200'
    try {
      const client = await Client.open(port)
      await createChat(client, { engine: 'codex' })
      await say(client, '/fake-hang', 'h1')
      const stalled = await client.entry(notice('No response from ChatGPT'))
      expect(stalled.level).toBe('warning')
      client.send({ type: 'session:interrupt' })
      await client.entry((e) => e.type === 'system' && e.interruptedByUser === true)
      // A normal turn afterwards is not reported.
      const before = client.entries().filter(notice('No response from ChatGPT')).length
      await say(client, 'quick', 'h2')
      await client.entry((e) => text(e).startsWith('echo: quick'))
      await new Promise((r) => setTimeout(r, 400))
      expect(client.entries().filter(notice('No response from ChatGPT')).length).toBe(before)
      client.send({ type: 'session:end' })
      client.close()
    } finally {
      delete process.env.BRIDGE_CODEX_STALL_MS
    }
  }, 30_000)

  it('stops restarting after a crash storm and ends the chat with the reason', async () => {
    await freshRuntime() // default policy: two crashes right after start = storm
    const client = await Client.open(port)
    await createChat(client, { engine: 'codex' })
    await say(client, '/fake-crash', 's1')
    await client.entry(notice('Resumed the conversation'))
    await say(client, '/fake-crash', 's2')
    const failed = await client.entry(notice('ChatGPT could not be restarted'))
    expect(failed.content).toContain('restarts are paused')
    expect(await client.next((f) => f.type === 'session:exit')).toBeTruthy()
    client.close()
  }, 30_000)

  it('refuses a second session on a thread that is already open', async () => {
    const a = await Client.open(port)
    await createChat(a, { engine: 'codex' })
    await say(a, 'owner', 'd1')
    await a.entry((e) => text(e).startsWith('echo: owner'))
    const conv = (await a.next((f) => f.type === 'session:conversation-id')).conversationId
    const runtime = runtimeMod.codexRuntime()
    const { CodexAgentSession } = await import('../codex/codex-session')
    await expect(
      CodexAgentSession.start({ host: await runtime.host(), cwd: state.home, resumeThreadId: conv }),
    ).rejects.toThrow('already open in another Bridge session')
    a.send({ type: 'session:end' })
    a.close()
  }, 30_000)
})

// One chat, one conversation across engines (conversation/manifest.ts): the
// tab keeps its conversation id, the joining engine starts with the history.
describe('engine switch inside one chat', () => {
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

  async function switchTo(client: Client, engine: string, model: string) {
    const from = client.frames.length
    const t0 = performance.now()
    client.send({ type: 'session:switch-engine', engine, model })
    const restarted = await client.next(
      (f) => f.type === 'session:restarted' || f.type === 'session:engine-switch-failed',
      15_000,
      from,
    )
    return { restarted, from, ms: performance.now() - t0 }
  }
  const after = (client: Client, from: number) => (pred: (e: Record<string, any>) => boolean) =>
    client.entry(
      (e) => pred(e) && client.frames.findIndex((f) => f.type === 'jsonl:entries' && f.entries.includes(e)) >= from,
      15_000,
    )
  /** A new answer: the replay after a switch repeats earlier ones after `from`. */
  const answerAfter = (client: Client, from: number, prefix: string) => {
    const old = new Set(
      client.frames
        .slice(0, from)
        .filter((f) => f.type === 'jsonl:entries')
        .flatMap((f) => f.entries.map((e: Record<string, any>) => e.uuid)),
    )
    return after(client, from)((e) => e.type === 'assistant' && text(e).startsWith(prefix) && !old.has(e.uuid))
  }

  it('carries the conversation Claude → ChatGPT → Claude in the same chat and resumes it', async () => {
    const client = await Client.open(port)
    expect(await createChat(client, { engine: 'claude' })).toMatchObject({ type: 'session:created', engine: 'claude' })
    await say(client, '/fake-note 4217', 'n1')
    await client.entry((e) => e.type === 'assistant' && text(e) === 'noted: 4217', 15_000)
    const first = (await client.next((f) => f.type === 'session:conversation-id')).conversationId

    // Claude → ChatGPT: same socket, same chat; the conversation gets its own id.
    const toGpt = await switchTo(client, 'codex', 'gpt-fake')
    expect(toGpt.restarted).toMatchObject({ type: 'session:restarted', engine: 'codex', model: 'gpt-fake' })
    expect(toGpt.restarted.engineSwitch.prepareMs).toBeLessThan(200)
    expect(client.frames.slice(toGpt.from).some((f) => f.type === 'jsonl:status' && f.compacted)).toBe(true)
    const id = (
      await client.next((f) => f.type === 'session:conversation-id' && f.conversationId !== first, 15_000, toGpt.from)
    ).conversationId
    const since = after(client, toGpt.from)
    await since((e) => e.type === 'assistant' && text(e) === 'noted: 4217') // history replayed into the chat
    await since((e) => e.subtype === 'engine_switch' && e.content === '→ gpt-fake')
    let mark = client.frames.length
    await say(client, '/fake-recall 4217', 'g1')
    const gptRecall = await after(client, mark)((e) => e.type === 'assistant' && text(e).startsWith('recall:'))
    expect(text(gptRecall)).toContain('noted: 4217')
    mark = client.frames.length
    await say(client, 'my dog is called Sharik', 'g2')
    await after(client, mark)((e) => text(e).startsWith('echo: my dog is called Sharik'))

    // ChatGPT → Claude: the new Claude session knows both segments.
    const toClaude = await switchTo(client, 'claude', 'claude-sonnet-5-5')
    expect(toClaude.restarted).toMatchObject({ engine: 'claude', model: 'claude-sonnet-5-5' })
    const back = after(client, toClaude.from)
    await back((e) => e.subtype === 'engine_switch' && e.content === '→ claude-sonnet-5-5')
    expect((await client.next((f) => f.type === 'session:conversation-id', 15_000, toClaude.from)).conversationId).toBe(
      id,
    )
    mark = client.frames.length
    await say(client, '/fake-recall Sharik', 'c2')
    const claudeRecall = await after(client, mark)((e) => e.type === 'assistant' && text(e).startsWith('recall:'))
    expect(text(claudeRecall)).toContain('my dog is called Sharik')
    mark = client.frames.length
    await say(client, '/fake-recall 4217', 'c3')
    expect(text(await after(client, mark)((e) => e.type === 'assistant' && text(e).startsWith('recall:')))).toContain(
      'noted: 4217',
    )
    console.info('engine switch timings', {
      toGpt: toGpt.restarted.engineSwitch,
      toGptMs: Math.round(toGpt.ms),
      toClaude: toClaude.restarted.engineSwitch,
      toClaudeMs: Math.round(toClaude.ms),
    })

    // One transcript: every segment on one parent chain, two switch markers.
    const { SESSIONS_BASE } = await import('./session-settings')
    const { unifiedLogPath } = await import('../conversation/conversation')
    const dir = path.join(
      SESSIONS_BASE,
      'tid-a-' + (await import('./session-settings')).sanitizeDirName('/work/project'),
      id,
    )
    const log = fs
      .readFileSync(unifiedLogPath({ id, dir }), 'utf8')
      .trim()
      .split('\n')
      .map((l) => JSON.parse(l))
    expect(log.filter((e) => e.subtype === 'engine_switch')).toHaveLength(2)
    const byUuid = new Map(log.map((e) => [e.uuid, e]))
    const chain: any[] = []
    for (let e = log.filter((x) => !x.isSidechain).at(-1); e; e = byUuid.get(e.parentUuid)) chain.push(e)
    const chained = chain.map(text)
    for (const needle of ['noted: 4217', 'echo: my dog is called Sharik [model=gpt-fake effort=high]'])
      expect(chained).toContain(needle)
    expect(chain.at(-1)).toMatchObject({ sessionId: first }) // back into the chat's first segment

    client.send({ type: 'session:end' })
    client.close()

    // Reopening the chat resumes the conversation on its current engine.
    const again = await Client.open(port)
    again.send({ type: 'session:resume', token: 'tok-a', conversationId: id, protocolVersion: 1, cwd: '/work/project' })
    expect(await again.next((f) => f.type === 'session:created' || f.type === 'session:error', 15_000)).toMatchObject({
      type: 'session:created',
      engine: 'claude',
    })
    await again.entry((e) => e.type === 'assistant' && text(e) === 'noted: 4217', 15_000)
    await again.entry((e) => text(e).startsWith('echo: my dog is called Sharik'), 15_000)
    again.send({ type: 'session:end' })
    again.close()
  }, 60_000)

  it('REAL119: receipts after a switch carry the user text, the engine gets the history, both ways', async () => {
    const client = await Client.open(port)
    const receipts = (id: string) => client.frames.filter((f) => f.type === 'session:submission' && f.id === id)
    const settled = (id: string, from: number) =>
      client.next(
        (f) => f.type === 'session:submission' && f.id === id && ['read', 'fallback', 'uncertain'].includes(f.state),
        15_000,
        from,
      )
    expect(await createChat(client, { engine: 'claude' })).toMatchObject({ engine: 'claude' })
    await say(client, '/fake-note 5307', 'r1')
    await client.entry((e) => e.type === 'assistant' && text(e) === 'noted: 5307', 15_000)

    // Claude → ChatGPT: the first turn carries the hidden history.
    await switchTo(client, 'codex', 'gpt-fake')
    let mark = client.frames.length
    client.send({ type: 'session:submitText', data: 'first on gpt', submissionId: 'g1' })
    expect(await settled('g1', mark)).toMatchObject({ state: 'read', text: 'first on gpt' })
    await after(client, mark)((e) => text(e).startsWith('echo: first on gpt'))
    mark = client.frames.length
    await say(client, '/fake-recall 5307', 'g2')
    expect(text(await answerAfter(client, mark, 'recall:'))).toContain('noted: 5307')

    // ChatGPT → Claude: same contract on the terminal/mod path.
    await switchTo(client, 'claude', 'claude-sonnet-5-5')
    mark = client.frames.length
    client.send({ type: 'session:submitText', data: 'first on claude', submissionId: 'c1' })
    await settled('c1', mark)
    mark = client.frames.length
    await say(client, '/fake-recall 5307', 'c2')
    expect(text(await answerAfter(client, mark, 'recall:'))).toContain('noted: 5307')
    for (const [id, typed] of [
      ['g1', 'first on gpt'],
      ['g2', '/fake-recall 5307'],
      ['c1', 'first on claude'],
      ['c2', '/fake-recall 5307'],
    ] as const) {
      expect(receipts(id).length).toBeGreaterThan(0)
      for (const r of receipts(id)) expect(r.text).toBe(typed)
    }
    client.send({ type: 'session:end' })
    client.close()
  }, 60_000)

  it('REAL119: the model gets its exact id and effort: new chat, effort change, both switches, resume', async () => {
    const client = await Client.open(port)
    // What the engine was actually told (instructions, inputs), not a helper's output.
    const told = async (word: string, id: string) => {
      const mark = client.frames.length
      await say(client, `/fake-recall ${word}`, id)
      const answer = await answerAfter(client, mark, 'recall:')
      return text(answer)
        .slice('recall: '.length)
        .split(' | ')
        .filter((line) => !line.startsWith('/fake-recall'))
    }
    // A new ChatGPT chat that picked neither model nor effort: the catalog default.
    await createChat(client, { engine: 'codex' })
    expect(await told('chat runs', 'm1')).toEqual([
      'This chat runs on the model `gpt-fake` with reasoning effort `medium`.',
    ])
    client.send({ type: 'session:change-effort', effort: 'high' })
    await client.next((f) => f.type === 'session:model-changed' && f.effort === 'high')
    expect((await told('chat runs', 'm2')).at(-1)).toBe(
      'This chat runs on the model `gpt-fake` with reasoning effort `high`.',
    )
    // ChatGPT → Claude: the Claude CLI is told its effort (it names its id itself).
    await switchTo(client, 'claude', 'claude-sonnet-5-5')
    expect(await told('reasoning effort', 'm3')).toContain(
      'This chat was started with reasoning effort `high`. A later `/effort` command, or a Bridge note in a user message that names a new reasoning effort, sets the current level.',
    )
    // Claude → ChatGPT: the joining engine keeps the chat's effort.
    // Its instructions come first; the history block after them quotes the old line.
    await switchTo(client, 'codex', 'gpt-fake-mini')
    expect((await told('chat runs', 'm4'))[0]).toBe(
      'This chat runs on the model `gpt-fake-mini` with reasoning effort `high`.',
    )
    const id = client.frames.filter((f) => f.type === 'session:conversation-id').at(-1)!.conversationId
    client.send({ type: 'session:end' })
    client.close()
    // Reopened: the resumed thread keeps the line it was given, no second copy.
    const again = await Client.open(port)
    again.send({ type: 'session:resume', token: 'tok-a', conversationId: id, protocolVersion: 1, cwd: '/work/project' })
    await again.next((f) => f.type === 'session:created', 15_000)
    const mark = again.frames.length
    again.send({ type: 'session:submitText', data: '/fake-recall chat runs', submissionId: 'm5' })
    // The reopened chat replays the earlier answers: only a new one counts.
    const earlier = new Set(client.entries().map((e) => e.uuid))
    const answer = await again.entry(
      (e) =>
        e.type === 'assistant' &&
        text(e).startsWith('recall:') &&
        !earlier.has(e.uuid) &&
        again.frames.findIndex((f) => f.type === 'jsonl:entries' && f.entries.includes(e)) >= mark,
      15_000,
    )
    expect(text(answer)).toContain('This chat runs on the model `gpt-fake-mini` with reasoning effort `high`.')
    again.send({ type: 'session:end' })
    again.close()
  }, 60_000)

  it('REAL120: the effort the chat shows is the one turn/start names and the model line states', async () => {
    const rpcFile = path.join(state.home, 'codex-home', 'fake-state', 'rpc.jsonl')
    const rpcCount = () =>
      fs.existsSync(rpcFile) ? fs.readFileSync(rpcFile, 'utf8').split('\n').filter(Boolean).length : 0
    const rpcSince = (from: number) =>
      fs
        .readFileSync(rpcFile, 'utf8')
        .split('\n')
        .filter(Boolean)
        .slice(from)
        .map(
          (line) =>
            JSON.parse(line) as {
              method: string
              model: string | null
              effort: string | null
              instructions: string | null
            },
        )
    let client = await Client.open(port)
    // One turn: what the engine ran it with (its echo), and what turn/start carried.
    const turn = async (message: string, id: string) => {
      const mark = client.frames.length
      const rpc = rpcCount()
      await say(client, message, id)
      const answer = text(await answerAfter(client, mark, `echo: ${message}`))
      const starts = rpcSince(rpc).filter((r) => r.method === 'turn/start')
      return { ran: /effort=(\w+)\]/.exec(answer)?.[1], sent: starts.map((r) => r.effort) }
    }
    const line = async (id: string) => {
      const mark = client.frames.length
      await say(client, '/fake-recall chat runs', id)
      return text(await answerAfter(client, mark, 'recall:'))
        .split(' | ')
        .filter((l) => l.startsWith('This chat runs'))
        .at(-1)
    }

    // (a) A new ChatGPT chat shows the effort its engine resolves (thread/start
    // gets that line); High picked in the selector reaches the first turn.
    let rpc = rpcCount()
    expect(await createChat(client, { engine: 'codex' })).toMatchObject({ effort: 'medium' })
    const start = rpcSince(rpc).find((r) => r.method === 'thread/start')!
    expect(start).toMatchObject({ effort: null })
    expect(start.instructions).toContain('This chat runs on the model `gpt-fake` with reasoning effort `medium`.')
    expect(await turn('a0', 'a0')).toEqual({ ran: 'medium', sent: ['medium'] })
    client.send({ type: 'session:change-effort', effort: 'high' })
    await client.next((f) => f.type === 'session:model-changed' && f.effort === 'high')
    expect(await turn('a1', 'a1')).toEqual({ ran: 'high', sent: ['high'] })
    expect(await line('a2')).toBe('This chat runs on the model `gpt-fake` with reasoning effort `high`.')
    client.send({ type: 'session:end' })
    client.close()

    // (b) Claude on High → ChatGPT: the chat stays on High in the UI and in the engine.
    client = await Client.open(port)
    expect(await createChat(client, { engine: 'claude' })).toMatchObject({ effort: 'high' })
    rpc = rpcCount()
    const { restarted } = await switchTo(client, 'codex', 'gpt-fake')
    expect(restarted).toMatchObject({ type: 'session:restarted', engine: 'codex', effort: 'high' })
    expect(rpcSince(rpc).find((r) => r.method === 'thread/start')!.instructions).toContain(
      'This chat runs on the model `gpt-fake` with reasoning effort `high`.',
    )
    expect(await turn('b1', 'b1')).toEqual({ ran: 'high', sent: ['high'] })

    // (c) The selector mid-chat: the next turn and the line follow it, both ways.
    client.send({ type: 'session:change-effort', effort: 'low' })
    expect(await client.next((f) => f.type === 'session:model-changed')).toMatchObject({ effort: 'low', inPlace: true })
    expect(await turn('c1', 'c1')).toEqual({ ran: 'low', sent: ['low'] })
    expect(await line('c2')).toBe('This chat runs on the model `gpt-fake` with reasoning effort `low`.')
    const back = client.frames.length
    client.send({ type: 'session:change-effort', effort: 'high' })
    expect(await client.next((f) => f.type === 'session:model-changed', 10_000, back)).toMatchObject({ effort: 'high' })
    expect(await turn('c3', 'c3')).toEqual({ ran: 'high', sent: ['high'] })
    expect(await line('c4')).toBe('This chat runs on the model `gpt-fake` with reasoning effort `high`.')

    // Reopened (thread/resume): the chat still shows and runs High.
    const conversationId = client.frames.filter((f) => f.type === 'session:conversation-id').at(-1)!.conversationId
    client.send({ type: 'session:end' })
    client.close()
    client = await Client.open(port)
    rpc = rpcCount()
    client.send({ type: 'session:resume', token: 'tok-a', conversationId, protocolVersion: 1, cwd: '/work/project' })
    expect(await client.next((f) => f.type === 'session:created', 15_000)).toMatchObject({ effort: 'high' })
    expect(rpcSince(rpc).some((r) => r.method === 'thread/resume')).toBe(true)
    expect(await turn('r1', 'r1')).toEqual({ ran: 'high', sent: ['high'] })

    // (d) ChatGPT → Claude keeps it too.
    const toClaude = await switchTo(client, 'claude', 'claude-sonnet-5-5')
    expect(toClaude.restarted).toMatchObject({ engine: 'claude', effort: 'high' })
    client.send({ type: 'session:end' })
    client.close()
  }, 90_000)
  it("keeps the chat's standing instructions across Claude → ChatGPT → Claude, exactly once", async () => {
    const client = await Client.open(port)
    const basePrompt = 'Standing rule BP-MARK-5521: answer tersely.'
    await createChat(client, { engine: 'claude', basePrompt })
    await say(client, '/fake-note 9', 'b1')
    await client.entry((e) => e.type === 'assistant' && text(e) === 'noted: 9', 15_000)

    const toGpt = await switchTo(client, 'codex', 'gpt-fake')
    expect(toGpt.restarted).toMatchObject({ type: 'session:restarted', engine: 'codex' })
    let mark = client.frames.length
    await say(client, '/fake-recall BP-MARK-5521', 'b2')
    const recall = text(await after(client, mark)((e) => e.type === 'assistant' && text(e).startsWith('recall:')))
    // The developer instructions of the codex thread carry the prompt, once.
    expect(recall).toContain('Standing rule BP-MARK-5521')
    expect(recall.split('BP-MARK-5521')).toHaveLength(2)

    // Back on Claude and on ChatGPT again: still there (the new segment keeps it too).
    expect((await switchTo(client, 'claude', 'claude-sonnet-5-5')).restarted).toMatchObject({ engine: 'claude' })
    expect((await switchTo(client, 'codex', 'gpt-fake')).restarted).toMatchObject({ engine: 'codex' })
    mark = client.frames.length
    await say(client, '/fake-recall BP-MARK-5521', 'b3')
    const again = text(await after(client, mark)((e) => e.type === 'assistant' && text(e).startsWith('recall:')))
    expect(again.split('Standing rule BP-MARK-5521')).toHaveLength(2)

    // Another chat without a prompt does not get this one.
    const other = await Client.open(port)
    await createChat(other, { engine: 'codex' })
    const from = other.frames.length
    await say(other, '/fake-recall BP-MARK-5521', 'o1')
    const none = await other.entry(
      (e) =>
        e.type === 'assistant' &&
        text(e).startsWith('recall:') &&
        other.frames.findIndex((f) => f.type === 'jsonl:entries' && f.entries.includes(e)) >= from,
      15_000,
    )
    expect(text(none)).toBe('recall: nothing about BP-MARK-5521')
    for (const c of [client, other]) {
      c.send({ type: 'session:end' })
      c.close()
    }
  }, 60_000)

  // Each chain recalls only at its end: an earlier recall answer would put the
  // marker into the carried history and hide a lost prompt.
  const chains: Array<[string, string[]]> = [
    ['ChatGPT → Claude', ['codex', 'claude']],
    ['ChatGPT → Claude → ChatGPT', ['codex', 'claude', 'codex']],
    ['Claude → ChatGPT → Claude', ['claude', 'codex', 'claude']],
  ]
  const modelOf = (engine: string) => (engine === 'codex' ? 'gpt-fake' : 'claude-sonnet-5-5')
  it.each(chains)(
    "keeps the chat's standing instructions on %s",
    async (_name, [start, ...hops]) => {
      const client = await Client.open(port)
      const marker = `BP-HOP-${start}-${hops.length}`
      await createChat(client, { engine: start, basePrompt: `Standing rule ${marker}: answer tersely.` })
      await say(client, '/fake-note 7', 'h0')
      await client.entry((e) => e.type === 'assistant' && text(e).includes('7'), 15_000)
      for (const engine of hops) {
        expect((await switchTo(client, engine, modelOf(engine))).restarted).toMatchObject({
          type: 'session:restarted',
          engine,
        })
      }
      const mark = client.frames.length
      await say(client, `/fake-recall ${marker}`, 'h1')
      const recall = text(await after(client, mark)((e) => e.type === 'assistant' && text(e).startsWith('recall:')))
      expect(recall).toContain(`Standing rule ${marker}`)
      client.send({ type: 'session:end' })
      client.close()
    },
    60_000,
  )

  it('restarts an empty chat on the other engine, holds a prompt sent meanwhile and refuses the same engine', async () => {
    const client = await Client.open(port)
    await createChat(client, { engine: 'codex' })
    expect((await switchTo(client, 'codex', 'gpt-fake-mini')).restarted).toMatchObject({
      type: 'session:engine-switch-failed',
    })
    // A prompt sent right after the pick waits for the new engine.
    const from = client.frames.length
    client.send({ type: 'session:switch-engine', engine: 'claude', model: 'claude-sonnet-5-5' })
    await say(client, '/fake-note 77', 'e1')
    expect(await client.next((f) => f.type === 'session:restarted', 15_000, from)).toMatchObject({ engine: 'claude' })
    await after(client, from)((e) => e.type === 'assistant' && text(e) === 'noted: 77')
    expect(client.frames.slice(from).some((f) => f.type === 'session:error')).toBe(false)
    client.send({ type: 'session:end' })
    client.close()
  }, 30_000)

  it('keeps the chat on the leaving engine when the joining one fails to start (B1)', async () => {
    const client = await Client.open(port)
    await createChat(client, { engine: 'claude' })
    await say(client, '/fake-note 4217', 'f1')
    await client.entry((e) => e.type === 'assistant' && text(e) === 'noted: 4217', 15_000)
    const first = (await client.next((f) => f.type === 'session:conversation-id')).conversationId
    const working = runtimeMod.codexRuntime()
    runtimeMod.setCodexRuntimeForTests(newRuntime({ FAKE_CODEX_THREAD_START_FAIL: 'thread start refused' }))
    try {
      const failed = await switchTo(client, 'codex', 'gpt-fake')
      expect(failed.restarted).toMatchObject({ type: 'session:engine-switch-failed' })
      expect(failed.restarted.error).toContain('thread start refused')
      expect(client.frames.slice(failed.from).some((f) => f.type === 'session:restarted')).toBe(false)
    } finally {
      await runtimeMod.codexRuntime().close()
      runtimeMod.setCodexRuntimeForTests(working)
    }
    // The Claude session still runs the chat, with its conversation as it was.
    const mark = client.frames.length
    await say(client, '/fake-recall 4217', 'f2')
    expect(text(await after(client, mark)((e) => e.type === 'assistant' && text(e).startsWith('recall:')))).toContain(
      'noted: 4217',
    )
    expect(
      client.frames.filter((f) => f.type === 'session:conversation-id').every((f) => f.conversationId === first),
    ).toBe(true)
    const { SESSIONS_BASE, sanitizeDirName } = await import('./session-settings')
    const sessionsDir = path.join(SESSIONS_BASE, 'tid-a-' + sanitizeDirName('/work/project'))
    const { readManifest } = await import('../conversation/manifest')
    // No conversation was made of this chat: the failed switch committed nothing.
    const made = fs
      .readdirSync(sessionsDir)
      .map((d) => readManifest(path.join(sessionsDir, d)))
      .filter((m) => m?.segments[0]?.nativeId === first)
    expect(made).toHaveLength(0)
    // And a later switch works.
    expect((await switchTo(client, 'codex', 'gpt-fake')).restarted).toMatchObject({ type: 'session:restarted' })
    client.send({ type: 'session:end' })
    client.close()
  }, 60_000)

  it('a second switch sent during the first waits and is refused; one conversation id (S3)', async () => {
    const client = await Client.open(port)
    await createChat(client, { engine: 'claude' })
    await say(client, '/fake-note 31', 'd1')
    await client.entry((e) => e.type === 'assistant' && text(e) === 'noted: 31', 15_000)
    const from = client.frames.length
    client.send({ type: 'session:switch-engine', engine: 'codex', model: 'gpt-fake' })
    client.send({ type: 'session:switch-engine', engine: 'codex', model: 'gpt-fake' })
    // A prompt sent in the same window reaches the joining engine.
    client.send({ type: 'session:submitText', data: '/fake-recall 31', submissionId: 'd2' })
    expect(await client.next((f) => f.type === 'session:restarted', 15_000, from)).toMatchObject({ engine: 'codex' })
    expect(await client.next((f) => f.type === 'session:engine-switch-failed', 15_000, from)).toMatchObject({
      error: 'This chat already runs on that engine',
    })
    const recall = await after(client, from)((e) => e.type === 'assistant' && text(e).startsWith('recall:'))
    expect(text(recall)).toContain('noted: 31')
    expect(client.frames.slice(from).filter((f) => f.type === 'session:restarted')).toHaveLength(1)
    await new Promise((r) => setTimeout(r, 300))
    const ids = new Set(
      client.frames
        .slice(from)
        .filter((f) => f.type === 'session:conversation-id')
        .map((f) => f.conversationId),
    )
    expect(ids.size).toBe(1)
    client.send({ type: 'session:end' })
    client.close()
  }, 30_000)

  it('carries a history far over 128 KiB in the first turn, within the byte budget (B2)', async () => {
    const client = await Client.open(port)
    await createChat(client, { engine: 'codex' })
    for (let i = 0; i < 3; i++) {
      const mark = client.frames.length
      await say(client, `bulk ${i} ${'ж'.repeat(30_000)}`, `b${i}`)
      await after(client, mark)((e) => text(e).startsWith(`echo: bulk ${i}`))
    }
    let mark = client.frames.length
    await say(client, 'the newest fact is OKAPI-3', 'b9')
    await after(client, mark)((e) => text(e).startsWith('echo: the newest fact is OKAPI-3'))

    const toClaude = await switchTo(client, 'claude', 'claude-sonnet-5-5')
    expect(toClaude.restarted).toMatchObject({ type: 'session:restarted', engine: 'claude' })
    const { HANDOFF_BUDGET_BYTES } = await import('../engine/handoff')
    // The bulk turns do not fit whole: the newest turns verbatim, the end of the
    // last bulk turn, the earlier requests listed.
    expect(toClaude.restarted.engineSwitch.contextBytes).toBeGreaterThan(30_000)
    expect(toClaude.restarted.engineSwitch.contextBytes).toBeLessThanOrEqual(HANDOFF_BUDGET_BYTES)
    const id = (await client.next((f) => f.type === 'session:conversation-id', 15_000, toClaude.from)).conversationId
    const { SESSIONS_BASE, sanitizeDirName } = await import('./session-settings')
    const { unifiedLogPath } = await import('../conversation/conversation')
    const dir = path.join(SESSIONS_BASE, 'tid-a-' + sanitizeDirName('/work/project'), id)
    expect(fs.statSync(unifiedLogPath({ id, dir })).size).toBeGreaterThan(128 * 1024)

    mark = client.frames.length
    await say(client, '/fake-recall OKAPI-3', 'b10')
    expect(text(await after(client, mark)((e) => e.type === 'assistant' && text(e).startsWith('recall:')))).toContain(
      'OKAPI-3',
    )
    client.send({ type: 'session:end' })
    client.close()
  }, 90_000)

  it('a segment whose codex thread is gone continues on a new one with the whole conversation (S6)', async () => {
    const client = await Client.open(port)
    await createChat(client, { engine: 'claude' })
    await say(client, '/fake-note 9001', 'l1')
    await client.entry((e) => e.type === 'assistant' && text(e) === 'noted: 9001', 15_000)
    const first = (await client.next((f) => f.type === 'session:conversation-id')).conversationId
    const toGpt = await switchTo(client, 'codex', 'gpt-fake')
    const id = (
      await client.next((f) => f.type === 'session:conversation-id' && f.conversationId !== first, 15_000, toGpt.from)
    ).conversationId
    let mark = client.frames.length
    await say(client, 'hello codex', 'l2') // takes the first-turn history
    await after(client, mark)((e) => text(e).startsWith('echo: hello codex'))
    client.send({ type: 'session:end' })
    client.close()

    // The app-server goes and its threads with it.
    await runtimeMod.codexRuntime().close()
    fs.rmSync(path.join(state.home, 'codex-home', 'fake-state', 'threads'), { recursive: true, force: true })
    runtimeMod.setCodexRuntimeForTests(newRuntime())

    const again = await Client.open(port)
    again.send({ type: 'session:resume', token: 'tok-a', conversationId: id, protocolVersion: 1, cwd: '/work/project' })
    expect(await again.next((f) => f.type === 'session:created' || f.type === 'session:error', 15_000)).toMatchObject({
      type: 'session:created',
      engine: 'codex',
    })
    mark = again.frames.length
    await say(again, '/fake-recall 9001', 'l3')
    expect(text(await after(again, mark)((e) => e.type === 'assistant' && text(e).startsWith('recall:')))).toContain(
      'noted: 9001',
    )
    again.send({ type: 'session:end' })
    again.close()
  }, 60_000)
})

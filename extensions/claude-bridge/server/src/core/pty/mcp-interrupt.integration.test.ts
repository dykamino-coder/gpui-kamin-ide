// INC-2026-0096: a user interrupt must stop the command on the client host.
// Real HTTP handler, real pending-call and task bookkeeping, and the client's
// real foreground shell executor behind a synthetic WS; no CLI, no host profile.
import { afterAll, afterEach, beforeAll, describe, expect, it, vi } from 'vitest'
import { promises as fs } from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { Hono } from 'hono'
import type { PtySession } from './types'
import { MODERN_MCP, TASK_EXTENSION } from './mcp-modern'

type Frame = {
  type: string
  requestId: string
  taskOwner?: string
  input?: { command?: string }
}
type RunForeground = (
  command: string,
  shell: string,
  timeout: number,
  shellFlags: string[],
  env: NodeJS.ProcessEnv,
  signal: AbortSignal,
  onOutput: (chunk: string) => void,
) => Promise<unknown>

const mock = vi.hoisted(() => ({
  home: '',
  sessions: [] as PtySession[],
  send: vi.fn(),
  call: vi.fn(),
}))
vi.mock('node:os', async (original) => ({
  default: { ...(await original<typeof import('node:os')>()), homedir: () => mock.home },
}))
vi.mock('./session-manager', () => ({
  getSession: (id: string) => mock.sessions.find((s) => s.id === id),
  sendMcpCall: mock.call,
}))
vi.mock('./session-ws', () => ({ getSessionWs: vi.fn() }))
vi.mock('./session-io', () => ({ sendToClient: mock.send, WS_SEND_MAX_BUFFER: 16 * 1024 * 1024 }))
vi.mock('../logging', () => ({ debugLog: vi.fn(), warnLog: vi.fn(), errorLog: vi.fn(), infoLog: vi.fn() }))
vi.mock('../events/bus', () => ({ eventBus: { emit: vi.fn() } }))

let tasks: typeof import('./mcp-tasks')
let calls: typeof import('./session-mcp-call')
let foreground: typeof import('./mcp-foreground-calls')
let app: Hono
let session: PtySession

/** The client host: runs every `mcp:call` through the extension's own Bash
 *  executor and obeys `mcp:cancel` exactly as the connection manager does. */
const host = {
  running: new Map<string, AbortController>(),
  output: new Map<string, string>(),
  cancels: [] as Frame[],
  acks: [] as boolean[],
}
let runForegroundCommand: RunForeground

// Parent plus a child of its own: the kill must take the tree, not one PID.
const LONG = `const {spawn}=require('node:child_process');const c=spawn(process.execPath,['-e','setInterval(()=>{},1000)'],{stdio:'ignore',windowsHide:true});console.log('pids='+process.pid+','+c.pid);setInterval(()=>{},1000)`
// Linux/macOS signal the process group at once. Windows goes through
// `taskkill /T /F`, whose own start-up can take over a second on a busy machine.
const KILL_LIMIT_MS = process.platform === 'win32' ? 5000 : 1000
const QUICK = `console.log('next prompt works')`

function onFrame(frame: Frame): boolean {
  if (frame.type === 'mcp:cancel') {
    host.cancels.push(frame)
    host.running.get(frame.requestId)?.abort()
    return true
  }
  if (frame.type !== 'mcp:call') return true
  const controller = new AbortController()
  host.running.set(frame.requestId, controller)
  void runForegroundCommand(
    String(frame.input?.command),
    process.execPath,
    60_000,
    ['-e'],
    process.env,
    controller.signal,
    (chunk) => host.output.set(frame.requestId, (host.output.get(frame.requestId) ?? '') + chunk),
  ).then(async (result) => {
    host.running.delete(frame.requestId)
    const outcome = { type: 'mcp:response' as const, requestId: frame.requestId, taskOwner: frame.taskOwner, result }
    host.acks.push(
      frame.taskOwner ? await tasks.settleMcpTask(session, outcome) : calls.settleMcpResult(session.id, outcome),
    )
  })
  return true
}

type Mode = 'task' | 'modern' | 'legacy'
function post(mode: Mode, method: string, params: Record<string, unknown>, id?: number) {
  const capabilities = mode === 'task' ? { extensions: { [TASK_EXTENSION]: {} } } : {}
  const meta = {
    'io.modelcontextprotocol/protocolVersion': MODERN_MCP,
    'io.modelcontextprotocol/clientCapabilities': capabilities,
  }
  return app.request('/mcp/' + session.mcpEndpoint!.id, {
    method: 'POST',
    headers: {
      authorization: 'Bearer ' + session.mcpEndpoint!.token,
      'Content-Type': 'application/json',
      ...(mode === 'legacy'
        ? {}
        : {
            'MCP-Protocol-Version': MODERN_MCP,
            'Mcp-Method': method,
            ...(method === 'tools/call' ? { 'Mcp-Name': String(params.name) } : {}),
          }),
    },
    body: JSON.stringify({
      jsonrpc: '2.0',
      ...(id === undefined ? {} : { id }),
      method,
      params: mode === 'legacy' ? params : { ...params, _meta: meta },
    }),
  })
}
function bash(mode: Mode, command: string, id: number) {
  return post(mode, 'tools/call', { name: 'Bash', arguments: { command } }, id)
}
function rpcResult(text: string) {
  const data = text
    .split('\n')
    .filter((line) => line.startsWith('data: '))
    .map((line) => JSON.parse(line.slice(6)))
  return data.length ? data[data.length - 1].result : JSON.parse(text).result
}
function alive(pid: number): boolean {
  try {
    process.kill(pid, 0)
    return true
  } catch {
    return false
  }
}
/** Starts the long command and returns once both of its processes exist. */
async function startLong(mode: Mode, id: number) {
  const before = new Set(host.output.keys())
  const pending = Promise.resolve(bash(mode, LONG, id)).then((response) => response.text())
  let pids: number[] = []
  let requestId = ''
  await vi.waitFor(
    () => {
      const entry = [...host.output].find(([key, text]) => !before.has(key) && /pids=\d+,\d+/.test(text))
      expect(entry).toBeDefined()
      requestId = entry![0]
      pids = /pids=(\d+),(\d+)/.exec(entry![1])!.slice(1).map(Number)
    },
    { timeout: 10_000, interval: 20 },
  )
  expect(pids.every(alive)).toBe(true)
  return { pending, pids, requestId }
}
/** Milliseconds until every process of the command is gone. */
async function deadWithin(pids: number[], limitMs: number): Promise<number> {
  const started = Date.now()
  await vi.waitFor(() => expect(pids.some(alive)).toBe(false), { timeout: limitMs, interval: 10 })
  return Date.now() - started
}

beforeAll(async () => {
  mock.home = await fs.mkdtemp(path.join(os.tmpdir(), 'kamin-mcp-interrupt-'))
  tasks = await import('./mcp-tasks')
  calls = await import('./session-mcp-call')
  foreground = await import('./mcp-foreground-calls')
  // The extension is a separate package; a variable path keeps tsc out of it.
  const executor = new URL('../../../../extension/src/main/mcp/tools/shell-foreground.ts', import.meta.url).href
  runForegroundCommand = ((await import(/* @vite-ignore */ executor)) as { runForegroundCommand: RunForeground })
    .runForegroundCommand
  const { handleMcpRequest } = await import('./mcp-http-handler')
  app = new Hono().post('/mcp/:sessionId', handleMcpRequest)
  session = {
    id: 'interrupt-pty',
    mcpEndpoint: await tasks.mcpIdentity('synthetic-user', '/synthetic/interrupt'),
    mcpToken: 'synthetic',
    mcpLog: [],
    registeredTools: [],
    ws: { readyState: 1 },
    mcpCallCount: 0,
    inputCount: 0,
    mcpInitialized: true,
    mcpLastError: null,
    lastActivityAt: new Date(),
  } as unknown as PtySession
  mock.sessions = [session]
  mock.send.mockImplementation((_ws: unknown, frame: Frame) => onFrame(frame))
  mock.call.mockImplementation((_id: string, tool: string, input: Record<string, unknown>, signal?: AbortSignal) =>
    calls.sendMcpCall(session, tool, input, signal),
  )
  tasks.startMcpTaskDelivery(() => mock.sessions)
})
afterEach(() => {
  host.cancels.length = 0
  host.acks.length = 0
})
afterAll(async () => {
  for (const controller of host.running.values()) controller.abort()
  mock.sessions = []
  calls.rejectAllPending('test cleanup')
  for (const task of await tasks.mcpTasks.working())
    await tasks.mcpTasks.update(task.owner, task.taskId, (t) => {
      t.status = 'cancelled'
    })
  if (!path.resolve(mock.home).startsWith(path.resolve(os.tmpdir()) + path.sep + 'kamin-mcp-interrupt-'))
    throw new Error('Unexpected test home')
  await fs.rm(mock.home, { recursive: true, force: true })
})

describe('INC-2026-0096: interrupt stops the running host command', () => {
  it.each(['legacy', 'modern', 'task'] as const)(
    'kills the process tree promptly and answers "interrupted" (%s call)',
    async (mode) => {
      const run = await startLong(mode, 11)
      expect(foreground.interruptSessionToolCalls(session)).toBeGreaterThan(0)
      expect(await deadWithin(run.pids, KILL_LIMIT_MS)).toBeLessThan(KILL_LIMIT_MS)

      const result = rpcResult(await run.pending)
      expect(result.isError).toBe(true)
      expect(result.content[0].text).toContain('Interrupted by user')
      expect(result.taskId).toBeUndefined()
      expect(host.cancels).toContainEqual(
        mode === 'task'
          ? { type: 'mcp:cancel', requestId: run.requestId, taskOwner: session.mcpEndpoint!.id }
          : { type: 'mcp:cancel', requestId: run.requestId },
      )
      // The host's own "cancelled" result is acknowledged, so its outbox empties.
      await vi.waitFor(() => expect(host.acks).toEqual([true]))
      if (mode === 'task') {
        const task = await tasks.mcpTasks.get(session.mcpEndpoint!.id, run.requestId)
        expect(task).toMatchObject({ status: 'cancelled' })
        expect(task!.background).toBeFalsy()
      } else expect(calls.pendingMcpCalls.has(run.requestId)).toBe(false)

      const next = rpcResult(await (await bash(mode, QUICK, 12)).text())
      expect(next.isError).toBeFalsy()
      expect(next.content[0].text).toContain('next prompt works')
    },
    30_000,
  )

  it('stops ten commands in a row without leaving a process behind', async () => {
    const survivors: number[] = []
    for (let round = 0; round < 10; round++) {
      const run = await startLong(round % 2 ? 'task' : 'legacy', 100 + round)
      foreground.interruptSessionToolCalls(session)
      expect(await deadWithin(run.pids, KILL_LIMIT_MS)).toBeLessThan(KILL_LIMIT_MS)
      expect(rpcResult(await run.pending).content[0].text).toContain('Interrupted by user')
      survivors.push(...run.pids.filter(alive))
    }
    expect(survivors).toEqual([])
    expect(host.running.size).toBe(0)
  }, 60_000)

  it.each(['legacy', 'task'] as const)(
    'stops the call named by notifications/cancelled (%s call)',
    async (mode) => {
      const run = await startLong(mode, 21)
      const other = await post(mode, 'notifications/cancelled', { requestId: 999 })
      expect(other.status).toBe(202)
      expect(run.pids.every(alive)).toBe(true)

      expect((await post(mode, 'notifications/cancelled', { requestId: 21 })).status).toBe(202)
      expect(await deadWithin(run.pids, KILL_LIMIT_MS)).toBeLessThan(KILL_LIMIT_MS)
      expect(rpcResult(await run.pending).content[0].text).toContain('Interrupted by user')
    },
    30_000,
  )

  it('leaves another session and an explicit background task running', async () => {
    const background = rpcResult(
      await (
        await post('task', 'tools/call', { name: 'Bash', arguments: { command: LONG, run_in_background: true } }, 31)
      ).text(),
    )
    expect(background).toMatchObject({ resultType: 'task', status: 'working' })
    await vi.waitFor(() => expect(host.output.get(background.taskId)).toMatch(/pids=\d+,\d+/), { timeout: 10_000 })
    const pids = /pids=(\d+),(\d+)/.exec(host.output.get(background.taskId)!)!.slice(1).map(Number)

    const stranger = { ...session, id: 'other-pty' } as PtySession
    expect(foreground.interruptSessionToolCalls(stranger)).toBe(0)
    expect(foreground.interruptSessionToolCalls(session)).toBe(0)
    await new Promise((resolve) => setTimeout(resolve, 300))
    expect(pids.every(alive)).toBe(true)
    expect(host.cancels).toEqual([])

    expect((await post('task', 'tasks/cancel', { taskId: background.taskId }, 32)).status).toBe(200)
    expect(await deadWithin(pids, 3000)).toBeLessThan(3000)
  }, 30_000)
})

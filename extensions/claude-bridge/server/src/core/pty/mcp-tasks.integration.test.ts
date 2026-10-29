// Real HTTP handler and task persistence, synthetic WS only; no CLI or host profile.
import { afterAll, beforeAll, describe, expect, it, vi } from 'vitest'
import { promises as fs } from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { Hono } from 'hono'
import type { PtySession } from './types'
import { MODERN_MCP, TASK_EXTENSION } from './mcp-modern'
import { TASK_LIMITS } from './mcp-task-store'

const mock = vi.hoisted(() => ({ home: '', sessions: [] as PtySession[], send: vi.fn(), call: vi.fn() }))
vi.mock('node:os', async (original) => ({
  default: { ...(await original<typeof import('node:os')>()), homedir: () => mock.home },
}))
vi.mock('./session-manager', () => ({
  getSession: (id: string) => mock.sessions.find((s) => s.id === id),
  sendMcpCall: mock.call,
}))
vi.mock('./session-ws', () => ({ getSessionWs: vi.fn() }))
vi.mock('./session-io', () => ({ sendToClient: mock.send }))
vi.mock('../logging', () => ({ debugLog: vi.fn(), warnLog: vi.fn(), errorLog: vi.fn() }))
vi.mock('../events/bus', () => ({ eventBus: { emit: vi.fn() } }))
let tasks: typeof import('./mcp-tasks')
let app: Hono
let session: PtySession
const meta = {
  'io.modelcontextprotocol/protocolVersion': MODERN_MCP,
  'io.modelcontextprotocol/clientCapabilities': { extensions: { [TASK_EXTENSION]: {} } },
}
function request(method: string, params: Record<string, unknown> = {}, optIn = true, id = 1) {
  return app.request('/mcp/' + session.mcpEndpoint!.id, {
    method: 'POST',
    headers: {
      authorization: 'Bearer ' + session.mcpEndpoint!.token,
      'Content-Type': 'application/json',
      'MCP-Protocol-Version': MODERN_MCP,
      'Mcp-Method': method,
      ...(method === 'tools/call' ? { 'Mcp-Name': String(params.name) } : {}),
    },
    body: JSON.stringify({
      jsonrpc: '2.0',
      id,
      method,
      params: { ...params, _meta: optIn ? meta : { ...meta, 'io.modelcontextprotocol/clientCapabilities': {} } },
    }),
  })
}
function rpcResult(text: string) {
  const data = text
    .split('\n')
    .filter((line) => line.startsWith('data: '))
    .map((line) => JSON.parse(line.slice(6)))
  return data.length ? data[data.length - 1].result : JSON.parse(text).result
}
async function create(command = 'synthetic long command') {
  const grace = TASK_LIMITS.foregroundGraceMs
  TASK_LIMITS.foregroundGraceMs = 0
  const response = await request('tools/call', { name: 'Bash', arguments: { command } })
  TASK_LIMITS.foregroundGraceMs = grace
  expect(response.status).toBe(200)
  const handle = rpcResult(await response.text())
  expect(handle).toMatchObject({ resultType: 'task', status: 'working', ttlMs: 86_400_000 })
  return handle.taskId as string
}
beforeAll(async () => {
  mock.home = await fs.mkdtemp(path.join(os.tmpdir(), 'kamin-mcp-wire-'))
  tasks = await import('./mcp-tasks')
  const { handleMcpRequest } = await import('./mcp-http-handler')
  app = new Hono().post('/mcp/:sessionId', handleMcpRequest)
  session = {
    id: 'first-pty',
    mcpEndpoint: await tasks.mcpIdentity('synthetic-user', '/synthetic/conversation'),
    mcpToken: 'synthetic',
    mcpLog: [],
    registeredTools: [],
    ws: { readyState: 1 },
    lastActivityAt: new Date(),
  } as unknown as PtySession
  mock.sessions = [session]
  mock.send.mockReturnValue(true)
  tasks.startMcpTaskDelivery(() => mock.sessions)
  mock.call.mockResolvedValue('ordinary result')
})
afterAll(async () => {
  // No active delivery after test teardown, then delete only this suite's directory.
  mock.sessions = []
  for (const task of await tasks.mcpTasks.working())
    await tasks.mcpTasks.update(task.owner, task.taskId, (t) => {
      t.status = 'cancelled'
    })
  if (!path.resolve(mock.home).startsWith(path.resolve(os.tmpdir()) + path.sep + 'kamin-mcp-wire-'))
    throw new Error('Unexpected test home')
  await fs.rm(mock.home, { recursive: true, force: true })
})
describe('MCP Tasks HTTP / WS lifecycle', () => {
  it('advertises Tasks and recommends foreground commands only for opted-in clients', async () => {
    const discovery = (await (await request('server/discover')).json()).result
    expect(discovery).toMatchObject({
      resultType: 'complete',
      supportedVersions: [MODERN_MCP],
      capabilities: { extensions: { [TASK_EXTENSION]: {} } },
    })
    expect(discovery.instructions).toContain('Leave run_in_background false or omitted')
    const ordinary = (await (await request('server/discover', {}, false)).json()).result
    expect(ordinary.instructions).toContain('For long-running jobs pass run_in_background:true')
  })
  it('returns a handle before execution settles, then retains output through PTY replacement and lost ack', async () => {
    const id = await create()
    await vi.waitFor(() =>
      expect(mock.send).toHaveBeenCalledWith(
        session.ws,
        expect.objectContaining({ type: 'mcp:call', requestId: id, taskOwner: session.mcpEndpoint!.id }),
      ),
    )
    expect((await (await request('tasks/get', { taskId: id })).json()).result.status).toBe('working')
    const oldIdentity = session.mcpEndpoint!
    session = {
      ...session,
      id: 'replacement-pty',
      mcpEndpoint: await tasks.mcpIdentity('synthetic-user', '/synthetic/conversation'),
    }
    mock.sessions = [session]
    expect(session.mcpEndpoint).toEqual(oldIdentity)
    const outcome = { type: 'mcp:response', requestId: id, taskOwner: oldIdentity.id, result: 'durable output' }
    expect(await tasks.settleMcpTask(session, outcome)).toBe(true)
    expect(await tasks.settleMcpTask(session, { ...outcome, result: 'duplicate' })).toBe(true)
    const response = await request('tasks/get', { taskId: id })
    expect((await response.json()).result).toMatchObject({
      resultType: 'complete',
      status: 'completed',
      result: { resultType: 'complete', content: [{ text: 'durable output' }] },
    })
    expect(response.headers.get('Mcp-Session-Id')).toBeNull()
    expect(
      mock.send.mock.calls.filter(([, frame]) => frame.type === 'mcp:call' && frame.requestId === id),
    ).toHaveLength(1)
  })
  it('answers a foreground command inline when it settles within the grace window', async () => {
    const pending = request('tools/call', { name: 'Bash', arguments: { command: 'quick' } })
    let frame: { requestId: string } | undefined
    await vi.waitFor(() => {
      frame = mock.send.mock.calls.map(([, f]) => f).find((f) => f.type === 'mcp:call' && f.input?.command === 'quick')
      expect(frame).toBeDefined()
    })
    await tasks.settleMcpTask(session, {
      type: 'mcp:response',
      requestId: frame!.requestId,
      taskOwner: session.mcpEndpoint!.id,
      result: 'quick output',
    })
    const result = rpcResult(await (await pending).text())
    expect(result).toMatchObject({ resultType: 'complete', content: [{ text: 'quick output' }] })
    expect(result.taskId).toBeUndefined()
  })
  it('preserves synchronous calls when the client omits task opt-in', async () => {
    const response = await request('tools/call', { name: 'Bash', arguments: { command: 'short' } }, false)
    expect(await response.text()).toContain('ordinary result')
    expect(mock.call).toHaveBeenCalledWith(session.id, 'Bash', { command: 'short' }, expect.any(AbortSignal))
  })
  it('cooperatively cancels a delivered task and rejects a late success', async () => {
    const id = await create()
    await vi.waitFor(() =>
      expect(mock.send).toHaveBeenCalledWith(session.ws, expect.objectContaining({ type: 'mcp:call', requestId: id })),
    )
    expect((await (await request('tasks/cancel', { taskId: id })).json()).result.resultType).toBe('complete')
    await vi.waitFor(() =>
      expect(mock.send).toHaveBeenCalledWith(session.ws, {
        type: 'mcp:cancel',
        requestId: id,
        taskOwner: session.mcpEndpoint!.id,
      }),
    )
    await tasks.settleMcpTask(session, {
      type: 'mcp:response',
      requestId: id,
      taskOwner: session.mcpEndpoint!.id,
      result: 'too late',
    })
    expect((await (await request('tasks/get', { taskId: id })).json()).result).toMatchObject({ status: 'cancelled' })
  })
  it('cancels an undelivered task without ever executing it', async () => {
    session.ws = { readyState: 0 } as PtySession['ws']
    const id = await create()
    await request('tasks/cancel', { taskId: id })
    expect((await (await request('tasks/get', { taskId: id })).json()).result.status).toBe('cancelled')
    expect(mock.send.mock.calls.some(([, frame]) => frame.type === 'mcp:call' && frame.requestId === id)).toBe(false)
    session.ws = { readyState: 1 } as PtySession['ws']
  })
  it('isolates task status, cancellation and subscriptions by authenticated owner', async () => {
    const id = await create()
    const previous = session
    session = {
      ...session,
      id: 'other-pty',
      mcpEndpoint: await tasks.mcpIdentity('other-user', '/synthetic/conversation'),
    }
    mock.sessions = [session, previous]
    expect((await request('tasks/get', { taskId: id })).status).toBe(400)
    expect((await request('tasks/cancel', { taskId: id })).status).toBe(400)
    expect(
      await tasks.settleMcpTask(session, {
        type: 'mcp:response',
        requestId: id,
        taskOwner: previous.mcpEndpoint!.id,
        result: 'wrong owner',
      }),
    ).toBe(false)
    const response = await request('subscriptions/listen', { notifications: { taskIds: [id] } })
    const reader = response.body!.getReader()
    expect(new TextDecoder().decode((await reader.read()).value)).not.toContain(id)
    await reader.cancel()
    session = previous
    mock.sessions = [session]
    await tasks.settleMcpTask(session, {
      type: 'mcp:response',
      requestId: id,
      taskOwner: session.mcpEndpoint!.id,
      result: 'done',
    })
  })
  it('acknowledges a subscription and sends the complete terminal result', async () => {
    const id = await create()
    const response = await request('subscriptions/listen', { notifications: { taskIds: [id] } })
    const reader = response.body!.getReader()
    const decode = (bytes: Uint8Array | undefined) => new TextDecoder().decode(bytes)
    expect(decode((await reader.read()).value)).toContain('notifications/subscriptions/acknowledged')
    expect(decode((await reader.read()).value)).toContain('working')
    await tasks.settleMcpTask(session, {
      type: 'mcp:response',
      requestId: id,
      taskOwner: session.mcpEndpoint!.id,
      result: 'finished',
    })
    const notification = decode((await reader.read()).value)
    expect(notification).toContain('notifications/tasks')
    expect(notification).toContain('finished')
    expect(notification).toContain('io.modelcontextprotocol/subscriptionId')
    const { closeToolNotificationStream } = await import('./mcp-tool-notify')
    closeToolNotificationStream(session.id)
    expect(decode((await reader.read()).value)).toContain('"result":{"resultType":"complete"}')
    expect((await reader.read()).done).toBe(true)
  })
  it('does not swallow request id zero as an unknown notification', async () => {
    expect((await request('unknown/method', {}, true, 0)).status).toBe(404)
  })
})

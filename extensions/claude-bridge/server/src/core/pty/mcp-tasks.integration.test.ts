// Real HTTP handler and task persistence, synthetic WS only; no CLI or host profile.
import { afterAll, afterEach, beforeAll, describe, expect, it, vi } from 'vitest'
import { promises as fs } from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { Hono } from 'hono'
import type { PtySession } from './types'
import { MODERN_MCP, TASK_EXTENSION } from './mcp-modern'
import { TASK_LIMITS } from './mcp-task-store'
import { mcpInstructions } from './mcp-instructions'

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
  it('advertises immediate background Tasks only for opted-in clients', async () => {
    const discovery = (await (await request('server/discover')).json()).result
    expect(discovery).toMatchObject({
      resultType: 'complete',
      supportedVersions: [MODERN_MCP],
      capabilities: { extensions: { [TASK_EXTENSION]: {} } },
    })
    expect(discovery.instructions).toContain('returns a task immediately')
    const ordinary = (await (await request('server/discover', {}, false)).json()).result
    expect(ordinary.instructions).toContain('client-side bash_id')
  })
  it.each([true, false])('shares initialize/discover instructions with Tasks support = %s', async (optIn) => {
    const initialized = (await (await request('initialize', {}, optIn)).json()).result
    const discovered = (await (await request('server/discover', {}, optIn)).json()).result
    expect(initialized.instructions).toBe(mcpInstructions(optIn))
    expect(discovered.instructions).toBe(initialized.instructions)
    expect(discovered.instructions.split('Shell:')).toHaveLength(2)
  })
  it.each(['Bash', 'PowerShell'])(
    'admits explicit background %s without grace and sends a protocol deadline',
    async (tool) => {
      const started = Date.now()
      const response = await request('tools/call', {
        name: tool,
        arguments: { command: 'background-' + tool, run_in_background: true, timeout: 1 },
      })
      expect(response.headers.get('content-type')).toContain('application/json')
      const handle = rpcResult(await response.text())
      expect(handle).toMatchObject({ resultType: 'task', status: 'working' })
      expect(Date.now() - started).toBeLessThan(2000)
      await vi.waitFor(() =>
        expect(mock.send).toHaveBeenCalledWith(
          session.ws,
          expect.objectContaining({
            type: 'mcp:call',
            requestId: handle.taskId,
            taskDeadlineMs: expect.any(Number),
            input: { command: 'background-' + tool, run_in_background: true, timeout: 1 },
          }),
        ),
      )
      const task = (await tasks.mcpTasks.get(session.mcpEndpoint!.id, handle.taskId))!
      expect(task.executionDeadlineMs! - started).toBeGreaterThan(TASK_LIMITS.executionMs)
      await tasks.settleMcpTask(session, {
        type: 'mcp:response',
        requestId: handle.taskId,
        taskOwner: session.mcpEndpoint!.id,
        result: 'full background output',
      })
      expect((await (await request('tasks/get', { taskId: handle.taskId })).json()).result).toMatchObject({
        status: 'completed',
        result: { content: [{ text: 'full background output' }] },
      })
    },
  )
  it('preserves legacy background calls for clients without Tasks', async () => {
    const input = { command: 'legacy', run_in_background: true }
    const response = await request('tools/call', { name: 'Bash', arguments: input }, false)
    expect(await response.text()).toContain('ordinary result')
    expect(mock.call).toHaveBeenCalledWith(session.id, 'Bash', input, expect.any(AbortSignal), expect.any(String))
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
    expect(mock.call).toHaveBeenCalledWith(
      session.id,
      'Bash',
      { command: 'short' },
      expect.any(AbortSignal),
      expect.any(String),
    )
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

describe('retention overload through the actual HTTP handler', () => {
  const limits = { ...TASK_LIMITS }
  let protectedId: string
  let protectedResult: unknown
  beforeAll(async () => {
    for (const task of await tasks.mcpTasks.working())
      await tasks.mcpTasks.settle(task.owner, task.taskId, 'test finished')
    const owner = session.mcpEndpoint!.id
    const task = await tasks.mcpTasks.create(owner, 'Bash', {})
    protectedId = task.taskId
    await tasks.mcpTasks.settle(owner, protectedId, 'unread protected output')
    protectedResult = (await (await request('tasks/get', { taskId: protectedId })).json()).result
    let count = (await tasks.mcpTasks.commands(owner)).length
    while (count++ < TASK_LIMITS.perOwner) {
      const next = await tasks.mcpTasks.create(owner, 'Bash', {})
      await tasks.mcpTasks.settle(owner, next.taskId, 'later output')
    }
    // This fixture durably creates and settles the actual 256-record owner limit.
    // Match the real-I/O retention tests rather than the 10-second hook default.
  }, 60_000)
  afterEach(() => {
    Object.assign(TASK_LIMITS, limits)
    vi.restoreAllMocks()
    mock.call.mockResolvedValue('ordinary result')
  })

  it.each(['Bash', 'PowerShell'])('answers foreground %s inline when owner retention is full', async (name) => {
    mock.call.mockClear()
    const result = rpcResult(await (await request('tools/call', { name, arguments: { command: 'synthetic' } })).text())
    expect(result).toMatchObject({ resultType: 'complete', content: [{ text: 'ordinary result' }] })
    expect(result).not.toHaveProperty('taskId')
    expect(mock.call).toHaveBeenCalledTimes(1)
    expect((await (await request('tasks/get', { taskId: protectedId })).json()).result).toEqual(protectedResult)
    expect(await tasks.mcpTasks.settle(session.mcpEndpoint!.id, protectedId, 'lost-ACK duplicate')).toMatchObject({
      result: { content: [{ text: 'unread protected output' }] },
    })
  })

  it('refuses explicit background before dispatch without deleting old results', async () => {
    mock.call.mockClear()
    const sent = mock.send.mock.calls.length
    const response = await request('tools/call', {
      name: 'Bash',
      arguments: { command: 'synthetic', run_in_background: true },
    })
    expect(response.status).toBe(503)
    expect((await response.json()).error.data).toEqual({ reason: 'retained_owner', executionStarted: false })
    expect(mock.call).not.toHaveBeenCalled()
    expect(mock.send).toHaveBeenCalledTimes(sent)
    expect((await (await request('tasks/get', { taskId: protectedId })).json()).result).toEqual(protectedResult)
  })

  it.each(['count', 'bytes'])('preserves foreground execution under global retention %s pressure', async (kind) => {
    if (kind === 'count') TASK_LIMITS.retained = 1
    else TASK_LIMITS.bytes = 1
    expect(rpcResult(await (await request('tools/call', { name: 'Bash', arguments: {} })).text())).toMatchObject({
      resultType: 'complete',
      content: [{ text: 'ordinary result' }],
    })
  })

  it('shares active capacity with ordinary calls and does not bypass that limit', async () => {
    mock.call.mockClear()
    const release = await tasks.mcpTasks.acquireInline(session.mcpEndpoint!.id)
    TASK_LIMITS.active = 1
    try {
      const response = await request('tools/call', { name: 'Bash', arguments: {} })
      expect(response.status).toBe(503)
      expect((await response.json()).error.data).toEqual({ reason: 'active_global', executionStarted: false })
      expect(mock.call).not.toHaveBeenCalled()
    } finally {
      release()
    }
  })

  it('releases an ordinary lease on success and execution failure', async () => {
    TASK_LIMITS.activePerOwner = 1
    mock.call.mockRejectedValueOnce(new Error('synthetic execution failed'))
    const failed = rpcResult(await (await request('tools/call', { name: 'Bash', arguments: {} })).text())
    expect(failed).toMatchObject({ isError: true, content: [{ text: 'Error: synthetic execution failed' }] })
    const next = rpcResult(await (await request('tools/call', { name: 'Bash', arguments: {} })).text())
    expect(next).toMatchObject({ resultType: 'complete', content: [{ text: 'ordinary result' }] })
    const release = await tasks.mcpTasks.acquireInline(session.mcpEndpoint!.id)
    release()
  })

  it.each(['queue', 'write'])('does not fall back after a %s persistence failure', async (kind) => {
    mock.call.mockClear()
    if (kind === 'queue') TASK_LIMITS.queued = 0
    else {
      TASK_LIMITS.perOwner++
      vi.spyOn(fs, 'rename').mockRejectedValueOnce(new Error('synthetic disk failure with a private path'))
    }
    const response = await request('tools/call', { name: 'Bash', arguments: {} })
    expect(response.status).toBe(503)
    const body = await response.json()
    expect(body.error.data).toEqual({
      reason: kind === 'queue' ? 'persistence_queue' : 'persistence_unavailable',
      executionStarted: 'unknown',
    })
    expect(JSON.stringify(body)).not.toContain('private path')
    expect(mock.call).not.toHaveBeenCalled()
  })

  it('rejects an oversized input without executing an ordinary fallback', async () => {
    mock.call.mockClear()
    const response = await request('tools/call', {
      name: 'Bash',
      arguments: { command: 'x'.repeat(TASK_LIMITS.inputBytes) },
    })
    expect(response.status).toBe(400)
    expect((await response.json()).error.data).toEqual({ reason: 'input_bytes', executionStarted: false })
    expect(mock.call).not.toHaveBeenCalled()
  })
})

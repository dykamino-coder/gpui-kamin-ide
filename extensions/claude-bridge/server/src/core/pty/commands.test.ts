import { beforeEach, describe, expect, it, vi } from 'vitest'
import type { PtySession } from './types'
const mock = vi.hoisted(() => ({ pending: new Map(), tasks: [] as any[], send: vi.fn() }))
vi.mock('./session-mcp-call', () => ({ pendingMcpCalls: mock.pending }))
vi.mock('./mcp-tasks', () => ({
  mcpTasks: {
    working: async () => mock.tasks,
    commands: async (owner: string) => mock.tasks.filter((t) => t.owner === owner && !t.hiddenFromCommands),
    clearCommands: async (owner: string) => {
      for (const task of mock.tasks)
        if (task.owner === owner && task.status !== 'working') task.hiddenFromCommands = true
    },
    get: async (owner: string, id: string) => mock.tasks.find((t) => t.owner === owner && t.taskId === id),
    update: async (owner: string, id: string, fn: (t: any) => void) =>
      fn(mock.tasks.find((t) => t.owner === owner && t.taskId === id)),
  },
}))
vi.mock('./session-io', () => ({ sendToClient: mock.send }))
import { acceptCommandReport, commandSnapshot, commandOutput, stopCommand, clearCompletedCommands } from './commands'
const session = { id: 'pty-a', mcpEndpoint: { id: 'owner-a' }, ws: {} } as PtySession
const other = { id: 'pty-b', mcpEndpoint: { id: 'owner-b' }, ws: {} } as PtySession
beforeEach(() => {
  mock.pending.clear()
  mock.tasks.length = 0
  mock.send.mockClear()
})
describe('session commands projection', () => {
  it('sorts background first and excludes another conversation', async () => {
    mock.pending.set('foreground', { sessionId: session.id, toolName: 'Grep', input: { command: 'fg' }, createdAt: 1 })
    mock.pending.set('foreign', { sessionId: other.id, toolName: 'Bash', input: {}, createdAt: 1 })
    mock.tasks.push({
      owner: 'owner-a',
      taskId: 'task',
      toolName: 'Bash',
      command: 'bg',
      input: {},
      background: true,
      status: 'working',
      createdAt: new Date(2).toISOString(),
    })
    await acceptCommandReport(session, {
      calls: [{ id: 'foreign', output: 'private' }],
      shells: [{ id: 'shell:1', tool: 'Bash', command: 'legacy', startedAt: 3, running: true, output: 'tail' }],
    })
    expect((await commandSnapshot(session)).commands.map((c) => c.id)).toEqual(['task', 'shell:1', 'foreground'])
    expect(await commandOutput(session, 'foreign')).toBe('')
    expect(await commandOutput(other, 'shell:1')).toBe('')
    expect(await commandOutput(session, 'shell:1')).toBe('tail')
  })
  it('cancels through the matching execution owner only', async () => {
    mock.tasks.push({ owner: 'owner-a', taskId: 'cancel-task', status: 'working' })
    await stopCommand(other, 'cancel-task')
    expect(mock.send).not.toHaveBeenCalled()
    await stopCommand(session, 'cancel-task')
    expect(mock.tasks[0].cancelRequested).toBe(true)
    expect(mock.send).toHaveBeenCalledWith(session.ws, {
      type: 'mcp:cancel',
      requestId: 'cancel-task',
      taskOwner: 'owner-a',
    })
  })
})

it('retains finished tasks and legacy shells with metadata, final output and owner-scoped clearing', async () => {
  mock.tasks.push({
    owner: 'owner-a',
    taskId: 'done',
    toolName: 'Bash',
    command: 'full command',
    input: {},
    description: 'Build app',
    background: true,
    status: 'completed',
    createdAt: new Date(1000).toISOString(),
    lastUpdatedAt: new Date(6000).toISOString(),
    result: { exitCode: 7, isError: true, content: [{ type: 'text', text: 'failed output' }] },
  })
  mock.tasks.push({ ...mock.tasks[0], owner: 'owner-b', taskId: 'private' })
  await acceptCommandReport(session, {
    calls: [],
    shells: [
      {
        id: 'shell:2',
        tool: 'PowerShell',
        command: 'echo legacy',
        description: 'Legacy task',
        startedAt: 2000,
        finishedAt: 8000,
        exitCode: 0,
        running: false,
        output: 'done',
      },
    ],
  })
  expect((await commandSnapshot(session)).commands).toEqual([
    expect.objectContaining({ id: 'done', description: 'Build app', finishedAt: 6000, status: 'failed', exitCode: 7 }),
    expect.objectContaining({
      id: 'shell:2',
      description: 'Legacy task',
      finishedAt: 8000,
      status: 'completed',
      exitCode: 0,
    }),
  ])
  expect(await commandOutput(session, 'done')).toBe('failed output')
  await clearCompletedCommands(other)
  expect((await commandSnapshot(session)).commands).toHaveLength(2)
  await clearCompletedCommands(session)
  expect((await commandSnapshot(session)).commands).toHaveLength(0)
  expect(await commandOutput(session, 'done')).toBe('failed output')
})

import { describe, expect, it, vi } from 'vitest'
import { runTaskShellTool, taskIdArgument } from './mcp-task-shell-tools'
import type { TaskRecord } from './mcp-task-store'
import type { PtySession } from './types'

const session = { mcpEndpoint: { id: 'owner-1' } } as unknown as PtySession
const TASK_ID = '7fc6f8ec-cb02-4157-a260-dd46360b64da'

function task(over: Partial<TaskRecord> = {}): TaskRecord {
  return {
    taskId: TASK_ID,
    owner: 'owner-1',
    toolName: 'Bash',
    input: {},
    status: 'working',
    createdAt: '',
    lastUpdatedAt: '',
    ttlMs: 1,
    pollIntervalMs: 1,
    ...over,
  }
}

function deps(record?: TaskRecord) {
  return { get: vi.fn(async () => record), cancel: vi.fn(async () => {}) }
}

describe('taskIdArgument', () => {
  it('takes task ids and leaves numeric legacy shell ids to the client', () => {
    expect(taskIdArgument({ bash_id: TASK_ID })).toBe(TASK_ID)
    expect(taskIdArgument({ shell_id: ` ${TASK_ID} ` })).toBe(TASK_ID)
    expect(taskIdArgument({ bash_id: '7' })).toBeNull()
    expect(taskIdArgument({ bash_id: 7 })).toBeNull()
    expect(taskIdArgument({})).toBeNull()
  })
})

describe('runTaskShellTool', () => {
  it('stops a running background task on KillShell', async () => {
    const d = deps(task())
    const r = await runTaskShellTool(session, 'KillShell', { bash_id: TASK_ID }, d)
    expect(d.get).toHaveBeenCalledWith('owner-1', TASK_ID)
    expect(d.cancel).toHaveBeenCalledWith(session, TASK_ID)
    expect(r?.content[0]?.text).toBe(`Stopped Bash task ${TASK_ID}.`)
  })

  it('reports a running task and the output of a finished one on BashOutput', async () => {
    const running = await runTaskShellTool(session, 'BashOutput', { bash_id: TASK_ID }, deps(task()))
    expect(running?.content[0]?.text).toContain('is still running')
    const done = await runTaskShellTool(
      session,
      'BashOutput',
      { bash_id: TASK_ID },
      deps(task({ status: 'completed', result: { content: [{ type: 'text', text: 'bgdone\n' }] } })),
    )
    expect(done?.content[0]?.text).toBe(`Bash task ${TASK_ID} finished.\n\nbgdone\n`)
    expect(done?.isError).toBeUndefined()
  })

  it('does not cancel a task that already ended', async () => {
    const d = deps(task({ status: 'cancelled' }))
    const r = await runTaskShellTool(session, 'KillShell', { bash_id: TASK_ID }, d)
    expect(d.cancel).not.toHaveBeenCalled()
    expect(r?.content[0]?.text).toContain('was stopped')
  })

  it('leaves unknown ids, numeric ids, other tools and non-shell tasks to the client', async () => {
    expect(await runTaskShellTool(session, 'KillShell', { bash_id: TASK_ID }, deps(undefined))).toBeNull()
    const d = deps(task())
    expect(await runTaskShellTool(session, 'KillShell', { bash_id: '3' }, d)).toBeNull()
    expect(d.get).not.toHaveBeenCalled()
    expect(await runTaskShellTool(session, 'Read', { bash_id: TASK_ID }, deps(task()))).toBeNull()
    expect(
      await runTaskShellTool(session, 'BashOutput', { bash_id: TASK_ID }, deps(task({ toolName: 'Glob' }))),
    ).toBeNull()
  })
})

import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { promises as fs } from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { McpTaskStore, TASK_LIMITS, taskView, taskDeadline } from './mcp-task-store'
let directory: string
let clock: number
let store: McpTaskStore
beforeEach(async () => {
  directory = await fs.mkdtemp(path.join(os.tmpdir(), 'kamin-mcp-task-test-'))
  clock = Date.now()
  store = new McpTaskStore(directory, () => clock)
})
afterEach(async () => {
  vi.restoreAllMocks()
  if (!path.resolve(directory).startsWith(path.resolve(os.tmpdir()) + path.sep + 'kamin-mcp-task-test-'))
    throw new Error('Unexpected test directory')
  await fs.rm(directory, { recursive: true, force: true })
})
describe('durable bounded task lifecycle', () => {
  it('retains a background deadline across restart and past the foreground execution limit', async () => {
    const fg = await store.create('owner', 'Bash', { command: 'foreground' })
    const bg = await store.create('owner-bg', 'PowerShell', { command: 'background', run_in_background: true })
    expect(taskDeadline(fg)).toBe(clock + TASK_LIMITS.executionMs)
    expect(taskDeadline(bg)).toBe(clock + TASK_LIMITS.backgroundExecutionMs)
    expect(taskDeadline(bg) + 60_000).toBe(clock + TASK_LIMITS.ttlMs)
    expect(taskView(bg)).not.toHaveProperty('executionDeadlineMs')
    clock += TASK_LIMITS.executionMs + 60_001
    store = new McpTaskStore(directory, () => clock)
    await store.working()
    expect(store.hasWorkingOwner('owner')).toBe(false)
    expect(store.hasWorkingOwner('owner-bg')).toBe(true)
    expect(taskDeadline((await store.get('owner-bg', bg.taskId))!)).toBe(taskDeadline(bg))
  })
  it('retains completed output across store restart and lost acknowledgement', async () => {
    const task = await store.create('owner', 'Bash', { command: 'echo test' })
    expect(taskView(task)).not.toHaveProperty('input')
    expect(await store.get('other', task.taskId)).toBeUndefined()
    await store.settle('owner', task.taskId, { content: [{ type: 'text', text: 'done' }] })
    store = new McpTaskStore(directory, () => clock)
    const persisted = await store.get('owner', task.taskId)
    expect(persisted).toMatchObject({
      status: 'completed',
      result: { resultType: 'complete', content: [{ type: 'text', text: 'done' }] },
    })
    expect(await store.settle('owner', task.taskId, 'duplicate')).toEqual(persisted)
    expect(
      await store.update('owner', task.taskId, (t) => {
        t.status = 'cancelled'
      }),
    ).toEqual(persisted)
  })
  it('does not acknowledge a failed durable settlement; retry succeeds', async () => {
    const task = await store.create('owner', 'Bash', {})
    vi.spyOn(fs, 'rename').mockRejectedValueOnce(new Error('synthetic disk failure'))
    await expect(store.settle('owner', task.taskId, 'done')).rejects.toThrow('disk failure')
    expect((await store.get('owner', task.taskId))?.status).toBe('working')
    expect(await fs.readdir(directory)).toEqual([task.taskId + '.json'])
    expect((await store.settle('owner', task.taskId, 'done'))?.status).toBe('completed')
  })
  it('settles cancellation before a late successful result', async () => {
    const task = await store.create('owner', 'Bash', {})
    await store.update('owner', task.taskId, (t) => {
      t.cancelRequested = true
    })
    expect((await store.settle('owner', task.taskId, 'late output'))?.status).toBe('cancelled')
    expect((await store.get('owner', task.taskId))?.result).toBeUndefined()
  })
  it('bounds active admission independently per owner and globally', async () => {
    for (let i = 0; i < TASK_LIMITS.active; i++)
      await store.create('owner-' + Math.floor(i / TASK_LIMITS.activePerOwner), 'Bash', {})
    await expect(store.create('new-owner', 'Bash', {})).rejects.toThrow('capacity')
    const first = (await store.working())[0]!
    await store.settle(first.owner, first.taskId, 'done')
    await expect(store.create('owner-1', 'Bash', {})).rejects.toThrow('capacity')
    await expect(store.create('new-owner', 'Bash', {})).resolves.toHaveProperty('taskId')
  })
  it('expires from creation time and rejects excessive inputs/results', async () => {
    await expect(store.create('owner', 'Bash', { command: 'x'.repeat(TASK_LIMITS.inputBytes) })).rejects.toThrow(
      'input too large',
    )
    const task = await store.create('owner', 'Bash', {})
    await store.settle('owner', task.taskId, 'x'.repeat(TASK_LIMITS.resultBytes))
    expect((await store.get('owner', task.taskId))?.result).toMatchObject({ isError: true })
    clock += TASK_LIMITS.ttlMs
    expect(await store.get('owner', task.taskId)).toBeUndefined()
    expect(await fs.readdir(directory)).toEqual([])
  })
  it('does not create a visible handle if admission cannot be persisted', async () => {
    vi.spyOn(fs, 'rename').mockRejectedValueOnce(new Error('synthetic admission failure'))
    await expect(store.create('owner', 'Bash', {})).rejects.toThrow('admission failure')
    expect(await store.working()).toEqual([])
    expect(await fs.readdir(directory)).toEqual([])
  })
})

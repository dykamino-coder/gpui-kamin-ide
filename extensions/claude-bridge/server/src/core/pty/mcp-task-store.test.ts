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

describe('settled outcomes do not consume admission (INC-2026-0082)', () => {
  // Real limits are sized for ~50 sessions; the contract is the same at a small scale.
  const real = { ...TASK_LIMITS }
  beforeEach(() => {
    Object.assign(TASK_LIMITS, { retained: 48, perOwner: 16, active: 8, activePerOwner: 2 })
  })
  afterEach(() => {
    Object.assign(TASK_LIMITS, real)
  })

  async function settledTask(owner: string): Promise<string> {
    const task = await store.create(owner, 'Bash', { command: 'echo' })
    clock += 1
    await store.settle(owner, task.taskId, 'done')
    return task.taskId
  }

  it('admits a fresh owner into a store full of settled tasks and evicts the oldest settled one', async () => {
    const ids: string[] = []
    for (let i = 0; i < TASK_LIMITS.retained; i++) ids.push(await settledTask('owner-' + (i % 4)))
    const fresh = await store.create('fresh-owner', 'Bash', {})
    expect(fresh.status).toBe('working')
    expect(await store.get('owner-0', ids[0]!)).toBeUndefined()
    expect(await store.get('owner-1', ids[1]!)).toMatchObject({ status: 'completed' })
    expect((await fs.readdir(directory)).length).toBe(TASK_LIMITS.retained)
  }, 60_000)

  it('lets an owner with a full settled history start new commands', async () => {
    for (let i = 0; i < TASK_LIMITS.perOwner; i++) await settledTask('owner')
    await expect(store.create('owner', 'Bash', {})).resolves.toHaveProperty('taskId')
  }, 60_000)

  it('admits after restart over an already full settled store', async () => {
    for (let i = 0; i < TASK_LIMITS.retained; i++) await settledTask('owner-' + (i % 4))
    store = new McpTaskStore(directory, () => clock)
    await expect(store.create('fresh-owner', 'Bash', {})).resolves.toHaveProperty('taskId')
  }, 60_000)

  it('never evicts working tasks', async () => {
    for (let i = TASK_LIMITS.active; i < TASK_LIMITS.retained; i++) await settledTask('done-' + (i % 4))
    for (let i = 0; i < TASK_LIMITS.active; i++)
      await store.create('busy-' + Math.floor(i / TASK_LIMITS.activePerOwner), 'Bash', {})
    await expect(store.create('fresh-owner', 'Bash', {})).rejects.toThrow('capacity')
    expect(await store.working()).toHaveLength(TASK_LIMITS.active)
    expect((await fs.readdir(directory)).length).toBe(TASK_LIMITS.retained)
  }, 60_000)

  it('makes byte room for a settling result from older settled outcomes', async () => {
    const big = 'x'.repeat(TASK_LIMITS.resultBytes - 1024)
    TASK_LIMITS.bytes = 2.5 * TASK_LIMITS.resultBytes
    const first = await store.create('owner-a', 'Bash', {})
    await store.settle('owner-a', first.taskId, big)
    clock += 1
    const second = await store.create('owner-b', 'Bash', {})
    await store.settle('owner-b', second.taskId, big)
    clock += 1
    const third = await store.create('owner-c', 'Bash', {})
    expect((await store.settle('owner-c', third.taskId, big))?.status).toBe('completed')
    expect(await store.get('owner-a', first.taskId)).toBeUndefined()
    expect(await store.get('owner-b', second.taskId)).toMatchObject({ status: 'completed' })
  })
})

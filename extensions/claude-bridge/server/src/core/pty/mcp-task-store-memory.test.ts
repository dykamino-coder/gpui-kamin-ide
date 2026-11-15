import { afterEach, beforeEach, describe, expect, it } from 'vitest'
import { promises as fs } from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { McpTaskStore, TASK_LIMITS } from './mcp-task-store'

// A foreground task lives in memory until it becomes a background task; only
// then is it written to disk, where a restarted server finds it.
let directory: string
let clock: number
let store: McpTaskStore
const files = async () => (await fs.readdir(directory)).filter((name) => name.endsWith('.json'))
beforeEach(async () => {
  directory = await fs.mkdtemp(path.join(os.tmpdir(), 'kamin-mcp-task-memory-'))
  clock = Date.now()
  store = new McpTaskStore(directory, () => clock)
})
afterEach(async () => {
  if (!path.resolve(directory).startsWith(path.resolve(os.tmpdir()) + path.sep + 'kamin-mcp-task-memory-'))
    throw new Error('Unexpected test directory')
  await fs.rm(directory, { recursive: true, force: true })
})

describe('in-memory foreground tasks', () => {
  it('admits, delivers and settles a foreground task without writing a file', async () => {
    const task = await store.create('owner', 'Bash', { command: 'echo hi' }, { durable: false })
    await store.update('owner', task.taskId, (record) => {
      record.delivered = true
    })
    expect(await store.settle('owner', task.taskId, 'hi')).toMatchObject({
      status: 'completed',
      result: { content: [{ text: 'hi' }] },
    })
    expect(await store.get('owner', task.taskId)).toMatchObject({ result: { content: [{ text: 'hi' }] } })
    expect(await files()).toEqual([])
    expect(store.isDurable(task.taskId)).toBe(false)
  })

  it('is forgotten by a restarted server, like an ordinary inline call', async () => {
    const task = await store.create('owner', 'Bash', { command: 'echo hi' }, { durable: false })
    const restarted = new McpTaskStore(directory, () => clock)
    expect(await restarted.get('owner', task.taskId)).toBeUndefined()
  })

  it('writes the task to disk when it turns into a background task, and a restart resumes it', async () => {
    const task = await store.create('owner', 'Bash', { command: 'sleep 600' }, { durable: false })
    await store.update('owner', task.taskId, (record) => {
      record.delivered = true
    })
    const persisted = await store.persist('owner', task.taskId, (record) => {
      record.background = true
    })
    expect(persisted).toMatchObject({ status: 'working', background: true, delivered: true })
    expect(await files()).toEqual([`${task.taskId}.json`])
    expect(store.isDurable(task.taskId)).toBe(true)
    const restarted = new McpTaskStore(directory, () => clock)
    expect((await restarted.working()).map((t) => t.taskId)).toEqual([task.taskId])
    expect(await restarted.settle('owner', task.taskId, 'done')).toMatchObject({ status: 'completed' })
    expect(await new McpTaskStore(directory, () => clock).get('owner', task.taskId)).toMatchObject({
      status: 'completed',
      result: { content: [{ text: 'done' }] },
    })
  })

  it('persists a task that settled just before it turned into a background task', async () => {
    const task = await store.create('owner', 'Bash', { command: 'echo late' }, { durable: false })
    await store.settle('owner', task.taskId, 'late')
    expect(
      await store.persist('owner', task.taskId, (record) => {
        record.background = true
      }),
    ).toMatchObject({ status: 'completed', result: { content: [{ text: 'late' }] } })
    expect(await new McpTaskStore(directory, () => clock).get('owner', task.taskId)).toMatchObject({
      status: 'completed',
    })
  })

  it('drops the result body after the inline answer and the record at expiry', async () => {
    const task = await store.create('owner', 'Bash', { command: 'echo hi' }, { durable: false })
    await store.settle('owner', task.taskId, 'x'.repeat(1000))
    await store.release('owner', task.taskId)
    const listed = await store.get('owner', task.taskId)
    expect(listed).toMatchObject({ status: 'completed', result: { isError: undefined } })
    expect(JSON.stringify(listed)).not.toContain('x'.repeat(100))
    clock += TASK_LIMITS.ttlMs + 1
    expect(await store.get('owner', task.taskId)).toBeUndefined()
  })

  it('counts in-memory tasks against the active limits', async () => {
    const limit = TASK_LIMITS.activePerOwner
    TASK_LIMITS.activePerOwner = 1
    try {
      await store.create('owner', 'Bash', { command: 'one' }, { durable: false })
      await expect(store.create('owner', 'Bash', { command: 'two' }, { durable: false })).rejects.toMatchObject({
        reason: 'active_owner',
      })
    } finally {
      TASK_LIMITS.activePerOwner = limit
    }
  })
})

// Real file I/O and production limits; logical time, no user commands or account.
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { promises as fs } from 'node:fs'
import path from 'node:path'
import os from 'node:os'
import { McpTaskStore, TASK_LIMITS } from './mcp-task-store'

let directory: string
let clock: number
let store: McpTaskStore
const limits = { ...TASK_LIMITS }
beforeEach(async () => {
  directory = await fs.mkdtemp(path.join(os.tmpdir(), 'kamin-retention-contract-'))
  clock = Date.now()
  store = new McpTaskStore(directory, () => clock)
})
afterEach(async () => {
  vi.restoreAllMocks()
  Object.assign(TASK_LIMITS, limits)
  if (!directory.startsWith(path.join(os.tmpdir(), 'kamin-retention-contract-')))
    throw new Error('Unexpected directory')
  await fs.rm(directory, { recursive: true, force: true })
})

it.each(['completed', 'failed', 'cancelled'] as const)(
  'preserves unread %s outcomes through actual owner pressure, restart and TTL',
  async (status) => {
    const first = await store.create('owner', 'Bash', {})
    if (status === 'cancelled')
      await store.update('owner', first.taskId, (t) => {
        t.cancelRequested = true
      })
    await store.settle(
      'owner',
      first.taskId,
      'unread original output',
      status === 'failed' ? 'original denial' : undefined,
    )
    for (let i = 1; i < TASK_LIMITS.perOwner; i++) {
      clock++
      const task = await store.create('owner', 'Bash', {})
      await store.settle('owner', task.taskId, 'later output')
    }
    await expect(store.create('owner', 'Bash', {})).rejects.toMatchObject({
      reason: 'retained_owner',
    })
    store = new McpTaskStore(directory, () => clock)
    const before = await store.get('owner', first.taskId)
    expect(before?.status).toBe(status)
    if (status === 'completed')
      expect(before?.result?.content).toEqual([{ type: 'text', text: 'unread original output' }])
    if (status === 'failed') expect(before?.error?.message).toBe('original denial')
    expect(await store.settle('owner', first.taskId, 'duplicate must not replace output')).toEqual(before)
    expect(await store.get('other', first.taskId)).toBeUndefined()
    clock = Date.parse(first.createdAt) + first.ttlMs - 1
    expect(await store.get('owner', first.taskId)).toEqual(before)
    clock++
    expect(await store.get('owner', first.taskId)).toBeUndefined()
    await expect(store.create('owner', 'Bash', {})).resolves.toHaveProperty('taskId')
  },
  60_000,
)

it('preserves the first unread result at the actual global retained limit', async () => {
  let first = ''
  for (let i = 0; i < TASK_LIMITS.retained; i++) {
    const owner = 'owner-' + (i % 20)
    const task = await store.create(owner, 'PowerShell', {})
    if (i === 0) first = task.taskId
    await store.settle(owner, task.taskId, 'result-' + i)
    clock++
  }
  await expect(store.create('fresh', 'Bash', {})).rejects.toMatchObject({
    reason: 'retained_global',
  })
  const release = await store.acquireInline('fresh')
  release()
  store = new McpTaskStore(directory, () => clock)
  expect((await store.get('owner-0', first))?.result?.content).toEqual([{ type: 'text', text: 'result-0' }])
}, 60_000)

it('reserves worst-case terminal bytes across restart so admitted results can settle', async () => {
  TASK_LIMITS.bytes = 2 * (TASK_LIMITS.resultBytes + TASK_LIMITS.inputBytes + 8192)
  const command = 'x'.repeat(TASK_LIMITS.inputBytes - 100)
  const a = await store.create('a', 'Bash', { command })
  const b = await store.create('b', 'PowerShell', { command })
  store = new McpTaskStore(directory, () => clock)
  await expect(store.create('c', 'Bash', {})).rejects.toMatchObject({
    reason: 'retained_bytes',
  })
  const output = 'y'.repeat(TASK_LIMITS.resultBytes - 100)
  await store.settle('a', a.taskId, output)
  await store.settle('b', b.taskId, output)
  store = new McpTaskStore(directory, () => clock)
  expect((await store.get('a', a.taskId))?.result?.content).toEqual([{ type: 'text', text: output }])
  expect((await store.get('b', b.taskId))?.result?.content).toEqual([{ type: 'text', text: output }])
})

it('keeps terminal payloads out of the RAM projection and preserves them when history is cleared', async () => {
  const task = await store.create('owner', 'Bash', {})
  await store.settle('owner', task.taskId, {
    exitCode: 0,
    content: [{ type: 'text', text: 'kept on disk' }],
  })
  const read = vi.spyOn(fs, 'readFile')
  expect((await store.commands('owner'))[0]?.result).toEqual({
    isError: undefined,
    exitCode: 0,
  })
  expect(read).not.toHaveBeenCalled()
  expect((await store.get('owner', task.taskId))?.result?.content).toEqual([{ type: 'text', text: 'kept on disk' }])
  expect(read).toHaveBeenCalledTimes(1)
  await store.clearCommands('owner')
  store = new McpTaskStore(directory, () => clock)
  expect(await store.commands('owner')).toEqual([])
  expect((await store.get('owner', task.taskId))?.result?.content).toEqual([{ type: 'text', text: 'kept on disk' }])
})

it('counts ordinary leases with durable work and releases them idempotently', async () => {
  Object.assign(TASK_LIMITS, { active: 3, activePerOwner: 2 })
  await store.create('a', 'Bash', {})
  const releaseA = await store.acquireInline('a')
  await expect(store.create('a', 'Bash', {})).rejects.toMatchObject({
    reason: 'active_owner',
  })
  await expect(store.acquireInline('a')).rejects.toMatchObject({
    reason: 'active_owner',
  })
  const releaseB = await store.acquireInline('b')
  await expect(store.create('c', 'Bash', {})).rejects.toMatchObject({
    reason: 'active_global',
  })
  releaseA()
  releaseA()
  await expect(store.create('c', 'Bash', {})).resolves.toHaveProperty('taskId')
  releaseB()
})

it('retains other results and an unsettled record through a failed atomic write', async () => {
  const old = await store.create('owner', 'Bash', {})
  await store.settle('owner', old.taskId, 'old output')
  const next = await store.create('owner', 'Bash', {})
  vi.spyOn(fs, 'rename').mockRejectedValueOnce(new Error('synthetic write failure'))
  await expect(store.settle('owner', next.taskId, 'next output')).rejects.toThrow('synthetic write failure')
  expect((await store.get('owner', next.taskId))?.status).toBe('working')
  expect((await store.get('owner', old.taskId))?.result?.content).toEqual([{ type: 'text', text: 'old output' }])
  expect((await store.settle('owner', next.taskId, 'next output'))?.status).toBe('completed')
})

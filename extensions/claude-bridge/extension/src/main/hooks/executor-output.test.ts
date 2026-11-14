import { EventEmitter } from 'node:events'
import { PassThrough } from 'node:stream'
import { afterEach, expect, it, vi } from 'vitest'

// BR-18B: the result was built on `exit`, which can arrive before the last
// output. A hook that exited 0 was then reported with an empty stdout, and its
// JSON decision was lost.

class FakeProcess extends EventEmitter {
  stdout = new PassThrough()
  stderr = new PassThrough()
  stdin = new PassThrough()
  pid = 4242
  kill = vi.fn()
}
let fake: FakeProcess
vi.mock('node:child_process', async (original) => ({
  ...(await original<typeof import('node:child_process')>()),
  spawn: vi.fn(() => fake),
}))
import { executeHook, HOOK_OUTPUT_DRAIN_MS, type HookExecuteRequest } from './executor'

afterEach(() => vi.useRealTimers())

function run(): Promise<Awaited<ReturnType<typeof executeHook>>> {
  fake = new FakeProcess()
  return executeHook({
    type: 'hook:execute', requestId: 'r', sessionId: 's', hookId: 'h', event: 'PreToolUse',
    command: process.execPath, args: [], cwd: process.cwd(),
    payload: { hook_event_name: 'PreToolUse' }, timeoutMs: 60_000,
  } as HookExecuteRequest)
}

const deny = '{"hookSpecificOutput":{"permissionDecision":"deny"}}'

it('reports the output that arrives after exit and before close', async () => {
  vi.useFakeTimers()
  const result = run()
  await vi.advanceTimersByTimeAsync(0)
  fake.emit('exit', 0, null)
  fake.stdout.write(deny)
  fake.stderr.write('late')
  await vi.advanceTimersByTimeAsync(0)
  fake.emit('close', 0, null)
  const r = await result
  expect(r).toMatchObject({ exitCode: 0, outcome: 'success', stdout: deny, stderr: 'late' })
  expect(r.jsonOutput).toEqual(JSON.parse(deny))
})

it('reports an exited hook after the window when its pipes stay open', async () => {
  vi.useFakeTimers()
  let settled = false
  const result = run().then((r) => { settled = true; return r })
  await vi.advanceTimersByTimeAsync(0)
  fake.stdout.write('before')
  fake.emit('exit', 3, null)
  await vi.advanceTimersByTimeAsync(HOOK_OUTPUT_DRAIN_MS - 1)
  expect(settled).toBe(false)
  await vi.advanceTimersByTimeAsync(1)
  expect(await result).toMatchObject({ exitCode: 3, outcome: 'error', stdout: 'before' })
})

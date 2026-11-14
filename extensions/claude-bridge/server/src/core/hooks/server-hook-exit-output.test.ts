import 'reflect-metadata'
import { EventEmitter } from 'node:events'
import { PassThrough } from 'node:stream'
import { afterEach, expect, it, vi } from 'vitest'

// BR-18B: the result was built on `exit`, which can arrive before the last
// output; a hook that exited 0 could lose its stdout and its JSON decision.

class FakeProcess extends EventEmitter {
  stdout = new PassThrough()
  stderr = new PassThrough()
  stdin = new PassThrough()
  kill = vi.fn()
}
let fake: FakeProcess
vi.mock('node:child_process', async (original) => ({
  ...(await original<typeof import('node:child_process')>()),
  spawn: vi.fn(() => fake),
}))
import { dispatchHook, HOOK_OUTPUT_DRAIN_MS } from './dispatcher'
import { clearSession, listSession, registerSessionHooks } from './registry'

const SESSION = 'server-hook-exit-output'
afterEach(() => {
  clearSession(SESSION)
  vi.useRealTimers()
})

function run() {
  fake = new FakeProcess()
  registerSessionHooks(
    SESSION,
    { PreToolUse: [{ matcher: 'Read', hooks: [{ type: 'command', command: 'synthetic', host: 'server' }] }] },
    { kind: 'user' },
  )
  const hook = listSession(SESSION).at(-1)!
  return dispatchHook(SESSION, hook.id, { hook_event_name: 'PreToolUse' } as never, {} as never)
}

const deny = '{"hookSpecificOutput":{"permissionDecision":"deny"}}'

it('reports the output that arrives after exit and before close', async () => {
  vi.useFakeTimers()
  const result = run()
  await vi.advanceTimersByTimeAsync(0)
  fake.emit('exit', 0, null)
  fake.stdout.write(deny)
  await vi.advanceTimersByTimeAsync(0)
  fake.emit('close', 0, null)
  const r = await result
  expect(r).toMatchObject({ exitCode: 0, outcome: 'success', stdout: deny })
  expect(r.jsonOutput).toEqual(JSON.parse(deny))
})

it('reports an exited hook after the window when its pipes stay open', async () => {
  vi.useFakeTimers()
  let settled = false
  const result = run().then((r) => {
    settled = true
    return r
  })
  await vi.advanceTimersByTimeAsync(0)
  fake.stdout.write('before')
  fake.emit('exit', 3, null)
  await vi.advanceTimersByTimeAsync(HOOK_OUTPUT_DRAIN_MS - 1)
  expect(settled).toBe(false)
  await vi.advanceTimersByTimeAsync(1)
  expect(await result).toMatchObject({ exitCode: 3, outcome: 'error', stdout: 'before' })
})

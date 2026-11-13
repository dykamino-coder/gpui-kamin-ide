import 'reflect-metadata'
import { EventEmitter } from 'node:events'
import { afterEach, expect, it, vi } from 'vitest'

vi.mock('../logging', async (importOriginal) => ({
  ...(await importOriginal<typeof import('../logging')>()),
  debugLog: vi.fn(),
}))
import { debugLog } from '../logging'
import { dispatchHook, handleLocalHookResponse } from './dispatcher'
import { clearSession, listSession, registerSessionHooks } from './registry'

const SESSION = 'dispatcher-timing'
afterEach(() => clearSession(SESSION))

it('logs how long a client-host hook took, split from the round trip', async () => {
  registerSessionHooks(
    SESSION,
    { PreToolUse: [{ matcher: 'Read', hooks: [{ type: 'command', command: 'true', host: 'local' }] }] },
    { kind: 'user' },
  )
  const hook = listSession(SESSION).at(-1)!
  const ws = Object.assign(new EventEmitter(), {
    readyState: 1,
    send(frame: string) {
      const { requestId } = JSON.parse(frame) as { requestId: string }
      setTimeout(
        () =>
          handleLocalHookResponse(requestId, {
            stdout: '',
            stderr: '',
            exitCode: 0,
            outcome: 'success',
            durationMs: 7,
          }),
        20,
      )
    },
  })
  const result = await dispatchHook(
    SESSION,
    hook.id,
    { hook_event_name: 'PreToolUse', cwd: '/w' } as never,
    ws as never,
  )
  expect(result.exitCode).toBe(0)
  const entry = vi.mocked(debugLog).mock.calls.find(([msg]) => msg === 'Hook dispatched')
  expect(entry?.[1]).toMatchObject({
    sessionId: SESSION,
    event: 'PreToolUse',
    host: 'local',
    outcome: 'success',
    hostMs: 7,
  })
  expect((entry?.[1] as { totalMs: number }).totalMs).toBeGreaterThanOrEqual(15)
})

import { EventEmitter } from 'node:events'
import { afterEach, expect, it, vi } from 'vitest'
import { dispatchHook, handleLocalHookResponse } from './dispatcher'
import { rewriteHooksForCli } from './proxy-rewriter'
import { clearSession, listSession } from './registry'
import type { HookInputPayload, HookSource } from './types'

vi.mock('../config', () => ({ config: { port: 3030 } }))
vi.mock('../logging', () => ({ debugLog: vi.fn(), warnLog: vi.fn() }))
vi.mock('./audit-log', () => ({ appendExecution: vi.fn() }))

const session = 'settings-once'
const payload = (command: string): HookInputPayload => ({
  session_id: session,
  hook_event_name: 'PreToolUse',
  tool_name: 'mcp__user-tools__Bash',
  tool_input: { command },
})

afterEach(() => {
  clearSession(session)
  vi.restoreAllMocks()
})

// These sources all become CLI settings hooks, not skill-frontmatter hooks.
it.each<HookSource>([
  { kind: 'user' },
  { kind: 'user', client: true },
  { kind: 'project', projectPath: '/fixture' },
  { kind: 'plugin', pluginId: 'fixture', manifestPath: '/fixture/plugin.json' },
])('ignores once for conditional command settings from $kind, including parallel calls', async (source) => {
  rewriteHooksForCli(
    session,
    {
      PreToolUse: [{ hooks: [{ type: 'command', command: 'fixture', host: 'local', if: 'Bash(echo *)', once: true }] }],
    },
    source,
  )
  const hook = listSession(session)[0]!
  const send = vi.fn((raw: string) => {
    const frame = JSON.parse(raw)
    setImmediate(() =>
      handleLocalHookResponse(frame.requestId, {
        stdout: 'fixture',
        stderr: '',
        exitCode: 0,
        outcome: 'success',
        durationMs: 0,
      }),
    )
  })
  const ws = Object.assign(new EventEmitter(), { readyState: 1, send })
  await dispatchHook(session, hook.id, payload('pwd'), ws as never)
  expect(send).not.toHaveBeenCalled()
  const results = await Promise.all([
    dispatchHook(session, hook.id, payload('echo first'), ws as never),
    dispatchHook(session, hook.id, payload('echo second'), ws as never),
  ])
  expect(results.map((r) => r.stdout)).toEqual(['fixture', 'fixture'])
  expect(send).toHaveBeenCalledTimes(2)
  await dispatchHook(session, hook.id, payload('echo third'), ws as never)
  expect(send).toHaveBeenCalledTimes(3)
})

it.each([200, 403, 500])('keeps conditional HTTP settings active after status %s with once enabled', async (status) => {
  rewriteHooksForCli(
    session,
    {
      PreToolUse: [{ hooks: [{ type: 'http', url: 'http://fixture/hook', if: 'Bash(echo *)', once: true }] }],
    },
    { kind: 'user' },
  )
  const hook = listSession(session)[0]!
  const fetchMock = vi
    .spyOn(globalThis, 'fetch')
    .mockImplementation(async () => new Response('{"decision":"block"}', { status }))
  await dispatchHook(session, hook.id, payload('pwd'), null)
  expect(fetchMock).not.toHaveBeenCalled()
  const first = await dispatchHook(session, hook.id, payload('echo first'), null)
  const second = await dispatchHook(session, hook.id, payload('echo second'), null)
  expect(fetchMock).toHaveBeenCalledTimes(2)
  expect([first.httpStatus, second.httpStatus]).toEqual([status, status])
  expect([first.stdout, second.stdout]).toEqual(['{"decision":"block"}', '{"decision":"block"}'])
})

import 'reflect-metadata'
import { EventEmitter } from 'node:events'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { sessions } from '../pty/session-core'
import type { PtySession } from '../types/pty'
import { cancelSessionLocalExecs, handleLocalHookResponse } from './dispatcher'
import { commandLabel, httpHookAnswer } from './http-hook-answer'
import { clearSession, registerSessionHooks } from './registry'
import { createHooksRoutes } from './routes'
import type { HookExecutionResult } from './types'

const done = (exitCode: number, stdout = '', stderr = ''): HookExecutionResult => ({
  stdout,
  stderr,
  exitCode,
  outcome: exitCode === 0 ? 'success' : 'error',
  durationMs: 1,
})
const body = (event: string, r: HookExecutionResult) => {
  const a = httpHookAnswer(event, r, 'guard --strict')
  return { status: a.status, json: a.body === '' ? undefined : JSON.parse(a.body) }
}

// The expected bodies were checked against Claude Code 2.1.322: for each case
// the model received the same messages from the command hook and from its
// http form (see the PR for the comparison).
describe('httpHookAnswer', () => {
  it('passes JSON stdout of exit 0 through unchanged', () => {
    const out = '{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"ask"}}\n'
    expect(httpHookAnswer('PreToolUse', done(0, out), 'x')).toEqual({ status: 200, body: out })
    // Invalid JSON is passed too, so the CLI reports it as it does for a command.
    expect(httpHookAnswer('PostToolUse', done(0, '{oops'), 'x')).toEqual({ status: 200, body: '{oops' })
  })

  it('answers plain stdout of exit 0 with an empty object', () => {
    expect(body('PreToolUse', done(0, 'checked\n', 'note'))).toEqual({ status: 200, json: {} })
  })

  it('denies a PreToolUse call on exit 2 with the reason a command hook gets', () => {
    expect(body('PreToolUse', done(2, 'ignored', 'no secrets\n'))).toEqual({
      status: 200,
      json: {
        hookSpecificOutput: {
          hookEventName: 'PreToolUse',
          permissionDecision: 'deny',
          permissionDecisionReason: '[guard --strict]: no secrets\n',
        },
      },
    })
    expect(body('PreToolUse', done(2)).json.hookSpecificOutput.permissionDecisionReason).toBe(
      '[guard --strict]: No stderr output',
    )
  })

  it('blocks after PostToolUse on exit 2', () => {
    expect(body('PostToolUse', done(2, '', 'lint failed'))).toEqual({
      status: 200,
      json: { decision: 'block', reason: '[guard --strict]: lint failed' },
    })
  })

  it.each([
    ['exit 1', done(1, '', 'boom')],
    ['exit 3', done(3)],
    ['a timeout', { ...done(2, '', 'late'), outcome: 'timeout' as const }],
    ['a cancel', { ...done(0), outcome: 'cancelled' as const }],
  ])('reports %s as a non-blocking error', (_name, result) => {
    expect(httpHookAnswer('PreToolUse', result, 'x').status).toBe(500)
  })

  it('names exec-form hooks by command and arguments', () => {
    expect(commandLabel({ type: 'command', command: 'node', args: ['guard.js', '--strict'] })).toBe(
      'node guard.js --strict',
    )
  })
})

describe('http hook route', () => {
  const id = 'http-hook-route'
  const app = createHooksRoutes()
  afterEach(() => {
    cancelSessionLocalExecs(id)
    clearSession(id)
    sessions.delete(id)
  })

  it('answers the http form of a client hook in the http-hook shape', async () => {
    const ws = Object.assign(new EventEmitter(), {
      readyState: 1,
      send: vi.fn((raw: string) => {
        const frame = JSON.parse(raw)
        handleLocalHookResponse(frame.requestId, done(2, '', 'denied'))
      }),
    })
    sessions.set(id, { id, ws, mcpToken: 'synthetic', cwd: '/client', state: 'running' } as unknown as PtySession)
    const registered = registerSessionHooks(
      id,
      { PreToolUse: [{ matcher: 'Bash', hooks: [{ type: 'command', command: 'guard', host: 'local' }] }] },
      { kind: 'user' },
    )
    const hookId = [...registered.values()][0]!.id
    const res = await app.request(`/api/hooks/${id}/PreToolUse/${hookId}`, {
      method: 'POST',
      headers: { Authorization: 'Bearer synthetic', 'X-Bridge-Hook-Relay': 'http', 'Content-Type': 'application/json' },
      body: JSON.stringify({ hook_event_name: 'PreToolUse', tool_name: 'mcp__bridge__Bash' }),
    })
    expect(res.status).toBe(200)
    expect(await res.json()).toEqual({
      hookSpecificOutput: {
        hookEventName: 'PreToolUse',
        permissionDecision: 'deny',
        permissionDecisionReason: '[guard]: denied',
      },
    })
  })
})

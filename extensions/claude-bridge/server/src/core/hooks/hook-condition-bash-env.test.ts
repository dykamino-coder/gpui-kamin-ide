import 'reflect-metadata'
import { EventEmitter } from 'node:events'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { sessions } from '../pty/session-core'
import type { PtySession } from '../types/pty'
import { handleLocalHookResponse } from './dispatcher'
import { conditionHolds } from './hook-condition'
import { BASH_HOME_HEADER, rewriteHooksForCli } from './proxy-rewriter'
import { BASH_HOME_ENV } from './hook-environment'
import { clearSession, listSession } from './registry'
import { createHooksRoutes } from './routes'
import { bashRuleMatches } from './rule-matchers'

vi.mock('./audit-log', () => ({ appendExecution: vi.fn() }))
const sessionId = 'bash-cli-home'
afterEach(() => {
  clearSession(sessionId)
  sessions.delete(sessionId)
})

describe('Bash hook conditions in the CLI environment', () => {
  it.each(['echo "$HOME"', 'echo $HOME'])('expands CLI HOME in %s independently of client home', (command) => {
    const payload = {
      hook_event_name: 'PreToolUse',
      session_id: 'fixture',
      tool_name: 'mcp__user-tools__Bash',
      tool_input: { command },
    }
    const context = { home: 'C:\\Users\\client', bashHome: '/home/cli-at-startup' }
    expect(conditionHolds({ tool: 'Bash', content: 'echo /home/cli-at-startup' }, payload, context)).toBe(true)
    expect(conditionHolds({ tool: 'Bash', content: 'echo $HOME' }, payload, context)).toBe(false)
    expect(conditionHolds({ tool: 'Bash', content: 'echo C:*' }, payload, context)).toBe(false)
  })

  it.each(['echo "$CUSTOM_FROM_SETTINGS"', 'echo $CUSTOM_FROM_SETTINGS'])(
    'keeps native conservative matching for %s',
    (command) => {
      expect(bashRuleMatches('unrelated *', command)).toBe(true)
    },
  )

  it('does not expand single-quoted or escaped variables', () => {
    expect(bashRuleMatches('echo $HOME', "echo '$HOME'")).toBe(true)
    expect(bashRuleMatches('echo $HOME', 'echo \\$HOME')).toBe(true)
    expect(bashRuleMatches('unrelated *', "echo '$CUSTOM_FROM_SETTINGS'")).toBe(false)
  })

  it('carries the native HTTP hook HOME into the predicate without forwarding it to the client', async () => {
    const ws = Object.assign(new EventEmitter(), {
      readyState: 1,
      send: vi.fn((raw: string) => {
        const frame = JSON.parse(raw)
        handleLocalHookResponse(frame.requestId, {
          stdout: '',
          stderr: '',
          exitCode: 0,
          outcome: 'success',
          durationMs: 1,
        })
      }),
    })
    sessions.set(sessionId, {
      id: sessionId,
      ws,
      mcpToken: 'synthetic',
      cwd: '/client',
      state: 'running',
      host: { home: '/client-home' },
    } as unknown as PtySession)
    const hooks = rewriteHooksForCli(
      sessionId,
      {
        PreToolUse: [
          {
            matcher: 'Bash',
            hooks: [{ type: 'command', command: 'fixture', host: 'local', if: 'Bash(echo /effective-cli-home)' }],
          },
        ],
      },
      { kind: 'user' },
    )
    const rewritten = hooks.PreToolUse![0]!.hooks[0]!
    expect(rewritten.type).toBe('http')
    if (rewritten.type !== 'http') throw new Error('Expected HTTP relay')
    expect(rewritten.headers?.[BASH_HOME_HEADER]).toBe(`$${BASH_HOME_ENV}`)
    expect(rewritten.allowedEnvVars).toContain(BASH_HOME_ENV)
    const app = createHooksRoutes()
    for (const command of ['echo "$HOME"', 'pwd']) {
      const response = await app.request(`/api/hooks/${sessionId}/PreToolUse/${listSession(sessionId)[0]!.id}`, {
        method: 'POST',
        headers: {
          Authorization: 'Bearer synthetic',
          'X-Bridge-Hook-Relay': 'http',
          [BASH_HOME_HEADER]: '/effective-cli-home',
          'Content-Type': 'application/json',
        },
        body: JSON.stringify({
          hook_event_name: 'PreToolUse',
          session_id: sessionId,
          tool_name: 'mcp__user-tools__Bash',
          tool_input: { command },
        }),
      })
      expect(response.status).toBe(200)
    }
    expect(ws.send).toHaveBeenCalledTimes(1)
    expect(JSON.parse(ws.send.mock.calls[0]![0]).payload).not.toHaveProperty('bashHome')
  })
})

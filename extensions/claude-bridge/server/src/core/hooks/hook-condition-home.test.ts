import 'reflect-metadata'
import { EventEmitter } from 'node:events'
import { afterEach, expect, it, vi } from 'vitest'
import { sessions } from '../pty/session-core'
import { normalizeHostDeclaration } from '../pty/host-declaration'
import type { PtySession } from '../types/pty'
import { cancelSessionLocalExecs, handleLocalHookResponse } from './dispatcher'
import { rewriteHooksForCli } from './proxy-rewriter'
import { clearSession, listSession } from './registry'
import { createHooksRoutes } from './routes'

vi.mock('./audit-log', () => ({ appendExecution: vi.fn() }))

const id = 'client-home-condition'
const app = createHooksRoutes()

afterEach(() => {
  cancelSessionLocalExecs(id)
  clearSession(id)
  sessions.delete(id)
})

it.each([
  ['Read', '/home/client with spaces', '/workspace', '/home/client with spaces/notes/a.md'],
  ['Edit', '/home/client', '/workspace', '/home/client/notes/a.md'],
  ['Write', String.raw`C:\Users\client`, String.raw`D:\workspace`, String.raw`C:\Users\client\notes\a.md`],
  ['Read', '/home/client with spaces', '', 'notes/a.md'],
  ['Edit', '/home/client', '', 'notes/a.md'],
  ['Write', String.raw`C:\Users\client`, '', String.raw`notes\a.md`],
  ['NotebookEdit', String.raw`C:\Users\client with spaces`, '', 'notes/a.md'],
])(
  'evaluates %s home rules using the declared client home on each matching settings call',
  async (tool, home, cwd, filePath) => {
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
    sessions.set(id, {
      id,
      ws,
      mcpToken: 'synthetic',
      cwd,
      state: 'running',
      host: normalizeHostDeclaration({ tools: [tool], home }),
    } as unknown as PtySession)
    rewriteHooksForCli(
      id,
      {
        PreToolUse: [
          {
            matcher: tool,
            hooks: [
              {
                type: 'command',
                command: 'fixture',
                host: 'local',
                if: `${tool}(~/notes/*.md)`,
                once: true,
              },
            ],
          },
        ],
      },
      { kind: 'user' },
    )
    const hookId = listSession(id)[0]!.id
    const request = (file_path: string) =>
      app.request(`/api/hooks/${id}/PreToolUse/${hookId}`, {
        method: 'POST',
        headers: {
          Authorization: 'Bearer synthetic',
          'X-Bridge-Hook-Relay': 'http',
          'Content-Type': 'application/json',
        },
        body: JSON.stringify({
          hook_event_name: 'PreToolUse',
          tool_name: `mcp__user-tools__${tool}`,
          tool_input: { [tool === 'NotebookEdit' ? 'notebook_path' : 'file_path']: file_path },
          cwd: '/server',
          home: '/forged-home',
        }),
      })

    expect((await request('/other-user/notes/a.md')).status).toBe(200)
    expect(ws.send).not.toHaveBeenCalled()
    expect((await request(filePath)).status).toBe(200)
    expect(ws.send).toHaveBeenCalledTimes(1)
    expect((await request(filePath)).status).toBe(200)
    expect(ws.send).toHaveBeenCalledTimes(2)
  },
)

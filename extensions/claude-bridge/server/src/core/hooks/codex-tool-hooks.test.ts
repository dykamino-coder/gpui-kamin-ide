import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { dispatchHook } from './dispatcher'
import { clearSession } from './registry'
import { codexToolCall, HOOK_SYNC_ERROR, registerCodexToolHooks } from './codex-tool-hooks'
import type { PtySession } from '../pty/types'
import type { HookExecutionResult } from './types'

const paths = vi.hoisted(() => ({ home: '' }))
vi.mock('../sync/storage', () => ({
  getUserSyncDir: (owner: string) => path.join(paths.home, owner),
  getProjectSyncDir: (owner: string, project: string) => path.join(paths.home, owner, project),
}))
vi.mock('./dispatcher', () => ({ dispatchHook: vi.fn() }))
const local = { type: 'command', command: 'reviewed-guard' }
const success = { exitCode: 0, outcome: 'success' as const, stdout: '', stderr: '', durationMs: 1 }
const chat = (id: string, owner: string, cwd: string, protocol?: number) =>
  ({
    id,
    bearerHash: owner,
    cwd,
    engine: 'codex',
    launchConfig: { codexHookProtocol: protocol },
  }) as PtySession
let sessions: [PtySession, PtySession, PtySession]
function write(parts: string[], data: unknown) {
  const folder = path.join(paths.home, ...parts)
  fs.mkdirSync(folder, { recursive: true })
  fs.writeFileSync(path.join(folder, 'settings.json'), JSON.stringify(data))
}
function guard(event: string, matcher: string) {
  return { hooks: { [event]: [{ matcher, hooks: [local] }] } }
}
beforeEach(() => {
  sessions = [chat('one', 'owner', 'a', 1), chat('two', 'owner', 'b'), chat('three', 'other', 'a')]
  paths.home = fs.mkdtempSync(path.join(os.tmpdir(), 'bridge-tool-hooks-'))
  vi.mocked(dispatchHook).mockReset().mockResolvedValue(success)
})
afterEach(() => {
  sessions.forEach((s) => clearSession(s.id))
  fs.rmSync(paths.home, { recursive: true, force: true })
})

describe('workspace tool hook policy', () => {
  it('isolates projects and owners, preserves neutral legacy sessions, and blocks a matching legacy command', async () => {
    write(['owner', 'a'], guard('PreToolUse', 'Grep'))
    write(['owner'], guard('PreToolUse', 'Read'))
    for (const session of sessions) registerCodexToolHooks(session)
    const [a, b, c] = sessions
    expect(await codexToolCall(a, 'Grep', {})?.before()).toContain('project commands')
    expect(codexToolCall(b, 'Grep', {})).toBeUndefined()
    expect(await codexToolCall(b, 'Read', {})?.before()).toContain('update the client')
    expect(codexToolCall(c, 'Read', {})).toBeUndefined()
    expect(dispatchHook).not.toHaveBeenCalled()
    expect(await codexToolCall(a, 'Read', {})?.before()).toBeUndefined()
    expect(dispatchHook).toHaveBeenCalledOnce()
    expect(codexToolCall({ ...a, engine: 'claude' }, 'Read', {})).toBeUndefined()
  })

  it('honors a user policy when only the project disabled its hooks', async () => {
    const a = sessions[0]!
    write(['owner'], guard('PreToolUse', 'Read'))
    write(['owner', 'a'], { ...guard('PreToolUse', 'Grep'), disableAllHooks: true })
    registerCodexToolHooks(a)
    expect(codexToolCall(a, 'Grep', {})).toBeUndefined()
    await codexToolCall(a, 'Read', {})?.before()
    expect(dispatchHook).toHaveBeenCalledOnce()
  })

  it('keeps guarded command results in the foreground so the post hook sees the output before release', async () => {
    const a = sessions[0]!
    write(['owner'], guard('PostToolUse', 'Bash'))
    registerCodexToolHooks(a)
    const background = codexToolCall(a, 'Bash', { command: 'echo sample', run_in_background: true })!
    expect(background.mustFinishInline).toBe(true)
    expect(await background.before()).toContain('foreground')
    expect(await codexToolCall(a, 'Bash', { command: 'echo sample' })?.before()).toBeUndefined()
  })

  it.each(['timeout', 'error', 'cancelled'] as const)(
    'withholds tool execution and results for a %s hook',
    async (outcome) => {
      const a = sessions[0]!
      const group = [{ matcher: 'Read', hooks: [local] }]
      write(['owner'], { hooks: { PreToolUse: group, PostToolUse: group, PostToolUseFailure: group } })
      registerCodexToolHooks(a)
      vi.mocked(dispatchHook).mockResolvedValue({ ...success, outcome, exitCode: 1 })
      const invoke = codexToolCall(a, 'Read', {})!
      expect(await invoke.before()).toContain('Tool was not run')
      expect(await invoke.after({ content: [] })).toMatchObject({ isError: true })
      expect(await invoke.after({ isError: true })).toMatchObject({ isError: true })
      expect(vi.mocked(dispatchHook).mock.calls.map((c) => c[2].hook_event_name)).toEqual([
        'PreToolUse',
        'PostToolUse',
        'PostToolUseFailure',
      ])
    },
  )

  it('detects policy changes while a hook is pending and invalidates malformed snapshots until repaired', async () => {
    const a = sessions[0]!
    write(['owner'], guard('PreToolUse', 'Read'))
    registerCodexToolHooks(a)
    let complete!: (r: HookExecutionResult) => void
    vi.mocked(dispatchHook).mockReturnValue(
      new Promise((resolve) => {
        complete = resolve
      }),
    )
    const pending = codexToolCall(a, 'Read', {})!.before()
    write(['owner'], guard('PreToolUse', 'Write'))
    registerCodexToolHooks(a)
    complete(success)
    expect(await pending).toContain('configuration changed')
    write(['owner'], { hooks: { PreToolUse: [{ matcher: 'Read', hooks: null }] } })
    expect(() => registerCodexToolHooks(a)).toThrow()
    expect(await codexToolCall(a, 'Read', {})?.before()).toBe(HOOK_SYNC_ERROR)
    write(['owner'], {})
    registerCodexToolHooks(a)
    expect(codexToolCall(a, 'Read', {})).toBeUndefined()
  })
})

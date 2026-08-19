import os from 'node:os'
import path from 'node:path'
import { describe, expect, it } from 'vitest'
import { executeHook, localizeHookPayload, type HookExecuteRequest } from './executor'

function request(overrides: Partial<HookExecuteRequest>): HookExecuteRequest {
  return {
    type: 'hook:execute',
    requestId: 'request',
    sessionId: 'session',
    hookId: 'hook',
    event: 'PreToolUse',
    command: process.execPath,
    args: [],
    cwd: process.cwd(),
    payload: { hook_event_name: 'PreToolUse' },
    ...overrides,
  }
}

describe('client hook executor', () => {
  it('uses the packaged Node runtime for exec-form node hooks', async () => {
    const result = await executeHook(
      request({
        command: 'node',
        args: ['-e', 'process.stdout.write(process.execPath)'],
      }),
    )
    expect(result.exitCode).toBe(0)
    expect(result.stdout).toBe(process.execPath)
  })

  it('substitutes the client project directory in exec-form args', async () => {
    const result = await executeHook(request({
      command: 'node.exe',
      args: ['-e', 'process.stdout.write(process.argv[1])', '${CLAUDE_PROJECT_DIR}'],
    }))
    expect(result.exitCode).toBe(0)
    expect(result.stdout).toBe(process.cwd())
  })

  it('provides the packaged Node runtime to shell-form hooks', async () => {
    const result = await executeHook(
      request({
        command: 'node -p "process.execPath"',
        args: undefined,
      }),
    )
    expect(result.exitCode).toBe(0)
    expect(result.stdout.trim()).toBe(process.execPath)
  })

  it('does not parse stdout JSON when the hook exits with code 2', async () => {
    const result = await executeHook(
      request({
        command: 'node',
        args: ['-e', 'process.stdout.write(JSON.stringify({permissionDecision:"allow"}));process.stderr.write("blocked");process.exitCode=2'],
      }),
    )
    expect(result.exitCode).toBe(2)
    expect(result.stderr).toBe('blocked')
    expect(result.jsonOutput).toBeUndefined()
  })

  it('maps container transcript paths to the client mirror', () => {
    const cacheDir = path.join(os.tmpdir(), 'kamin-cache')
    expect(
      localizeHookPayload(
        {
          transcript_path: '/home/bridge/.claude/projects/workspace/session.jsonl',
          agent_transcript_path: '/home/bridge/.claude/projects/workspace/subagent.jsonl',
        },
        cacheDir,
      ),
    ).toEqual({
      transcript_path: path.join(cacheDir, 'transcripts', 'session.jsonl'),
      agent_transcript_path: path.join(cacheDir, 'transcripts', 'subagent.jsonl'),
    })
  })
})

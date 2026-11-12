import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { describe, expect, it } from 'vitest'
import { executeHook, localizeHookPayload, resolveHookCwd, type HookExecuteRequest } from './executor'

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

  it('runs a shell-form hook longer than any argv limit completely, with its stdin payload', async () => {
    const body = 'Ж'.repeat(70_000)
    const result = await executeHook(
      request({
        command: `: '${body}'\nread -r payload; echo "$payload" | grep -c PreToolUse; echo TAIL_ХВОСТ; exit 5`,
        args: undefined,
        shell: 'bash',
      }),
    )
    expect(result.stdout).toContain('1\n')
    expect(result.stdout).toContain('TAIL_ХВОСТ')
    expect(result.exitCode).toBe(5)
  }, 60_000)

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

  const containerDir = '/home/bridge/.claude/bridge-sessions/token/session'
  const printCwd = 'let s="";process.stdin.on("data",d=>s+=d).on("end",()=>process.stdout.write(JSON.stringify([process.cwd(),JSON.parse(s).cwd,process.env.CLAUDE_PROJECT_DIR])))'

  it('runs a hook in the home directory when the requested one does not exist here', async () => {
    const result = await executeHook(
      request({ command: 'node', args: ['-e', printCwd], cwd: containerDir, payload: { cwd: containerDir } }),
    )
    expect(result.stderr).toBe('')
    expect(result.exitCode).toBe(0)
    expect(JSON.parse(result.stdout)).toEqual([os.homedir(), os.homedir(), os.homedir()])
  })

  it('starts a shell-form hook although the requested directory does not exist here', async () => {
    const result = await executeHook(request({ command: 'echo started', args: undefined, cwd: containerDir }))
    expect(result.stderr).not.toContain('ENOENT')
    expect(result.exitCode).toBe(0)
    expect(result.stdout.trim()).toBe('started')
  })

  it('prefers the tab working directory over home when the request has no usable directory', async () => {
    const tabDir = fs.mkdtempSync(path.join(os.tmpdir(), 'kamin-hook-cwd-'))
    try {
      for (const cwd of [undefined, containerDir]) {
        const result = await executeHook(request({ command: 'node', args: ['-e', printCwd], cwd }), tabDir)
        expect(JSON.parse(result.stdout).map((dir: string) => fs.realpathSync(dir))).toEqual(
          Array(3).fill(fs.realpathSync(tabDir)),
        )
      }
      expect(resolveHookCwd(process.cwd(), tabDir)).toBe(process.cwd())
      expect(resolveHookCwd(path.join(tabDir, 'missing'), path.join(tabDir, 'gone'))).toBe(os.homedir())
    } finally {
      fs.rmSync(tabDir, { recursive: true, force: true })
    }
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

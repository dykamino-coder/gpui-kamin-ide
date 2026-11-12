import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { afterAll, describe, expect, it } from 'vitest'
import { bashExecute, powerShellExecute } from './shell'
import { disposeWarmShells } from './warm-shell'
import { executeHook } from '../../hooks/executor'

// A pre-started shell must be indistinguishable from a cold one: same folder,
// empty stdin, no positional parameters, exit status and stderr intact.
const text = (r: { content: Array<{ text?: string }> }): string => r.content.map((c) => c.text ?? '').join('')
afterAll(() => disposeWarmShells())

describe('warm shell pool', () => {
  it('runs consecutive Bash calls with cold-start semantics', async () => {
    const cwd = fs.mkdtempSync(path.join(os.tmpdir(), 'warm-bash-'))
    try {
      const outputs: string[] = []
      for (let i = 0; i < 3; i++) {
        const r = await bashExecute({ command: `echo run${i}; echo "args=$#"; cat; pwd -P; echo err >&2; exit ${i}` }, { cwd })
        outputs.push(text(r))
        expect(r.exitCode).toBe(i)
      }
      for (const [i, out] of outputs.entries()) {
        expect(out).toContain(`run${i}\nargs=0\n`)
        expect(out).toContain(path.basename(cwd))
        expect(out).toContain('--- stderr ---\nerr')
      }
    } finally {
      disposeWarmShells()
      fs.rmSync(cwd, { recursive: true, force: true })
    }
  }, 60_000)

  it.runIf(process.platform === 'win32')('runs consecutive PowerShell calls with their exit status', async () => {
    const cwd = fs.mkdtempSync(path.join(os.tmpdir(), 'warm-pwsh-'))
    try {
      for (let i = 0; i < 3; i++) {
        const r = await powerShellExecute({ command: `Write-Output "run${i}"; (Get-Location).Path; exit ${i}` }, { cwd })
        expect(text(r)).toContain(`run${i}`)
        expect(text(r)).toContain(path.basename(cwd))
        expect(r.exitCode).toBe(i)
      }
    } finally {
      disposeWarmShells()
      fs.rmSync(cwd, { recursive: true, force: true })
    }
  }, 60_000)

  it('runs consecutive Bash hooks with their JSON input, folder, stderr and exit status', async () => {
    const cwd = fs.mkdtempSync(path.join(os.tmpdir(), 'warm-hook-'))
    try {
      for (let i = 0; i < 3; i++) {
        const r = await executeHook(
          {
            type: 'hook:execute', requestId: `h${i}`, sessionId: 's', hookId: 'k', event: 'PreToolUse',
            command: `cat; echo; pwd -P; echo err >&2; exit ${i * 2}`,
            payload: { hook_event_name: 'PreToolUse', n: i },
            timeoutMs: 30_000,
          } as Parameters<typeof executeHook>[0],
          cwd,
        )
        expect(r.exitCode).toBe(i * 2)
        expect(r.stdout).toContain(`"n":${i}`)
        expect(r.stdout).toContain(path.basename(cwd))
        expect(r.stderr).toContain('err')
      }
    } finally {
      disposeWarmShells()
      fs.rmSync(cwd, { recursive: true, force: true })
    }
  }, 60_000)
})
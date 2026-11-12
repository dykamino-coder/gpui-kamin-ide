import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { afterAll, describe, expect, it } from 'vitest'
import { bashExecute, powerShellExecute } from './shell'
import { disposeWarmShells } from './warm-shell'

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
})

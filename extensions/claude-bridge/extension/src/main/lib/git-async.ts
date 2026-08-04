// ============================================================================
// Promise-based git invocation. execFileSync blocks the main process event
// loop — while a 30s clone is in flight, the window message pump stalls and
// Windows flags the app "Not Responding". This module exposes an async
// variant that yields between chunks of git output.
// ============================================================================

import { spawn, type SpawnOptions } from 'child_process'

export interface GitRunResult {
  stdout: string
  stderr: string
  code: number
}

export interface GitRunError extends Error {
  stdout: string
  stderr: string
  code: number | null
  signal: NodeJS.Signals | null
}

export function runGit(
  args: string[],
  opts: { cwd?: string; timeoutMs?: number; env?: NodeJS.ProcessEnv } = {},
): Promise<GitRunResult> {
  const spawnOpts: SpawnOptions = {
    cwd: opts.cwd,
    windowsHide: true,
    stdio: ['ignore', 'pipe', 'pipe'],
    env: opts.env ?? {
      ...process.env,
      GIT_TERMINAL_PROMPT: '0',
      GIT_ASKPASS: '',
    },
  }

  return new Promise<GitRunResult>((resolve, reject) => {
    const proc = spawn('git', args, spawnOpts)
    let stdout = ''
    let stderr = ''
    let timedOut = false
    const timeout = opts.timeoutMs
      ? setTimeout(() => { timedOut = true; try { proc.kill('SIGKILL') } catch {} }, opts.timeoutMs)
      : null

    proc.stdout?.on('data', d => { stdout += d.toString('utf-8') })
    proc.stderr?.on('data', d => { stderr += d.toString('utf-8') })

    proc.on('error', err => {
      if (timeout) clearTimeout(timeout)
      const e = err as GitRunError
      e.stdout = stdout
      e.stderr = stderr
      e.code = null
      e.signal = null
      reject(e)
    })

    proc.on('close', (code, signal) => {
      if (timeout) clearTimeout(timeout)
      if (timedOut) {
        const err = new Error(`git ${args.join(' ')} timed out after ${opts.timeoutMs}ms`) as GitRunError
        err.stdout = stdout
        err.stderr = stderr
        err.code = code
        err.signal = signal
        return reject(err)
      }
      if (code === 0) return resolve({ stdout, stderr, code: 0 })
      const err = new Error(`git exited with code ${code}: ${stderr.slice(0, 500)}`) as GitRunError
      err.stdout = stdout
      err.stderr = stderr
      err.code = code
      err.signal = signal
      reject(err)
    })
  })
}

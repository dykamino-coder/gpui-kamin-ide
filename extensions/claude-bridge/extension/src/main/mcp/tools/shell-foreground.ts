// Owns bounded foreground shell output and process deadline settlement.
import { spawn, type ChildProcess } from 'node:child_process'
import { StringDecoder } from 'node:string_decoder'
import type { McpResult } from './shell'
import { windowsJobRunner } from './windows-job-runner'
import { StreamCapture } from './shell-output'

const TERMINATION_GRACE_MS = 1000
const textResult = (text: string): McpResult => ({
  content: [{ type: 'text', text }],
})
const errorResult = (message: string): McpResult => ({
  ...textResult(`Error: ${message}`),
  isError: true,
})

/**
 * The native shell tools drop leading blank lines and trailing whitespace of
 * the output and keep everything between, line endings included.
 */
export function normalizeOutput(text: string): string {
  return text.replace(/^(?:[ \t]*\r?\n)+/, '').trimEnd()
}

export function runForegroundCommand(
  command: string,
  shell: string,
  timeout: number,
  shellFlags: string[] = ['-c'],
  env: NodeJS.ProcessEnv = process.env,
  signal?: AbortSignal,
  onOutput?: (chunk: string) => void,
  cwd?: string,
  /** A shell started ahead of time (warm-shell.ts); it already carries the
   *  same runner, flags, environment and folder as the spawn below. */
  prestarted?: () => ChildProcess | null,
  /** The tool name in the empty-output notice, as the native tools word it. */
  label = 'Command',
): Promise<McpResult> {
  if (signal?.aborted)
    return Promise.resolve({
      ...errorResult('Command cancelled'),
      isError: true,
    })
  return new Promise((resolve) => {
    const stdoutCapture = new StreamCapture('stdout')
    const stderrCapture = new StreamCapture('stderr')
    let timedOut = false
    let cancelled = false
    let settled = false
    let forceTimer: ReturnType<typeof setTimeout> | undefined

    const finish = (code: number | null, failure?: string): void => {
      if (settled) return
      settled = true
      clearTimeout(timer)
      if (!timedOut && !cancelled) clearTimeout(forceTimer)
      signal?.removeEventListener('abort', abort)
      child.stdout?.destroy()
      child.stderr?.destroy()
      // finish() also closes a saved-output file, so it runs on every path.
      const stdout = stdoutCapture.finish()
      const stderr = stderrCapture.finish()
      if (failure) {
        resolve(errorResult(failure))
        return
      }
      const parts: string[] = []
      const output = normalizeOutput(stdout + stderr)
      if (code !== null && code !== 0) parts.push(`Exit code ${code}`)
      if (output) parts.push(output)
      if (cancelled) parts.push('[Command cancelled; termination requested]')
      if (timedOut)
        parts.push(
          `[Command timed out after ${timeout}ms; termination requested. Check for surviving processes before retrying.]`,
        )
      const result = textResult(parts.join('\n') || `(${label} completed with no output)`)
      result.exitCode = code
      if (timedOut || cancelled || code !== 0) result.isError = true
      resolve(result)
    }

    // On Windows the command runs inside its own Job Object when the runner is
    // installed: ending the runner then ends every process of the command.
    const runner = windowsJobRunner()
    const argv = [...shellFlags, command]
    const child = prestarted?.() ?? spawn(runner ?? shell, runner ? [shell, ...argv] : argv, {
      stdio: ['ignore', 'pipe', 'pipe'],
      windowsHide: true,
      detached: process.platform !== 'win32',
      // Match CLI: GIT_EDITOR=true blocks interactive git editor; CLAUDECODE=1
      // identifies our session so plugins/scripts can adapt. See CLI's
      // `utils/Shell.ts` — same two flags.
      env,
      cwd,
    })

    const signalTree = (signal: NodeJS.Signals): void => {
      if (process.platform !== 'win32' && child.pid) {
        try {
          process.kill(-child.pid, signal)
          return
        } catch {
          /* group already exited */
        }
      }
      try {
        child.kill(signal)
      } catch {
        /* close/exit race */
      }
    }
    const terminate = (): void => {
      if (forceTimer) return
      // Do not depend on close: descendants can keep inherited pipes open
      // after the shell exits, and .killed only means a signal was sent.
      forceTimer = setTimeout(() => {
        signalTree('SIGKILL')
        finish(child.exitCode)
      }, TERMINATION_GRACE_MS)
      if (runner) {
        // The runner holds the only handle of the job. Its end closes the job,
        // Windows ends every process in it, the pipes close and `close`
        // settles the call; the timer above is only the ceiling.
        try {
          child.kill('SIGKILL')
        } catch {
          /* close/exit race */
        }
        return
      }
      if (process.platform === 'win32' && child.pid) {
        try {
          const killer = spawn('taskkill', ['/pid', String(child.pid), '/T', '/F'], {
            windowsHide: true,
            stdio: 'ignore',
          })
          killer.on('error', () => {
            signalTree('SIGTERM')
          })
          killer.on('exit', (code) => {
            if (code !== 0) signalTree('SIGTERM')
          })
          // Let taskkill enumerate the live parent before terminating it.
          // Killing the shell first can orphan its descendants on Windows.
          return
        } catch {
          /* bounded settlement still runs */
        }
      }
      signalTree('SIGTERM')
    }
    const abort = (): void => {
      cancelled = true
      terminate()
    }
    const timer = setTimeout(() => {
      timedOut = true
      terminate()
    }, timeout)
    signal?.addEventListener('abort', abort, { once: true })
    if (signal?.aborted) abort()

    // The live stream is decoded per stream across chunk boundaries; the
    // final result is decoded from the concatenated bytes below.
    const liveStdout = new StringDecoder('utf8')
    const liveStderr = new StringDecoder('utf8')
    child.stdout?.on('data', (chunk: Buffer) => {
      if (onOutput) {
        const text = liveStdout.write(chunk)
        if (text) onOutput(text)
      }
      stdoutCapture.push(chunk)
    })

    child.stderr?.on('data', (chunk: Buffer) => {
      if (onOutput) {
        const text = liveStderr.write(chunk)
        if (text) onOutput(text)
      }
      // Each stream keeps its own bounded memory (head + tail) and its own file.
      stderrCapture.push(chunk)
    })

    child.on('error', (err) => finish(null, `Failed to execute command: ${err.message}`))
    child.on('close', (code) => finish(code))
  })
}

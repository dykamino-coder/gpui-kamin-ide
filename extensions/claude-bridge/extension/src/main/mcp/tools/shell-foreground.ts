// Owns bounded foreground shell output and process deadline settlement.
import { spawn } from 'node:child_process'
import type { McpResult } from './shell'

const MAX_OUTPUT_BYTES = 100 * 1024
const TERMINATION_GRACE_MS = 1000
const textResult = (text: string): McpResult => ({ content: [{ type: 'text', text }] })
const errorResult = (message: string): McpResult => textResult(`Error: ${message}`)

export function runForegroundCommand(command: string, shell: string, timeout: number, shellFlags: string[] = ['-c'], env: NodeJS.ProcessEnv = process.env, signal?: AbortSignal): Promise<McpResult> {
  if (signal?.aborted) return Promise.resolve({ ...errorResult('Command cancelled'), isError: true })
  return new Promise((resolve) => {
    const stdoutChunks: Buffer[] = []
    const stderrChunks: Buffer[] = []
    let totalBytes = 0
    let stderrBytes = 0
    let truncated = false
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
      if (failure) { resolve(errorResult(failure)); return }
      const parts: string[] = []
      const stdout = Buffer.concat(stdoutChunks).toString('utf8')
      const stderr = Buffer.concat(stderrChunks).toString('utf8')
      if (stdout) parts.push(stdout)
      if (stderr) parts.push(`\n--- stderr ---\n${stderr}`)
      if (truncated || stderrBytes > MAX_OUTPUT_BYTES) parts.push(`\n[Output truncated: exceeded ${MAX_OUTPUT_BYTES / 1024}KB per stream]`)
      if (cancelled) parts.push('\n[Command cancelled; termination requested]')
      if (timedOut) parts.push(`\n[Command timed out after ${timeout}ms; termination requested. Check for surviving processes before retrying.]`)
      if (code !== null && code !== 0) parts.push(`\n[Exit code: ${code}]`)
      const result = textResult(parts.join('') || `Command completed with exit code ${code ?? 0}`)
      if (timedOut || cancelled) result.isError = true
      resolve(result)
    }

    const child = spawn(shell, [...shellFlags, command], {
      stdio: ['ignore', 'pipe', 'pipe'],
      windowsHide: true,
      detached: process.platform !== 'win32',
      // Match CLI: GIT_EDITOR=true blocks interactive git editor; CLAUDECODE=1
      // identifies our session so plugins/scripts can adapt. See CLI's
      // `utils/Shell.ts` — same two flags.
      env,
    })

    const signalTree = (signal: NodeJS.Signals): void => {
      if (process.platform !== 'win32' && child.pid) {
        try { process.kill(-child.pid, signal); return } catch { /* group already exited */ }
      }
      try { child.kill(signal) } catch { /* close/exit race */ }
    }
    const terminate = (): void => {
      if (forceTimer) return
      // Do not depend on close: descendants can keep inherited pipes open
      // after the shell exits, and .killed only means a signal was sent.
      forceTimer = setTimeout(() => {
        signalTree('SIGKILL')
        finish(child.exitCode)
      }, TERMINATION_GRACE_MS)
      if (process.platform === 'win32' && child.pid) {
        try {
          const killer = spawn('taskkill', ['/pid', String(child.pid), '/T', '/F'], { windowsHide: true, stdio: 'ignore' })
          killer.on('error', () => { /* bounded settlement still runs if taskkill is unavailable */ })
        } catch { /* bounded settlement still runs */ }
      }
      signalTree('SIGTERM')
    }
    const abort = (): void => { cancelled = true; terminate() }
    const timer = setTimeout(() => { timedOut = true; terminate() }, timeout)
    signal?.addEventListener('abort', abort, { once: true })
    if (signal?.aborted) abort()

    child.stdout?.on('data', (chunk: Buffer) => {
      if (truncated) return
      totalBytes += chunk.length
      if (totalBytes > MAX_OUTPUT_BYTES) {
        truncated = true
        // Take only what fits
        const remaining = MAX_OUTPUT_BYTES - (totalBytes - chunk.length)
        if (remaining > 0) {
          stdoutChunks.push(chunk.subarray(0, remaining))
        }
      } else {
        stdoutChunks.push(chunk)
      }
    })

    child.stderr?.on('data', (chunk: Buffer) => {
      // Cap stderr on its OWN byte counter — the old check gated on the stdout
      // counter but never incremented for stderr, so a command with tiny stdout
      // and huge stderr (a noisy compiler, `yes 1>&2`) buffered without bound.
      if (stderrBytes >= MAX_OUTPUT_BYTES) return
      const remaining = MAX_OUTPUT_BYTES - stderrBytes
      stderrChunks.push(chunk.length > remaining ? chunk.subarray(0, remaining) : chunk)
      stderrBytes += chunk.length
    })

    child.on('error', err => finish(null, `Failed to execute command: ${err.message}`))
    child.on('close', code => finish(code))
  })
}

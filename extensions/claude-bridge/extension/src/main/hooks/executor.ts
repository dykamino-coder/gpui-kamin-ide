// Electron-host hook executor. Receives `hook:execute` over WS from bridge,
// runs the user's command on the Windows host with full access to the user's
// PATH, env, IDE binaries, npm-installed tools, etc. Returns stdout/stderr/
// exitCode back to bridge — which forwards as a CLI-compatible hook response.

import { spawn } from 'node:child_process'
import os from 'node:os'

export interface HookExecuteRequest {
  type: 'hook:execute'
  requestId: string
  sessionId: string
  hookId: string
  event: string
  command: string
  shell?: 'bash' | 'powershell' | 'sh' | 'zsh'
  cwd?: string
  env?: Record<string, string>
  payload: Record<string, unknown>
  timeoutMs?: number
}

export interface HookExecuteResponse {
  type: 'hook:response'
  requestId: string
  result: {
    stdout: string
    stderr: string
    exitCode: number
    outcome: 'success' | 'error' | 'timeout' | 'cancelled'
    jsonOutput?: Record<string, unknown>
    durationMs: number
  }
}

// Match Claude Code's DEFAULT_HTTP_HOOK_TIMEOUT_MS = 10 minutes.
const DEFAULT_TIMEOUT_MS = 10 * 60 * 1000

/** Pick the best interpreter for the requested shell. On Windows we fall
 *  back to `bash.exe` from Git for Windows when shell='bash' (Claude Code
 *  itself does the same). */
function resolveShell(shell?: string): { bin: string; flag: string } {
  const isWin = process.platform === 'win32'
  switch (shell) {
    case 'powershell':
      return { bin: isWin ? 'pwsh.exe' : 'pwsh', flag: '-Command' }
    case 'sh':
      return { bin: 'sh', flag: '-c' }
    case 'zsh':
      return { bin: 'zsh', flag: '-c' }
    case 'bash':
    default:
      return { bin: isWin ? 'bash.exe' : 'bash', flag: '-c' }
  }
}

export function executeHook(req: HookExecuteRequest): Promise<HookExecuteResponse['result']> {
  const start = Date.now()
  const { bin, flag } = resolveShell(req.shell)
  const timeoutMs = req.timeoutMs ?? DEFAULT_TIMEOUT_MS
  const cwd = req.cwd && req.cwd.length > 0 ? req.cwd : os.homedir()

  return new Promise((resolve) => {
    let proc
    try {
      proc = spawn(bin, [flag, req.command], {
        cwd,
        env: { ...process.env, ...(req.env ?? {}), CLAUDE_BRIDGE_HOOK: '1' },
        windowsHide: true,
        stdio: ['pipe', 'pipe', 'pipe'],
      })
    } catch (err) {
      resolve({
        stdout: '', stderr: `spawn failed: ${err instanceof Error ? err.message : String(err)}`,
        exitCode: 127, outcome: 'error', durationMs: Date.now() - start,
      })
      return
    }

    let stdout = ''
    let stderr = ''
    proc.stdout?.on('data', d => { stdout += d.toString() })
    proc.stderr?.on('data', d => { stderr += d.toString() })

    const killer = setTimeout(() => {
      try { proc.kill('SIGKILL') } catch { /* ignore */ }
      resolve({
        stdout, stderr: stderr + `\n[hook killed after ${timeoutMs}ms]`,
        exitCode: 124, outcome: 'timeout', durationMs: Date.now() - start,
      })
    }, timeoutMs)

    proc.on('exit', (code) => {
      clearTimeout(killer)
      let jsonOutput: Record<string, unknown> | undefined
      try {
        const trimmed = stdout.trim()
        if (trimmed.startsWith('{')) jsonOutput = JSON.parse(trimmed)
      } catch { /* not JSON, keep raw stdout */ }
      resolve({
        stdout, stderr,
        exitCode: code ?? 0,
        outcome: code === 0 ? 'success' : 'error',
        jsonOutput,
        durationMs: Date.now() - start,
      })
    })

    proc.on('error', (err) => {
      clearTimeout(killer)
      resolve({
        stdout, stderr: stderr + `\nspawn error: ${err.message}`,
        exitCode: 127, outcome: 'error', durationMs: Date.now() - start,
      })
    })

    try {
      proc.stdin?.write(JSON.stringify(req.payload))
      proc.stdin?.end()
    } catch { /* ignore EPIPE */ }
  })
}

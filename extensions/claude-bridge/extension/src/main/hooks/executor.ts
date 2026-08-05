// VSIX bridge-host hook executor. Receives `hook:execute` over WS from bridge,
// runs the user's command on the client host with full access to the user's
// PATH, env, IDE binaries, npm-installed tools, etc. Returns stdout/stderr/
// exitCode back to bridge — which forwards as a CLI-compatible hook response.

import { spawn } from 'node:child_process'
import os from 'node:os'
import fs from 'node:fs'
import path from 'node:path'
import { loadInstalledPluginsMap, loadPluginOptions, substituteUserConfig } from '../plugin-helpers'

export interface HookExecuteRequest {
  type: 'hook:execute'
  requestId: string
  sessionId: string
  hookId: string
  event: string
  command: string
  args?: string[]
  pluginId?: string
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

export async function executeHook(req: HookExecuteRequest): Promise<HookExecuteResponse['result']> {
  const start = Date.now()
  const { bin, flag } = resolveShell(req.shell)
  const timeoutMs = req.timeoutMs ?? DEFAULT_TIMEOUT_MS
  const cwd = req.cwd && req.cwd.length > 0 ? req.cwd : os.homedir()

  let command = req.command
  let args = req.args
  const pluginEnv: Record<string, string> = {}
  if (req.pluginId) {
    const installed = await loadInstalledPluginsMap()
    const pluginRoot = installed.get(req.pluginId)?.installPath
    if (!pluginRoot || !fs.existsSync(pluginRoot)) {
      return {
        stdout: '', stderr: `Plugin ${req.pluginId} is not installed on the client host`,
        exitCode: 127, outcome: 'error', durationMs: Date.now() - start,
      }
    }
    const options = loadPluginOptions(req.pluginId)
    const dataDir = path.join(os.homedir(), '.claude', 'plugins', 'data', req.pluginId.replace(/[^a-zA-Z0-9\-_]/g, '-'))
    try { fs.mkdirSync(dataDir, { recursive: true }) } catch { /* best effort */ }
    if (!args && /\$\{user_config\.[^}]+\}/.test(command)) {
      return {
        stdout: '',
        stderr: 'Shell-form plugin hook commands cannot reference ${user_config.*}; use exec-form args or CLAUDE_PLUGIN_OPTION_<KEY>',
        exitCode: 2,
        outcome: 'error',
        durationMs: Date.now() - start,
      }
    }
    // Exec-form keeps configurable values in argv slots. Shell-form receives
    // them only through CLAUDE_PLUGIN_OPTION_* below.
    if (args) command = substituteUserConfig(command, options)
    command = command
      .replace(/\$\{CLAUDE_PLUGIN_ROOT\}/g, pluginRoot)
      .replace(/\$\{CLAUDE_PLUGIN_DATA\}/g, dataDir)
      .replace(/\$\{CLAUDE_PROJECT_DIR\}/g, cwd)
    args = args?.map(arg => substituteUserConfig(arg, options)
      .replace(/\$\{CLAUDE_PLUGIN_ROOT\}/g, pluginRoot)
      .replace(/\$\{CLAUDE_PLUGIN_DATA\}/g, dataDir)
      .replace(/\$\{CLAUDE_PROJECT_DIR\}/g, cwd))
    for (const [key, value] of Object.entries(options)) {
      pluginEnv[`CLAUDE_PLUGIN_OPTION_${key.toUpperCase()}`] = String(value)
    }
    pluginEnv.CLAUDE_PLUGIN_ROOT = pluginRoot
    pluginEnv.CLAUDE_PLUGIN_DATA = dataDir
    pluginEnv.CLAUDE_PROJECT_DIR = cwd
    const pluginBin = path.join(pluginRoot, 'bin')
    if (fs.existsSync(pluginBin)) pluginEnv.PATH = `${pluginBin}${path.delimiter}${process.env.PATH ?? ''}`
  }
  return new Promise((resolve) => {
    let proc
    try {
      proc = args
        ? spawn(command, args, {
          cwd,
          env: { ...process.env, ...(req.env ?? {}), ...pluginEnv, CLAUDE_BRIDGE_HOOK: '1' },
          windowsHide: true,
          stdio: ['pipe', 'pipe', 'pipe'],
        })
        : spawn(bin, [flag, command], {
        cwd,
        env: { ...process.env, ...(req.env ?? {}), ...pluginEnv, CLAUDE_BRIDGE_HOOK: '1' },
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
    const maxOutput = 1024 * 1024
    proc.stdout?.on('data', d => { if (stdout.length < maxOutput) stdout += d.toString().slice(0, maxOutput - stdout.length) })
    proc.stderr?.on('data', d => { if (stderr.length < maxOutput) stderr += d.toString().slice(0, maxOutput - stderr.length) })

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

    proc.stdin?.on('error', () => { /* ignore EPIPE when the hook exits without reading stdin */ })
    try {
      proc.stdin?.write(JSON.stringify(req.payload))
      proc.stdin?.end()
    } catch { /* ignore EPIPE */ }
  })
}

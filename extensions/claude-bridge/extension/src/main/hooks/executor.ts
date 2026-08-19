// VSIX bridge-host hook executor. Receives `hook:execute` over WS from bridge,
// runs the user's command on the client host with full access to the user's
// PATH, env, IDE binaries, npm-installed tools, etc. Returns stdout/stderr/
// exitCode back to bridge — which forwards as a CLI-compatible hook response.

import { spawn } from 'node:child_process'
import os from 'node:os'
import fs from 'node:fs'
import path from 'node:path'
import { loadInstalledPluginsMap, loadPluginOptions, substituteUserConfig } from '../plugin-helpers'
import {
  powerShellSingleQuote,
  resolveHookShell,
  shellSingleQuote,
  toPosixShellPath,
  type ResolvedShell,
} from '../utils/command-runtime'

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

function portableBasename(value: string): string {
  return value.split(/[\\/]/).filter(Boolean).at(-1) ?? value
}

/** Server payloads contain transcript paths from the Linux container. Local
 * hooks execute on the client, where the same JSONL is mirrored under the
 * Kamin cache directory. Do not expose a path that cannot exist on Windows. */
export function localizeHookPayload(
  payload: Record<string, unknown>,
  cacheDir: string | undefined = process.env.KAMIN_CACHE_DIR,
): Record<string, unknown> {
  const localized = { ...payload }
  if (!cacheDir) return localized
  for (const key of ['transcript_path', 'agent_transcript_path'] as const) {
    const remotePath = localized[key]
    if (typeof remotePath === 'string' && remotePath.length > 0) {
      localized[key] = path.join(cacheDir, 'transcripts', portableBasename(remotePath))
    }
  }
  return localized
}

function nativeNodeCommand(command: string): string {
  return /(^|[\\/])node(?:\.exe)?$/i.test(command) ? process.execPath : command
}

function shellNodeShim(shell: ResolvedShell): string {
  if (shell.kind === 'powershell') {
    const executable = powerShellSingleQuote(process.execPath)
    return `function node { & ${executable} @args }; function node.exe { & ${executable} @args }; `
  }
  const executable = toPosixShellPath(process.execPath)
  return `node() { ${shellSingleQuote(executable)} "$@"; }; node.exe() { node "$@"; }; `
}

function shellPluginEnvValue(value: string, shell: ResolvedShell): string {
  return shell.kind === 'bash' || shell.kind === 'sh' || shell.kind === 'zsh'
    ? toPosixShellPath(value)
    : value
}

function powerShellEnvPlaceholders(command: string): string {
  return command
    .replace(/\$\{CLAUDE_PLUGIN_ROOT\}/g, '${env:CLAUDE_PLUGIN_ROOT}')
    .replace(/\$\{CLAUDE_PLUGIN_DATA\}/g, '${env:CLAUDE_PLUGIN_DATA}')
    .replace(/\$\{CLAUDE_PROJECT_DIR\}/g, '${env:CLAUDE_PROJECT_DIR}')
}

async function killProcessTree(proc: import('node:child_process').ChildProcess): Promise<void> {
  if (!proc.pid) return
  if (process.platform === 'win32') {
    await new Promise<void>((resolve) => {
      const killer = spawn('taskkill', ['/pid', String(proc.pid), '/T', '/F'], {
        windowsHide: true,
        stdio: 'ignore',
      })
      killer.on('error', () => resolve())
      killer.on('close', () => resolve())
    })
    return
  }
  try { process.kill(-proc.pid, 'SIGKILL') } catch {
    try { proc.kill('SIGKILL') } catch { /* best effort */ }
  }
}

export async function executeHook(req: HookExecuteRequest): Promise<HookExecuteResponse['result']> {
  const start = Date.now()
  const timeoutMs = req.timeoutMs ?? DEFAULT_TIMEOUT_MS
  const cwd = req.cwd && req.cwd.length > 0 ? req.cwd : os.homedir()
  const execForm = req.args !== undefined

  let shell: ResolvedShell | undefined
  if (!execForm) {
    try {
      shell = await resolveHookShell(req.shell)
    } catch (err) {
      return {
        stdout: '', stderr: err instanceof Error ? err.message : String(err),
        exitCode: 127, outcome: 'error', durationMs: Date.now() - start,
      }
    }
  }

  let command = req.command
  let args = req.args
  const pluginEnv: Record<string, string> = {}
  if (execForm) {
    command = command.replace(/\$\{CLAUDE_PROJECT_DIR\}/g, cwd)
    args = args?.map(arg => arg.replace(/\$\{CLAUDE_PROJECT_DIR\}/g, cwd))
  } else if (shell?.kind === 'powershell') {
    command = powerShellEnvPlaceholders(command)
  }
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
    if (!execForm && /\$\{user_config\.[^}]+\}/.test(command)) {
      return {
        stdout: '',
        stderr: 'Shell-form plugin hook commands cannot reference ${user_config.*}; use exec-form args or CLAUDE_PLUGIN_OPTION_<KEY>',
        exitCode: 2,
        outcome: 'error',
        durationMs: Date.now() - start,
      }
    }
    // Exec-form keeps configurable values in argv slots and receives native
    // host paths. Shell-form keeps Claude's documented env placeholders so
    // quoting remains the plugin author's responsibility instead of ours.
    if (execForm) {
      command = substituteUserConfig(command, options)
        .replace(/\$\{CLAUDE_PLUGIN_ROOT\}/g, pluginRoot)
        .replace(/\$\{CLAUDE_PLUGIN_DATA\}/g, dataDir)
        .replace(/\$\{CLAUDE_PROJECT_DIR\}/g, cwd)
      args = args?.map(arg => substituteUserConfig(arg, options)
        .replace(/\$\{CLAUDE_PLUGIN_ROOT\}/g, pluginRoot)
        .replace(/\$\{CLAUDE_PLUGIN_DATA\}/g, dataDir)
        .replace(/\$\{CLAUDE_PROJECT_DIR\}/g, cwd))
    }
    for (const [key, value] of Object.entries(options)) {
      pluginEnv[`CLAUDE_PLUGIN_OPTION_${key.toUpperCase()}`] = String(value)
    }
    pluginEnv.CLAUDE_PLUGIN_ROOT = shell ? shellPluginEnvValue(pluginRoot, shell) : pluginRoot
    pluginEnv.CLAUDE_PLUGIN_DATA = shell ? shellPluginEnvValue(dataDir, shell) : dataDir
    const pluginBin = path.join(pluginRoot, 'bin')
    if (fs.existsSync(pluginBin)) pluginEnv.PATH = `${pluginBin}${path.delimiter}${process.env.PATH ?? ''}`
  }
  pluginEnv.CLAUDE_PROJECT_DIR = shell ? shellPluginEnvValue(cwd, shell) : cwd
  if (execForm) command = nativeNodeCommand(command)
  else command = shellNodeShim(shell!) + command

  const payload = localizeHookPayload(req.payload, req.env?.KAMIN_CACHE_DIR ?? process.env.KAMIN_CACHE_DIR)
  return new Promise((resolve) => {
    let proc: import('node:child_process').ChildProcess
    let settled = false
    const finish = (result: HookExecuteResponse['result']): void => {
      if (settled) return
      settled = true
      resolve(result)
    }
    try {
      proc = execForm
        ? spawn(command, args ?? [], {
          cwd,
          env: { ...process.env, ...(req.env ?? {}), ...pluginEnv, CLAUDE_BRIDGE_HOOK: '1' },
          windowsHide: true,
          stdio: ['pipe', 'pipe', 'pipe'],
          detached: process.platform !== 'win32',
        })
        : spawn(shell!.bin, [...shell!.args, command], {
          cwd,
          env: { ...process.env, ...(req.env ?? {}), ...pluginEnv, CLAUDE_BRIDGE_HOOK: '1' },
          windowsHide: true,
          stdio: ['pipe', 'pipe', 'pipe'],
          detached: process.platform !== 'win32',
        })
    } catch (err) {
      finish({
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
      void killProcessTree(proc)
      finish({
        stdout, stderr: stderr + `\n[hook killed after ${timeoutMs}ms]`,
        exitCode: 124, outcome: 'timeout', durationMs: Date.now() - start,
      })
    }, timeoutMs)

    proc.on('exit', (code) => {
      clearTimeout(killer)
      let jsonOutput: Record<string, unknown> | undefined
      if (code === 0) {
        try {
          const trimmed = stdout.trim()
          if (trimmed.startsWith('{')) jsonOutput = JSON.parse(trimmed)
        } catch { /* not JSON, keep raw stdout */ }
      }
      finish({
        stdout, stderr,
        exitCode: code ?? 0,
        outcome: code === 0 ? 'success' : 'error',
        jsonOutput,
        durationMs: Date.now() - start,
      })
    })

    proc.on('error', (err) => {
      clearTimeout(killer)
      finish({
        stdout, stderr: stderr + `\nspawn error: ${err.message}`,
        exitCode: 127, outcome: 'error', durationMs: Date.now() - start,
      })
    })

    proc.stdin?.on('error', () => { /* ignore EPIPE when the hook exits without reading stdin */ })
    try {
      proc.stdin?.write(JSON.stringify(payload))
      proc.stdin?.end()
    } catch { /* ignore EPIPE */ }
  })
}

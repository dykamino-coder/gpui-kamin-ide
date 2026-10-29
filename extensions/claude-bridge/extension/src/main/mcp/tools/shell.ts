import { spawn, type ChildProcess } from 'node:child_process'
import { runForegroundCommand } from './shell-foreground'
import path from 'node:path'
import type { ToolCallContext } from '../executor'
import { getOrCreateSnapshot, wrapCommandWithSnapshot, shouldUseLoginShell } from './shell-snapshot'
import { listEnabledPluginBinDirs } from '../../plugin-helpers'
import { findBashExecutable, findPowerShellExecutable } from '../../utils/command-runtime'

async function shellEnv(): Promise<NodeJS.ProcessEnv> {
  const binDirs = await listEnabledPluginBinDirs()
  return {
    ...process.env,
    PATH: [...binDirs, process.env.PATH ?? ''].filter(Boolean).join(path.delimiter),
    GIT_EDITOR: 'true',
    CLAUDECODE: '1',
  }
}

export type McpResult = {
  isError?: boolean
  content: Array<{ type: string; text: string }>
}

function textResult(text: string): McpResult {
  return { content: [{ type: 'text', text }] }
}

function errorResult(message: string): McpResult {
  return { content: [{ type: 'text', text: `Error: ${message}` }] }
}

/**
 * Shim functions prepended to every bash command on Windows. Certain CLI
 * tools pop a Windows MessageBox when invoked without a real console window
 * (our spawn of `bash.exe -c …` from the VSIX bridge host has no
 * attached conhost). The modal blocks the entire agent turn until the user
 * clicks OK — useless and scary.
 *
 * Defining a shell function with the same name as the binary takes priority
 * over PATH lookup, so `nvm -v 2>&1 || echo "nvm not found"` resolves to our
 * function (return 127, print to stderr) and the `|| echo` fallback still
 * fires. Add new shims here when other tools show similar modal popups.
 */
const WINDOWS_MODAL_SHIM =
  process.platform === 'win32'
    ? `\
nvm() { echo "[bridge] nvm-windows blocked: shows a modal GUI prompt in non-interactive shells — use fnm, volta, or a direct Node installer" >&2; return 127; }
`
    : ''

/** Track background processes so BashOutput can poll them and KillShell can
 *  terminate them. `stdoutCursor`/`stderrCursor` mark how much each stream the
 *  model has already read, so BashOutput returns only NEW output (native-CLI
 *  parity). `command` is kept for the status header. */
interface BackgroundOutput {
  stdout: string
  stderr: string
  exitCode: number | null
  running: boolean
  stdoutCursor: number
  stderrCursor: number
  command: string
  owner?: string
  tabId?: string | null
  startedAt: number
  tool: string
}
const backgroundProcesses = new Map<number, ChildProcess>()
const backgroundOutput = new Map<number, BackgroundOutput>()
let nextBgId = 1

// Retain the output of at most this many background shells. `backgroundOutput`
// is deliberately kept after a process exits (so BashOutput can read the final
// output), but nothing evicted it — over a long session with many
// run_in_background calls it grew without bound. Evict the oldest FINISHED
// entries once over the cap (never evict a still-running shell).
const MAX_BG_RETAINED = 100
function evictOldBackgroundOutput(): void {
  if (backgroundOutput.size <= MAX_BG_RETAINED) return
  for (const [id, out] of backgroundOutput) {
    if (backgroundOutput.size <= MAX_BG_RETAINED) break
    if (!out.running) backgroundOutput.delete(id)
  }
}

/**
 * Find bash executable — matches CLI's Windows policy:
 * 1. Check CLAUDE_CODE_GIT_BASH_PATH env var
 * 2. Find git in PATH → derive bash.exe location
 * 3. Check standard system/user Git for Windows locations
 * 4. Error if not found (never launch System32 bash.exe / WSL by accident)
 *
 * CLI ALWAYS uses bash on Windows. No PowerShell, no cmd.
 */
let cachedBash: string | null = null
let bashLookupInFlight: Promise<string> | null = null

async function findBashAsync(): Promise<string> {
  if (cachedBash) return cachedBash
  if (bashLookupInFlight) return bashLookupInFlight

  bashLookupInFlight = (async () => {
    cachedBash = await findBashExecutable()
    return cachedBash
  })()
  return bashLookupInFlight
}

/**
 * PowerShell tool — Windows preview. Uses pwsh or powershell.exe.
 */
function shellTimeout(input: Record<string, unknown>, context?: ToolCallContext): number {
  if (typeof context?.taskDeadlineMs === 'number' && Number.isFinite(context.taskDeadlineMs)) {
    return Math.max(1, Math.min(context.taskDeadlineMs - Date.now(), 86_400_000))
  }
  return typeof input.timeout === 'number' && Number.isFinite(input.timeout)
    ? Math.max(1, Math.min(input.timeout, 1_800_000))
    : 1_500_000
}

export async function powerShellExecute(input: Record<string, unknown>, context?: ToolCallContext): Promise<McpResult> {
  const command = input.command as string
  if (!command) return errorResult('command is required')

  // Default raised to 25 min (was 9 min) because users hit it with legit
  // long commands like `npm install` / `podman build` / test suites that
  // legitimately run for many minutes. Cap matches HEAVY_TOOL_TIMEOUT_MS
  // and BASH_MAX_TIMEOUT_MS so we don't get cut off by either layer.
  const timeout = shellTimeout(input, context)

  let pwshPath: string
  try {
    pwshPath = await resolvePowerShell()
  } catch (err) {
    return errorResult(err instanceof Error ? err.message : String(err))
  }
  const flags = ['-NoLogo', '-NonInteractive', '-Command']
  const env = await shellEnv()
  if (input.run_in_background === true && context?.taskDeadlineMs === undefined) {
    return runBackgroundCommand(command, pwshPath, String(input.description ?? ''), flags, env, context, 'PowerShell')
  }
  return runForegroundCommand(command, pwshPath, timeout, flags, env, context?.signal, context?.onOutput, context?.cwd)
}

export async function bashExecute(input: Record<string, unknown>, context?: ToolCallContext): Promise<McpResult> {
  const command = input.command as string
  if (!command) return errorResult('command is required')

  // Default raised to 25 min (was 9 min) because users hit it with legit
  // long commands like `npm install` / `podman build` / test suites that
  // legitimately run for many minutes. Cap matches HEAVY_TOOL_TIMEOUT_MS
  // and BASH_MAX_TIMEOUT_MS so we don't get cut off by either layer.
  const timeout = shellTimeout(input, context)
  const runInBackground = input.run_in_background === true && context?.taskDeadlineMs === undefined
  const description = (input.description as string) || ''

  let shell: string
  try {
    shell = await findBashAsync()
  } catch (err) {
    return errorResult(err instanceof Error ? err.message : String(err))
  }

  // Shell snapshot — captured lazily on first call. Gives subsequent bash
  // invocations access to user aliases, functions, PATH tweaks from
  // `.profile` / `.bashrc`. If capture failed (unusual managed shells,
  // no `-i` support), fall back to a login shell flag so profile init
  // still runs, at the cost of re-sourcing per call.
  const snapshot = await getOrCreateSnapshot(shell)
  const wrapped = wrapCommandWithSnapshot(WINDOWS_MODAL_SHIM + command, snapshot)
  const shellFlags = shouldUseLoginShell(snapshot) ? ['-lc'] : ['-c']
  const env = await shellEnv()

  if (runInBackground) {
    return runBackgroundCommand(wrapped, shell, description, shellFlags, env, context, 'Bash', command)
  }

  return runForegroundCommand(
    wrapped,
    shell,
    timeout,
    shellFlags,
    env,
    context?.signal,
    context?.onOutput,
    context?.cwd,
  )
}

const BG_OUTPUT_CAP = 100 * 1024

function appendBounded(chunks: Buffer[], lengthRef: { size: number }, chunk: Buffer): void {
  lengthRef.size += chunk.length
  chunks.push(chunk)
  while (lengthRef.size > BG_OUTPUT_CAP && chunks.length > 0) {
    const head = chunks[0]!
    if (lengthRef.size - head.length >= BG_OUTPUT_CAP) {
      lengthRef.size -= head.length
      chunks.shift()
    } else {
      const keep = BG_OUTPUT_CAP - (lengthRef.size - head.length)
      chunks[0] = head.subarray(head.length - keep)
      lengthRef.size = BG_OUTPUT_CAP
    }
  }
}

function runBackgroundCommand(
  command: string,
  shell: string,
  description: string,
  shellFlags: string[] = ['-c'],
  env: NodeJS.ProcessEnv = process.env,
  context?: ToolCallContext,
  tool = 'Bash',
  displayCommand = command,
): McpResult {
  evictOldBackgroundOutput()
  const bgId = nextBgId++

  const child = spawn(shell, [...shellFlags, command], {
    stdio: ['ignore', 'pipe', 'pipe'],
    detached: true,
    // Suppress any child-console window flashing on Windows. Modal dialogs
    // from misbehaving tools like nvm-windows are handled separately via
    // WINDOWS_MODAL_SHIM — `windowsHide` only covers console windows.
    windowsHide: true,
    // Match CLI: GIT_EDITOR=true silences interactive git editor prompts
    // (commit without -m, merge w/o --no-edit). CLAUDECODE=1 is a marker
    // some plugin-provided scripts check for to adjust behaviour.
    env,
    cwd: context?.cwd,
  })

  if (child.pid) {
    backgroundProcesses.set(bgId, child)
    const stdoutChunks: Buffer[] = []
    const stderrChunks: Buffer[] = []
    const stdoutLen = { size: 0 }
    const stderrLen = { size: 0 }
    const output: BackgroundOutput = {
      stdout: '',
      stderr: '',
      exitCode: null,
      running: true,
      stdoutCursor: 0,
      stderrCursor: 0,
      command: displayCommand,
      owner: context?.shellOwner,
      tabId: context?.tabId,
      startedAt: Date.now(),
      tool,
    }
    backgroundOutput.set(bgId, output)

    child.stdout?.on('data', (chunk: Buffer) => {
      appendBounded(stdoutChunks, stdoutLen, chunk)
      output.stdout = Buffer.concat(stdoutChunks).toString('utf8')
    })

    child.stderr?.on('data', (chunk: Buffer) => {
      appendBounded(stderrChunks, stderrLen, chunk)
      output.stderr = Buffer.concat(stderrChunks).toString('utf8')
    })

    child.on('exit', (code) => {
      output.exitCode = code
      output.running = false
      backgroundProcesses.delete(bgId)
    })

    child.unref()
  }

  return textResult(
    `Background process started (bash_id: ${bgId}, pid: ${child.pid || 'unknown'}).\n` +
      (description ? `Description: ${description}\n` : '') +
      `The command is running in the background. Read its output with BashOutput ` +
      `(bash_id: ${bgId}) and stop it with KillShell.`,
  )
}

/** BashOutput — return output produced since the last read (per-stream cursor,
 *  native-CLI parity) plus the shell's live status. `filter` (optional regex)
 *  keeps only stdout lines that match. */
export async function readBackgroundOutput(input: Record<string, unknown>): Promise<McpResult> {
  const raw = input.bash_id ?? input.shell_id ?? input.id
  const bgId = typeof raw === 'number' ? raw : parseInt(String(raw ?? ''), 10)
  if (!Number.isFinite(bgId)) return errorResult('bash_id is required (the numeric id from run_in_background)')
  const out = backgroundOutput.get(bgId)
  if (!out) return errorResult(`No background shell with bash_id ${bgId} (it may have been evicted).`)

  let newStdout = out.stdout.slice(out.stdoutCursor)
  const newStderr = out.stderr.slice(out.stderrCursor)
  out.stdoutCursor = out.stdout.length
  out.stderrCursor = out.stderr.length

  const filter = typeof input.filter === 'string' ? input.filter : null
  if (filter && newStdout) {
    try {
      const re = new RegExp(filter)
      newStdout = newStdout
        .split('\n')
        .filter((l) => re.test(l))
        .join('\n')
    } catch {
      /* invalid regex — return unfiltered */
    }
  }

  const status = out.running ? 'running' : `exited (code ${out.exitCode ?? 'unknown'})`
  const parts = [`Status: ${status}`]
  if (newStdout) parts.push(`\n<stdout>\n${newStdout}\n</stdout>`)
  if (newStderr) parts.push(`\n<stderr>\n${newStderr}\n</stderr>`)
  if (!newStdout && !newStderr) parts.push(out.running ? '\n(no new output yet)' : '\n(no more output)')
  return textResult(parts.join(''))
}

/** KillShell — terminate a background shell by bash_id. */
export async function killBackgroundShell(input: Record<string, unknown>): Promise<McpResult> {
  const raw = input.bash_id ?? input.shell_id ?? input.id
  const bgId = typeof raw === 'number' ? raw : parseInt(String(raw ?? ''), 10)
  if (!Number.isFinite(bgId)) return errorResult('bash_id is required (the numeric id from run_in_background)')
  const child = backgroundProcesses.get(bgId)
  const out = backgroundOutput.get(bgId)
  if (!child && !out) return errorResult(`No background shell with bash_id ${bgId}.`)
  if (!child || child.killed || (out && !out.running)) {
    return textResult(`Background shell ${bgId} already exited (code ${out?.exitCode ?? 'unknown'}).`)
  }
  try {
    // detached:true spawns the child in its own process group (negative pid) so
    // killing the group takes down grandchildren too (dev servers spawn workers).
    if (typeof child.pid === 'number' && process.platform !== 'win32') {
      try {
        process.kill(-child.pid, 'SIGTERM')
      } catch {
        child.kill('SIGTERM')
      }
    } else {
      child.kill()
    }
  } catch (e) {
    return errorResult(`Failed to kill shell ${bgId}: ${e instanceof Error ? e.message : String(e)}`)
  }
  return textResult(`Killed background shell ${bgId}.`)
}

let cachedPwsh: string | null = null
let pwshLookupInFlight: Promise<string> | null = null

async function resolvePowerShell(): Promise<string> {
  if (cachedPwsh) return cachedPwsh
  if (pwshLookupInFlight) return pwshLookupInFlight
  pwshLookupInFlight = (async () => {
    cachedPwsh = await findPowerShellExecutable()
    return cachedPwsh
  })()
  return pwshLookupInFlight
}

/** Read-only UI snapshot: does not advance the model's BashOutput cursors. */
export function backgroundShellSnapshot(
  tabId: string,
  owner?: string,
): import('../../../shared/commands').ShellSnapshot[] {
  return [...backgroundOutput]
    .filter(([, out]) => out.tabId === tabId && out.owner === owner)
    .slice(-100)
    .map(([id, out]) => ({
      id: `shell:${id}`,
      tool: out.tool,
      command: out.command.slice(0, 512),
      startedAt: out.startedAt,
      running: out.running,
      output: (out.stdout + (out.stderr ? '\n--- stderr ---\n' + out.stderr : '')).slice(-16384),
    }))
}
export async function stopBackgroundShellForOwner(tabId: string, owner: string | undefined, id: string): Promise<void> {
  if (!/^shell:[0-9]+$/.test(id)) return
  const out = backgroundOutput.get(Number(id.slice(6)))
  if (out?.tabId !== tabId || out.owner !== owner) return
  await killBackgroundShell({ bash_id: Number(id.slice(6)) })
}

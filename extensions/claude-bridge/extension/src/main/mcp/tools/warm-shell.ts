// Pre-started shell processes for foreground Bash/PowerShell calls.
//
// Native Claude Code starts a new shell for every command: ~90 ms for Git Bash
// and ~400 ms for pwsh on Windows before the command's first instruction. Here
// the next shell is started while the model is still thinking, and waits for
// the path of its script on stdin. Each command still gets a fresh process with
// the same flags, environment, working directory and job object as a cold
// start; only the start-up time moves off the critical path.
import { spawn, type ChildProcess } from 'node:child_process'
import os from 'node:os'
import { windowsJobRunner } from './windows-job-runner'

/** A started shell has not proven anything until it is used, so it is kept
 *  only briefly: after this idle time it is ended and the next call starts cold. */
const IDLE_MS = 10 * 60_000

/** `bash-hook` keeps stdin open after the two start lines: a hook reads its
 *  JSON input there. */
export type WarmKind = 'bash' | 'powershell' | 'bash-hook'

/** Hooks of different plugins differ in environment; a few may wait at once. */
const MAX_WAITING_HOOKS = 4

/** The command a waiting shell runs: read the working folder and the script
 *  path, enter the folder, give the script an empty stdin as with a cold
 *  start, then run the script in place. The shell waits in the temp folder so
 *  it never keeps a project folder busy (Windows refuses to delete a folder
 *  that is a process's current directory). */
export function acceptorCommand(kind: WarmKind): string {
  const start = 'IFS= read -r __ocb_d && IFS= read -r REPLY || exit 0; '
  const enter = '__ocb_o=${OLDPWD-}; cd -- "$__ocb_d" || exit 1; OLDPWD=$__ocb_o; unset __ocb_d __ocb_o; . "$REPLY"'
  // bash reads a pipe byte by byte for `read`, so the hook input that follows
  // the two lines is still unread when the script starts.
  if (kind === 'bash-hook') return start + enter
  return kind === 'bash'
    ? start + 'exec 0</dev/null; ' + enter
    : // Encoding setup and a first Invoke-Expression run while waiting: they
      // are the slow first-use part of every PowerShell command script.
      '[Console]::OutputEncoding = $OutputEncoding = New-Object System.Text.UTF8Encoding $false; ' +
        "Invoke-Expression '$null = Get-Location'; " +
        '$__ocb_d = [Console]::In.ReadLine(); if ($null -eq $__ocb_d) { exit 0 }; Set-Location -LiteralPath $__ocb_d; ' +
        '[System.IO.Directory]::SetCurrentDirectory($__ocb_d); Remove-Variable __ocb_d; ' +
        'Invoke-Expression ([System.IO.File]::ReadAllText([Console]::In.ReadLine()))'
}

interface Waiting {
  key: string
  kind: WarmKind
  child: ChildProcess
  timer: ReturnType<typeof setTimeout>
}

const waiting = new Map<string, Waiting>()

export function warmKey(shell: string, flags: string[], env: NodeJS.ProcessEnv): string {
  return JSON.stringify([shell, flags, Object.entries(env).sort(([a], [b]) => (a < b ? -1 : 1))])
}

function startWaiting(shell: string, flags: string[], kind: WarmKind, env: NodeJS.ProcessEnv): ChildProcess {
  const runner = windowsJobRunner()
  const argv = [...flags, acceptorCommand(kind)]
  const child = spawn(runner ?? shell, runner ? [shell, ...argv] : argv, {
    stdio: ['pipe', 'pipe', 'pipe'],
    windowsHide: true,
    detached: process.platform !== 'win32',
    env,
    cwd: os.tmpdir(),
  })
  child.on('error', () => {
    /* surfaced as an unusable process when taken */
  })
  child.stdin?.on('error', () => {
    /* the process ended before its script was written */
  })
  return child
}

function discard(entry: Waiting): void {
  clearTimeout(entry.timer)
  if (waiting.get(entry.key) === entry) waiting.delete(entry.key)
  entry.child.stdin?.destroy()
  try {
    entry.child.kill()
  } catch {
    /* already gone */
  }
}

/** Takes the waiting shell for `key` (null if none or it died) and starts the
 *  next one in the background. A different key ends the stale waiting shell. */
export function takeWarmShell(
  key: string,
  shell: string,
  flags: string[],
  kind: WarmKind,
  env: NodeJS.ProcessEnv,
): ChildProcess | null {
  let taken: ChildProcess | null = null
  for (const entry of [...waiting.values()]) {
    if (entry.key !== key) {
      // One waiting shell per kind: a new environment replaces it. Hooks keep
      // a few, since the hooks of different plugins alternate around each call.
      if (entry.kind === kind && kind !== 'bash-hook') discard(entry)
      continue
    }
    clearTimeout(entry.timer)
    waiting.delete(key)
    const alive = entry.child.exitCode === null && entry.child.signalCode === null && entry.child.pid !== undefined
    if (alive && entry.child.stdin?.writable) taken = entry.child
    else discard(entry)
  }
  const child = startWaiting(shell, flags, kind, env)
  const entry: Waiting = { key, kind, child, timer: setTimeout(() => discard(entry), IDLE_MS) }
  entry.timer.unref?.()
  child.unref()
  ;(child.stdout as unknown as { unref?: () => void })?.unref?.()
  ;(child.stderr as unknown as { unref?: () => void })?.unref?.()
  ;(child.stdin as unknown as { unref?: () => void })?.unref?.()
  child.once('exit', () => {
    if (waiting.get(key) === entry) discard(entry)
  })
  waiting.set(key, entry)
  const hooks = [...waiting.values()].filter((e) => e.kind === 'bash-hook')
  if (hooks.length > MAX_WAITING_HOOKS) discard(hooks[0]!)
  return taken
}

/** Hands `scriptPath` to a taken shell; it starts running the script now.
 *  With `keepStdin` the caller writes the script's stdin and ends it. */
export function startWarmShell(child: ChildProcess, cwd: string, scriptPath: string, keepStdin = false): ChildProcess {
  ;(child as unknown as { ref?: () => void }).ref?.()
  for (const stream of [child.stdout, child.stderr, child.stdin]) (stream as unknown as { ref?: () => void })?.ref?.()
  const start = `${cwd}\n${scriptPath}\n`
  if (keepStdin) child.stdin?.write(start)
  else child.stdin?.end(start)
  return child
}

/** Ends every waiting shell (host shutdown, tests). */
export function disposeWarmShells(): void {
  for (const entry of [...waiting.values()]) discard(entry)
}

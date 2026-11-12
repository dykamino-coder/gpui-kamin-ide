// A shell script of any length is handed to its shell through a private file,
// never as a command-line argument.
//
// Git for Windows Bash (MSYS2 runtime) silently cuts a double-quoted command
// line argument after about 8 K characters: `bash -c <script>` then executes a
// prefix of the script and may report success. A cut `rm -rf /a/b/c` becomes
// `rm -rf /a/`. cmd.exe refuses lines over 8191 characters and CreateProcess
// lines over 32767. The script therefore lives in a file and the shell
// receives only a short, fixed command that reads that file. stdin stays the
// command's own stream, so commands that read stdin do not consume the script.
import { randomBytes } from 'node:crypto'
import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { powerShellSingleQuote, shellSingleQuote, toPosixShellPath } from './command-runtime'

export type ScriptShellKind = 'bash' | 'sh' | 'zsh' | 'powershell'

export interface ScriptFile {
  readonly path: string
  /** Removes the file. Safe to call more than once. */
  dispose(): void
}

export interface ScriptInvocation {
  /** Shell arguments: the given flags followed by `loader`. */
  args: string[]
  /** The short fixed command that runs the file in place of the script. */
  loader: string
  file: ScriptFile
}

const PREFIX = 'cmd-'
const STALE_MS = 24 * 3600_000
const RETRY_MS = 2000
let swept = false

/** The Bridge's own directory for command scripts, readable only by the user. */
export function scriptDirectory(): string {
  return path.join(os.tmpdir(), 'kamin-bridge-scripts')
}

// A host that was killed cannot delete its scripts; remove day-old leftovers.
function sweepStale(directory: string): void {
  if (swept) return
  swept = true
  try {
    const now = Date.now()
    for (const name of fs.readdirSync(directory)) {
      if (!name.startsWith(PREFIX)) continue
      const file = path.join(directory, name)
      try {
        if (now - fs.statSync(file).mtimeMs > STALE_MS) fs.unlinkSync(file)
      } catch {
        /* in use or already gone */
      }
    }
  } catch {
    /* best effort */
  }
}

function remove(file: string, retry: boolean): void {
  try {
    fs.unlinkSync(file)
  } catch (err) {
    const code = (err as NodeJS.ErrnoException).code
    if (code === 'ENOENT' || !retry) return
    // Windows refuses to delete a file that a dying process still holds open.
    setTimeout(() => remove(file, false), RETRY_MS).unref()
  }
}

/** Writes `content` exactly (UTF-8 without BOM, line endings untouched). */
export function writeScriptFile(content: string, extension: '.sh' | '.ps1'): ScriptFile {
  const directory = scriptDirectory()
  fs.mkdirSync(directory, { recursive: true, mode: 0o700 })
  sweepStale(directory)
  const file = path.join(directory, `${PREFIX}${process.pid}-${randomBytes(8).toString('hex')}${extension}`)
  fs.writeFileSync(file, content, { encoding: 'utf8', flag: 'wx', mode: 0o600 })
  let disposed = false
  return {
    path: file,
    dispose: () => {
      if (disposed) return
      disposed = true
      remove(file, true)
    },
  }
}

// -Command sets exit code 1 when the last statement failed; Invoke-Expression
// itself succeeds, so the script reports that status explicitly. The newline
// ends a trailing comment of the user's script.
const POWERSHELL_STATUS_TRAILER = '\nif (-not $?) { exit 1 }\n'

/**
 * Writes `script` to a file and returns the arguments that run it.
 *
 * `flags` are the shell's flags ending with its command flag (`-c`, `-lc`,
 * `-Command`). POSIX shells source the file, so `$0`, `$#`, `exit`, `set -e`
 * and the exit status behave as with `-c`; `BASH_SOURCE` names the file.
 * PowerShell evaluates the file text in the global scope like `-Command`, and
 * is not subject to the script execution policy, as `-Command` is not.
 */
export function scriptInvocation(
  kind: ScriptShellKind,
  flags: string[],
  script: string,
  platform: NodeJS.Platform = process.platform,
): ScriptInvocation {
  if (kind === 'powershell') {
    const file = writeScriptFile(script + POWERSHELL_STATUS_TRAILER, '.ps1')
    const loader = `Invoke-Expression ([System.IO.File]::ReadAllText(${powerShellSingleQuote(file.path)}))`
    return { args: [...flags, loader], loader, file }
  }
  const file = writeScriptFile(script, '.sh')
  const loader = `. ${shellSingleQuote(toPosixShellPath(file.path, platform))}`
  return { args: [...flags, loader], loader, file }
}

/** cmd.exe rejects a command line longer than this many UTF-16 units. */
export const CMD_LINE_LIMIT = 8191

/**
 * Fails before start when `spawn(command, args, { shell: true })` on Windows
 * would exceed cmd.exe's line limit. cmd.exe would refuse it too, but only
 * after start and with a message that is easy to miss in a log.
 */
export function assertCmdLineFits(
  command: string,
  args: readonly string[] = [],
  platform: NodeJS.Platform = process.platform,
  comspec: string = process.env.ComSpec ?? 'cmd.exe',
): void {
  if (platform !== 'win32') return
  const line = [command, ...args].join(' ')
  // Node runs `<comspec> /d /s /c "<line>"`.
  const length = `${comspec} /d /s /c "${line}"`.length
  if (length > CMD_LINE_LIMIT) {
    throw new Error(
      `Command is too long for cmd.exe (${length} > ${CMD_LINE_LIMIT} characters); it was not started. ` +
        'Move the command into a script file and run that file.',
    )
  }
}

// Shell snapshot — mirrors Claude Code CLI's `ShellSnapshot` approach.
//
// Our Bash tool spawns a fresh `bash -c <command>` per call (no persistent
// shell). To give each call what an interactive shell of the user has without
// re-running `.bashrc`/`.profile` every time, we capture once:
//
//   - functions (`declare -fp`), except `_*` completion helpers;
//   - aliases, with `expand_aliases` on so they work under `bash -c`;
//   - `shopt` options;
//   - PATH entries the profile added, prepended to the PATH of each call;
//   - environment variables the profile exported or changed.
//
// Only what the profile itself changed is replayed. A full `declare -p` dump
// (the previous approach) re-imposed the capture-time PATH and environment on
// every later call — dropping plugin `bin/` directories and any variable the
// host set after capture — and tried to overwrite bash's read-only variables.
//
// The snapshot file lives under `%TEMP%/ocb-shell-snapshot-<pid>.sh`. We fall
// back to a login shell (`bash -lc`) if snapshot creation fails.

import fs from 'fs'
import os from 'os'
import path from 'path'
import { spawn } from 'child_process'

let snapshotPath: string | null = null
let snapshotAttempted = false
let snapshotAvailable = false
let snapshotPromise: Promise<string | null> | null = null

const SEPARATOR = '\u0001OCB-SNAPSHOT\u0001'

// Variables bash, MSYS or the terminal set by themselves on every start, or
// that describe one process rather than the user's setup.
const SKIP_VARIABLES = new Set([
  'PATH', 'PWD', 'OLDPWD', 'SHLVL', '_', 'HOME', 'TMP', 'TEMP', 'TMPDIR', 'SHELL', 'TERM',
  'PS1', 'PS2', 'PS4', 'PROMPT_COMMAND', 'COLUMNS', 'LINES', 'HOSTNAME', 'HOSTTYPE', 'MACHTYPE',
  'OSTYPE', 'SSH_AGENT_PID', 'SSH_AUTH_SOCK', 'EXECIGNORE', 'MSYSTEM', 'MSYSTEM_CARCH',
  'MSYSTEM_CHOST', 'MSYSTEM_PREFIX', 'MINGW_CHOST', 'MINGW_PACKAGE_PREFIX', 'MINGW_PREFIX',
  'CONFIG_SITE', 'ACLOCAL_PATH', 'MANPATH', 'INFOPATH', 'PKG_CONFIG_PATH', 'PKG_CONFIG_SYSTEM_INCLUDE_PATH',
  'PKG_CONFIG_SYSTEM_LIBRARY_PATH', 'SHELLOPTS', 'BASHOPTS', 'GIT_EDITOR', 'CLAUDECODE',
])

export function buildSnapshotScript(): string {
  // Sections separated by SEPARATOR lines. The login PATH is compared with the
  // PATH of a plain `bash -c` (see `plainPath`) in the same POSIX form.
  return [
    `printf '%s\\n' '${SEPARATOR}functions'`,
    `for f in $(compgen -A function); do case "$f" in _*) ;; *) declare -f "$f" ;; esac; done 2>/dev/null`,
    `printf '%s\\n' '${SEPARATOR}aliases'`,
    'alias -p 2>/dev/null',
    `printf '%s\\n' '${SEPARATOR}shopt'`,
    'shopt -p 2>/dev/null',
    `printf '%s\\n' '${SEPARATOR}path'`,
    'printf "%s\\n" "$PATH"',
    `printf '%s\\n' '${SEPARATOR}env'`,
    'for n in $(compgen -e); do printf "%s\\0%s\\0" "$n" "${!n}"; done',
  ].join('\n')
}

function shellQuote(value: string): string {
  return `'${value.replace(/'/g, `'\\''`)}'`
}

/** Snapshot file text from the capture output; `baseEnv` is the environment
 *  the capture shell inherited. */
export function renderSnapshot(
  output: string,
  baseEnv: NodeJS.ProcessEnv,
  platform = process.platform,
  plainPath = '',
): string {
  const sections = new Map<string, string>()
  for (const part of output.split(`${SEPARATOR}`).slice(1)) {
    const newline = part.indexOf('\n')
    sections.set(part.slice(0, newline), part.slice(newline + 1))
  }
  const lines = ['# open-claude-bridge shell snapshot (auto-generated)']
  lines.push(sections.get('functions')?.trimEnd() ?? '')
  lines.push('shopt -s expand_aliases')
  lines.push(sections.get('shopt')?.trimEnd() ?? '')
  // Git for Windows aliases `node`/`python`/... to `winpty <exe>`, which
  // fails without a console ("stdin is not a tty"); native Claude Code drops
  // them too.
  const aliases = (sections.get('aliases') ?? '').split('\n').filter((line) => line && !/\bwinpty\b/.test(line))
  lines.push(aliases.join('\n'))
  // PATH entries the profile added: login PATH minus the PATH of a plain shell.
  const plain = new Set(plainPath.split(':'))
  const added = [...new Set((sections.get('path') ?? '').trim().split(':'))].filter((p) => p && !plain.has(p))
  if (added.length) lines.push(`export PATH=${shellQuote(added.join(':'))}:"$PATH"`)
  const env = (sections.get('env') ?? '').split('\0')
  const caseInsensitive = platform === 'win32'
  const base = new Map(Object.entries(baseEnv).map(([k, v]) => [caseInsensitive ? k.toUpperCase() : k, v]))
  for (let i = 0; i + 1 < env.length; i += 2) {
    const name = env[i]!.replace(/^\n+/, '')
    const value = env[i + 1]!
    if (!/^[A-Za-z_][A-Za-z0-9_]*$/.test(name) || SKIP_VARIABLES.has(name) || name.startsWith('ORIGINAL_') ||
      name.startsWith('OCB_') || name.startsWith('BASH')) continue
    if (base.get(caseInsensitive ? name.toUpperCase() : name) === value) continue
    lines.push(`export ${name}=${shellQuote(value)}`)
  }
  return lines.filter(Boolean).join('\n') + '\n'
}

/** Capture a snapshot by running an interactive login bash. Returns the
 *  path to the snapshot file, or null on failure. Async — `spawnSync`
 *  previously blocked the main process event loop for up to 10s. */
/** PATH as a plain non-login `bash -c` sees it (MSYS converts it to POSIX). */
function plainShellPath(bashPath: string): Promise<string> {
  return new Promise((resolve) => {
    const proc = spawn(bashPath, ['-c', 'printf "%s" "$PATH"'], { windowsHide: true, stdio: ['ignore', 'pipe', 'ignore'] })
    const chunks: Buffer[] = []
    proc.stdout?.on('data', (d: Buffer) => chunks.push(d))
    proc.on('error', () => resolve(''))
    proc.on('close', () => resolve(Buffer.concat(chunks).toString('utf8')))
  })
}

async function captureSnapshot(bashPath: string): Promise<string | null> {
  const plain = plainShellPath(bashPath)
  const captured = await captureLoginShell(bashPath)
  if (!captured) return null
  const tmpFile = path.join(os.tmpdir(), `ocb-shell-snapshot-${process.pid}.sh`)
  try {
    fs.writeFileSync(tmpFile, renderSnapshot(captured, process.env, process.platform, await plain), 'utf-8')
  } catch {
    return null
  }
  return tmpFile
}

function captureLoginShell(bashPath: string): Promise<string | null> {
  return new Promise((resolve) => {
    // `-i -l -c` = interactive login shell — runs `.profile` / `.bashrc` so
    // user customisations populate the environment. Then dump them.
    const proc = spawn(bashPath, ['-ilc', buildSnapshotScript()], {
      windowsHide: true,
      stdio: ['ignore', 'pipe', 'pipe'],
    })
    const chunks: Buffer[] = []
    const timer = setTimeout(() => {
      try {
        proc.kill('SIGKILL')
      } catch {}
      resolve(null)
    }, 10_000)
    proc.stdout?.on('data', (d: Buffer) => chunks.push(d))
    proc.stderr?.on('data', () => {
      /* profile noise */
    })
    proc.on('error', () => {
      clearTimeout(timer)
      resolve(null)
    })
    proc.on('close', (code) => {
      clearTimeout(timer)
      const stdout = Buffer.concat(chunks).toString('utf8')
      resolve(code === 0 && stdout.includes(SEPARATOR) ? stdout : null)
    })
  })
}

/** Returns the path to the snapshot file, creating it on first call. Null
 *  if snapshot creation failed — callers should fall back to login shell.
 *  In-flight promise deduped so parallel bashExecute calls share one capture. */
export async function getOrCreateSnapshot(bashPath: string): Promise<string | null> {
  if (snapshotAttempted) return snapshotAvailable ? snapshotPath : null
  if (snapshotPromise) return snapshotPromise
  snapshotPromise = (async () => {
    snapshotPath = await captureSnapshot(bashPath)
    snapshotAvailable = Boolean(snapshotPath)
    snapshotAttempted = true
    return snapshotPath
  })()
  return snapshotPromise
}

/** Prepend either `source <snapshotFile>` (preferred) or nothing (login
 *  shell handles it, added via shell flag by the caller). Returns the
 *  final command string to pass to `bash -c`. */
export function wrapCommandWithSnapshot(command: string, snapshotFile: string | null): string {
  if (!snapshotFile) return command
  // `|| true` guards against the (very rare) case where the snapshot file
  // has been tmp-reaped between capture and now. Forward slashes: MSYS bash
  // reads `C:/...` without backslash-escape surprises.
  const file = process.platform === 'win32' ? snapshotFile.replace(/\\/g, '/') : snapshotFile
  return `source '${file.replace(/'/g, "'\\''")}' 2>/dev/null || true\n${command}`
}

/** Should callers use `bash -lc` (login shell) instead of `bash -c`? True
 *  iff no snapshot is available — then the login shell does its own init. */
export function shouldUseLoginShell(snapshotFile: string | null): boolean {
  return !snapshotFile
}

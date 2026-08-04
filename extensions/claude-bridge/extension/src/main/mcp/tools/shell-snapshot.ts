// Shell snapshot — mirrors Claude Code CLI's `ShellSnapshot` approach.
//
// Our Bash tool spawns a fresh `bash -c <command>` per call (no persistent
// shell). That means every call starts with an empty environment: aliases,
// functions, PATH tweaks and env vars the user set in a previous turn are
// GONE. To mimic the statefulness of an interactive shell without the
// complexity of a persistent PTY, we:
//
//   1. On first use, capture the user's login-shell setup into a tmp file
//      (by running `bash -ilc 'declare -p; declare -fp; alias'` etc.).
//   2. Prepend `source <snapshotFile>` to every subsequent command.
//
// This gives commands access to:
//   - Environment variables from `.profile`, `.bashrc`, `.bash_profile`
//     (e.g. PATH additions for pyenv, nvm, rbenv, corepack shims).
//   - User-defined aliases (`ll`, `gs`, etc.).
//   - User-defined functions.
//   - Exported variables set in profile scripts.
//
// The snapshot file lives under `%TEMP%/ocb-shell-snapshot-<pid>.sh` so it's
// per-process and auto-cleaned by the OS tmp reaper. We fall back to a
// login shell (`bash -lc`) if snapshot creation fails — which gives a
// similar behaviour at the cost of profile re-execution per call.

import fs from 'fs'
import os from 'os'
import path from 'path'
import { spawn } from 'child_process'

let snapshotPath: string | null = null
let snapshotAttempted = false
let snapshotAvailable = false
let snapshotPromise: Promise<string | null> | null = null

function buildSnapshotScript(): string {
  // `declare -p` dumps all shell variables with their attributes, `declare
  // -fp` dumps every function, `alias -p` dumps aliases. Joining with `;\n`
  // means we can source the file back safely. `shopt` flags are session-
  // only and not critical; we skip them.
  return [
    '# open-claude-bridge shell snapshot (auto-generated)',
    'declare -p 2>/dev/null || true',
    'declare -fp 2>/dev/null || true',
    'alias -p 2>/dev/null || true',
  ].join('\n')
}

/** Capture a snapshot by running an interactive login bash. Returns the
 *  path to the snapshot file, or null on failure. Async — `spawnSync`
 *  previously blocked the main process event loop for up to 10s. */
function captureSnapshot(bashPath: string): Promise<string | null> {
  const tmpFile = path.join(os.tmpdir(), `ocb-shell-snapshot-${process.pid}.sh`)
  return new Promise((resolve) => {
    // `-i -l -c` = interactive login shell — runs `.profile` / `.bashrc` so
    // user customisations populate the environment. Then dump everything.
    const proc = spawn(bashPath, ['-ilc', buildSnapshotScript()], {
      windowsHide: true,
      stdio: ['ignore', 'pipe', 'pipe'],
    })
    let stdout = ''
    const timer = setTimeout(() => { try { proc.kill('SIGKILL') } catch {} resolve(null) }, 10_000)
    proc.stdout?.on('data', d => { stdout += d.toString('utf-8') })
    proc.stderr?.on('data', () => { /* noise from `declare -p` on some shells */ })
    proc.on('error', () => { clearTimeout(timer); resolve(null) })
    proc.on('close', code => {
      clearTimeout(timer)
      if (code !== 0 || !stdout) return resolve(null)
      try { fs.writeFileSync(tmpFile, stdout, 'utf-8') } catch { return resolve(null) }
      resolve(tmpFile)
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
  // has been tmp-reaped between capture and now.
  return `source '${snapshotFile.replace(/'/g, "'\\''")}' 2>/dev/null || true\n${command}`
}

/** Should callers use `bash -lc` (login shell) instead of `bash -c`? True
 *  iff no snapshot is available — then the login shell does its own init. */
export function shouldUseLoginShell(snapshotFile: string | null): boolean {
  return !snapshotFile
}

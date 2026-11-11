// Finds the Windows job runner (`kamin-jobrun.exe`, crates/jobrun). A command
// started through it lives in its own Job Object: ending the runner ends every
// process of the command, including MSYS children whose Windows parent has
// already exited and which `taskkill /T` therefore cannot find (INC-2026-0096).
import { spawnSync } from 'node:child_process'
import { existsSync } from 'node:fs'
import { dirname, join } from 'node:path'

const RUNNER = 'kamin-jobrun.exe'

export interface RunnerProbe {
  platform: NodeJS.Platform
  env: NodeJS.ProcessEnv
  /** The extension host executable: `<install>\runtime\kaminhost.exe`. */
  execPath: string
  exists: (path: string) => boolean
  /** True when `<runner> --probe` exits 0: a Job Object can be set up here. */
  works: (runner: string) => boolean
}

/** The runner to use, or null when this host has none that works. */
export function findWindowsJobRunner(probe: RunnerProbe): string | null {
  if (probe.platform !== 'win32') return null
  const hostDir = dirname(probe.execPath)
  const candidates = [
    probe.env.KAMIN_JOBRUN_EXE,
    join(hostDir, '..', RUNNER),
    join(hostDir, RUNNER),
  ]
  for (const candidate of candidates) {
    if (candidate && probe.exists(candidate)) return probe.works(candidate) ? candidate : null
  }
  return null
}

function works(runner: string): boolean {
  try {
    return spawnSync(runner, ['--probe'], { windowsHide: true, timeout: 5000 }).status === 0
  } catch {
    return false
  }
}

let cached: string | null | undefined

/** Resolved once per extension host: the install does not change under it. */
export function windowsJobRunner(): string | null {
  cached ??= findWindowsJobRunner({
    platform: process.platform,
    env: process.env,
    execPath: process.execPath,
    exists: existsSync,
    works,
  })
  return cached
}
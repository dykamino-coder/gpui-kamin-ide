// Finds the ripgrep binary of the Grep tool. The installer ships `rg.exe` next
// to `kamin-jobrun.exe` (`<install>\rg.exe`, the parent of `runtime\`), because
// Windows hosts rarely have ripgrep on PATH: without it every Grep fell back to
// the in-process walker, ~40x slower on a large repo and without ripgrep's
// glob/type/context/.gitignore semantics.
import { existsSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { whichExecutable } from '../../utils/command-runtime'

export interface RipgrepProbe {
  platform: NodeJS.Platform
  env: NodeJS.ProcessEnv
  /** The extension host executable: `<install>\runtime\kaminhost.exe`. */
  execPath: string
  exists: (path: string) => boolean
  which: (command: string) => Promise<string | null>
}

/** Explicit override, then the bundled binary, then PATH; null when none. */
export async function findRipgrep(probe: RipgrepProbe): Promise<string | null> {
  const exe = probe.platform === 'win32' ? 'rg.exe' : 'rg'
  const configured = probe.env.KAMIN_RG_PATH
  if (configured && probe.exists(configured)) return configured
  const hostDir = dirname(probe.execPath)
  for (const candidate of [join(hostDir, '..', exe), join(hostDir, exe)]) {
    if (probe.exists(candidate)) return candidate
  }
  return probe.which('rg')
}

let cached: Promise<string | null> | undefined

/** Resolved once per extension host: neither the install nor PATH of this
 *  process changes under it. */
export function ripgrepPath(): Promise<string | null> {
  cached ??= findRipgrep({
    platform: process.platform,
    env: process.env,
    execPath: process.execPath,
    exists: existsSync,
    which: (command) => whichExecutable(command),
  })
  return cached
}

/** Test hook: forget the resolved binary. */
export function resetRipgrepPathForTests(): void {
  cached = undefined
}

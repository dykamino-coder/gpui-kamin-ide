// Starts the one-time work of the first Bash/PowerShell/Grep call when a Bridge
// session connects instead of inside that call: the shell snapshot (an
// interactive login bash, ~1 s on Windows), the bash/pwsh/rg lookups and the
// plugin PATH. The first tool call then costs what every later one costs.
import { getOrCreateSnapshot } from './shell-snapshot'
import { ripgrepPath } from './ripgrep-path'
import { listEnabledPluginBinDirs } from '../../plugin-helpers'
import { findBashExecutable, findPowerShellExecutable } from '../../utils/command-runtime'

let started: Promise<void> | null = null

export function prewarmShellTools(): Promise<void> {
  started ??= (async () => {
    await Promise.allSettled([
      findBashExecutable().then((bash) => getOrCreateSnapshot(bash)),
      process.platform === 'win32' ? findPowerShellExecutable() : Promise.resolve(),
      ripgrepPath(),
      listEnabledPluginBinDirs(),
    ])
  })()
  return started
}

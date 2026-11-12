// ============================================================================
// Which codex executable Bridge launches.
//
// npm installs codex as a Node launcher (bin/codex.js) that execs the native
// binary. On Windows the PATH entry is a .cmd/.ps1 shim; spawning it would go
// through cmd.exe quoting, which mangles `-c key="value"` overrides, so the
// launcher is run with the current Node directly. The container image puts
// `codex` on PATH. BRIDGE_CODEX_BIN pins an explicit binary or launcher.
// ============================================================================

import fs from 'node:fs'
import path from 'node:path'

export const CODEX_BIN_ENV = 'BRIDGE_CODEX_BIN'

export interface CodexCommand {
  command: string
  /** Prefix arguments before the codex subcommand (the launcher script). */
  args: string[]
}

export interface ResolveCodexOptions {
  env?: NodeJS.ProcessEnv
  platform?: NodeJS.Platform
  exists?: (file: string) => boolean
  nodePath?: string
}

const isScript = (file: string) => /\.(c|m)?js$/i.test(file)

export function resolveCodexCommand(options: ResolveCodexOptions = {}): CodexCommand {
  const env = options.env ?? process.env
  const platform = options.platform ?? process.platform
  const exists = options.exists ?? fs.existsSync
  const node = options.nodePath ?? process.execPath
  const pinned = env[CODEX_BIN_ENV]
  if (pinned) return isScript(pinned) ? { command: node, args: [pinned] } : { command: pinned, args: [] }
  if (platform === 'win32' && env.APPDATA) {
    const launcher = path.win32.join(env.APPDATA, 'npm', 'node_modules', '@openai', 'codex', 'bin', 'codex.js')
    if (exists(launcher)) return { command: node, args: [launcher] }
  }
  return { command: 'codex', args: [] }
}

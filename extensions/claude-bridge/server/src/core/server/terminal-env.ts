import { defaultCodexHome } from '../codex/codex-home'
import { injectProxyEnv } from '../config/settings'

/** Keep the login shell's HOME, but always use Bridge's shared Codex account. */
export function terminalEnv(
  base: NodeJS.ProcessEnv = process.env,
  home = defaultCodexHome(base),
): Record<string, string> {
  const env = Object.fromEntries(
    Object.entries(base).filter((entry): entry is [string, string] => entry[1] !== undefined),
  )
  delete env.CLAUDECODE
  delete env.CLAUDE_CODE_SESSION
  injectProxyEnv(env)
  env.CODEX_HOME = home
  return env
}

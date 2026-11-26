import { applyCodexNetworkEnv, defaultCodexHome } from '../codex/codex-home'
import { injectProxyEnv } from '../config/settings'

/**
 * Keep the login shell's HOME, but always use Bridge's shared Codex account.
 * `codex login` runs here, so the shell gets the same proxy bypass and CA as
 * the Bridge codex child (`injectProxy` is replaceable only for unit tests).
 */
export function terminalEnv(
  base: NodeJS.ProcessEnv = process.env,
  home = defaultCodexHome(base),
  injectProxy: (env: Record<string, string>) => void = injectProxyEnv,
): Record<string, string> {
  const env = Object.fromEntries(
    Object.entries(base).filter((entry): entry is [string, string] => entry[1] !== undefined),
  )
  delete env.CLAUDECODE
  delete env.CLAUDE_CODE_SESSION
  injectProxy(env)
  applyCodexNetworkEnv(env)
  env.CODEX_HOME = home
  return env
}

// NO_PROXY merging shared by every Bridge-spawned agent process (Claude CLI,
// codex app-server). Kept dependency-free so callers need not load settings.

/**
 * Adds loopback (and `extra`) to NO_PROXY/no_proxy, keeping existing entries
 * and their order. Bridge's MCP/hook endpoints listen on 127.0.0.1, so a
 * child process configured with a proxy must still reach them directly.
 */
export function mergeLoopbackNoProxy(env: Record<string, string | undefined>, extra?: string | null): void {
  const loopback = '127.0.0.1,localhost,::1'
  const parts = [extra, env.NO_PROXY, env.no_proxy, loopback].filter(Boolean)
  const merged = [
    ...new Set(
      parts
        .join(',')
        .split(',')
        .map((s) => s.trim())
        .filter(Boolean),
    ),
  ].join(',')
  env.NO_PROXY = merged
  env.no_proxy = merged
}

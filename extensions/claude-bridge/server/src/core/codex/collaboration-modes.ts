import type { CodexHost } from './codex-host'

export type CollaborationMode = 'default' | 'plan'
export const isCollaborationMode = (value: unknown): value is CollaborationMode =>
  value === 'default' || value === 'plan'

const catalogs = new WeakMap<CodexHost, Promise<CollaborationMode[]>>()

/** One preset list per running app-server. A failed probe leaves ordinary turns available. */
export function collaborationModes(host: CodexHost): Promise<CollaborationMode[]> {
  const cached = catalogs.get(host)
  if (cached) return cached
  const catalog = host.rpc
    .request<{ data: Array<{ mode?: unknown }> }>('collaborationMode/list', {}, { timeoutMs: 2000 })
    .then(({ data }) => [...new Set(data.map((entry) => entry.mode).filter(isCollaborationMode))])
    .catch(() => {
      catalogs.delete(host)
      return []
    })
  catalogs.set(host, catalog)
  return catalog
}

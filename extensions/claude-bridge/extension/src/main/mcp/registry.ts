import type { McpRegistryEntry } from '../../shared/types'

// The public MCP registry. Without `version=latest&visibility=commercial` the
// endpoint answers with an empty list, and it pages with an opaque cursor.
const REGISTRY_URL = 'https://api.anthropic.com/mcp-registry/v0/servers'
const PAGE_SIZE = 100
const MAX_PAGES = 20

export interface McpRegistryList { ok: boolean; servers: McpRegistryEntry[]; error?: string }

type Json = Record<string, unknown>
const isRegistryObject = (value: unknown): value is Json => typeof value === 'object' && value !== null && !Array.isArray(value)
const registryText = (value: unknown): string | undefined => typeof value === 'string' && value.trim() ? value : undefined

function remoteInstall(remotes: unknown): McpRegistryEntry['install'] {
  if (!Array.isArray(remotes)) return undefined
  for (const remote of remotes) {
    if (!isRegistryObject(remote)) continue
    const url = registryText(remote.url)
    if (!url) continue
    if (remote.type === 'streamable-http' || remote.type === 'http') return { type: 'http', url }
    if (remote.type === 'sse') return { type: 'sse', url }
  }
  return undefined
}

function packageInstall(packages: unknown): McpRegistryEntry['install'] {
  if (!Array.isArray(packages)) return undefined
  for (const pkg of packages) {
    if (!isRegistryObject(pkg)) continue
    const identifier = registryText(pkg.identifier)
    const transport = isRegistryObject(pkg.transport) ? pkg.transport.type : 'stdio'
    if (!identifier || transport !== 'stdio') continue
    if (pkg.registryType === 'npm') return { type: 'stdio', command: registryText(pkg.runtimeHint) ?? 'npx', args: ['-y', identifier] }
    if (pkg.registryType === 'pypi') return { type: 'stdio', command: registryText(pkg.runtimeHint) ?? 'uvx', args: [identifier] }
  }
  return undefined
}

/** One registry record (`{ server: {...} }`, or the bare server object) as the entry the UI lists. */
export function toRegistryEntry(record: unknown): McpRegistryEntry | null {
  const server = isRegistryObject(record) && isRegistryObject(record.server) ? record.server : record
  if (!isRegistryObject(server)) return null
  const name = registryText(server.name)
  if (!name) return null
  const entry: McpRegistryEntry = { name }
  const title = registryText(server.title)
  if (title) entry.title = title
  const description = registryText(server.description)
  if (description) entry.description = description
  const repository = isRegistryObject(server.repository) ? registryText(server.repository.url) : registryText(server.repository)
  if (repository) entry.repository = repository
  const homepage = registryText(server.websiteUrl) ?? registryText(server.homepage)
  if (homepage) entry.homepage = homepage
  const install = remoteInstall(server.remotes) ?? packageInstall(server.packages)
  if (install) entry.install = install
  return entry
}

/** Every latest registry entry, following the cursor up to a fixed page bound. */
export async function listMcpRegistry(fetchPage: typeof fetch = fetch): Promise<McpRegistryList> {
  const servers: McpRegistryEntry[] = []
  try {
    let cursor: string | undefined
    for (let page = 0; page < MAX_PAGES; page++) {
      const url = new URL(REGISTRY_URL)
      url.searchParams.set('version', 'latest')
      url.searchParams.set('visibility', 'commercial')
      url.searchParams.set('limit', String(PAGE_SIZE))
      if (cursor) url.searchParams.set('cursor', cursor)
      const res = await fetchPage(url, { headers: { 'User-Agent': 'open-claude-bridge' } })
      if (!res.ok) return { ok: false, error: `HTTP ${res.status}`, servers }
      const data = await res.json() as unknown
      const records = Array.isArray(data) ? data : isRegistryObject(data) && Array.isArray(data.servers) ? data.servers : []
      for (const record of records) {
        const entry = toRegistryEntry(record)
        if (entry) servers.push(entry)
      }
      const next = isRegistryObject(data) && isRegistryObject(data.metadata) ? registryText(data.metadata.nextCursor) : undefined
      if (!next || next === cursor || records.length === 0) break
      cursor = next
    }
    return { ok: true, servers }
  } catch (err) {
    return { ok: false, error: err instanceof Error ? err.message : String(err), servers }
  }
}

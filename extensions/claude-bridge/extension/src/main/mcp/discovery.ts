// Auto-discovery for MCP servers.
//
// Three sources scanned concurrently:
//   1. Installed plugins: <installPath>/.mcp.json + plugin.json:mcpServers
//   2. User's Claude config: ~/.claude.json → mcpServers
//   3. Project-level <cwd>/.mcp.json for every open tab's cwd
//
// Also exports the `.mcp.json` → ExternalMcpServerConfig parser and the
// env-var / home / user_config / CLAUDE_PLUGIN_ROOT expansion helpers,
// since parseMcpJson owns all of that resolution logic.

import { randomUUID } from 'crypto'
import path from 'path'
import fs from 'fs'
import os from 'os'
import type { ExternalMcpServerConfig } from '../../shared/types'
import { listEnabledInstalledPlugins, loadPluginOptions, substituteUserConfig } from '../plugin-helpers'

/** Substitute ${ENV_VAR} / ${ENV_VAR:-default} references with process.env values.
 *  The `:-default` form matches POSIX `${var:-default}` expansion: when the
 *  variable is unset OR empty, the literal default is used. */
export function resolveEnvVars(value: string): string {
  return value.replace(/\$\{([^}:]+)(?::-([^}]*))?\}/g, (_m, name: string, fallback?: string) => {
    const v = process.env[name]
    if (v !== undefined && v !== '') return v
    return fallback ?? ''
  })
}

/** Expand a leading `~/` or `~\` to the current user's home directory. Matches
 *  how shells expand paths in plugin config values like `PSM_DOTENV_PATH:
 *  "~/.claude/plugins/…"`. Only touches the prefix — a tilde in the middle of
 *  a string stays literal. */
export function expandHome(value: string): string {
  if (value === '~') return os.homedir()
  if (value.startsWith('~/') || value.startsWith('~\\')) {
    return path.join(os.homedir(), value.slice(2))
  }
  return value
}

/** Parse a .mcp.json file into ExternalMcpServerConfig entries. When the
 *  source is a plugin (pluginId = `name@marketplace`), also resolves
 *  `${user_config.KEY}` refs using the saved plugin options from
 *  `settings.json:pluginConfigs` + `.credentials.json:pluginSecrets`. */
export function parseMcpJson(parsed: Record<string, any>, sourcePath: string, pluginId?: string): ExternalMcpServerConfig[] {
  const configs: ExternalMcpServerConfig[] = []

  // Check for nested mcpServers format: { "mcpServers": { "name": { ... } } }
  const root = parsed.mcpServers ?? parsed

  // ${CLAUDE_PLUGIN_ROOT} should resolve to the plugin root. `sourcePath`
  // may be `<plugin>/.mcp.json` (root is dirname) OR
  // `<plugin>/.claude-plugin/plugin.json` (root is dirname's dirname).
  let pluginRoot = path.dirname(sourcePath)
  if (path.basename(pluginRoot) === '.claude-plugin') {
    pluginRoot = path.dirname(pluginRoot)
  }
  // Expand in this order: ${user_config.*} → ${CLAUDE_PLUGIN_ROOT} →
  // ${CLAUDE_PLUGIN_DATA} → env vars → leading ~. `user_config` first so the
  // token resolves to its value BEFORE resolveEnvVars' greedy `${VAR}` regex
  // could swallow it (the same gotcha that CLAUDE_PLUGIN_ROOT has).
  // CLAUDE_PLUGIN_DATA / ROOT next for the same reason. Tilde is last so
  // paths that come out of an env var expansion also get expanded.
  const userConfig = pluginId ? loadPluginOptions(pluginId) : null
  // Mirror CLI's `getPluginDataDir`: ~/.claude/plugins/data/<sanitizedId>/
  // (sanitized: non-alnum/`-`/`_` → `-`). Created lazily on first access
  // so the dir exists before subprocesses try to write to it.
  const pluginDataDir = pluginId ? path.join(os.homedir(), '.claude', 'plugins', 'data', pluginId.replace(/[^a-zA-Z0-9\-_]/g, '-')) : null
  let dataDirEnsured = false
  const ensureDataDir = (): string | null => {
    if (!pluginDataDir) return null
    if (!dataDirEnsured) {
      try { fs.mkdirSync(pluginDataDir, { recursive: true }) } catch { /* best effort */ }
      dataDirEnsured = true
    }
    return pluginDataDir
  }
  const resolveAll = (s: string): string => {
    let out = s
    if (userConfig) out = substituteUserConfig(out, userConfig)
    out = out.replace(/\$\{CLAUDE_PLUGIN_ROOT\}/g, pluginRoot)
    if (out.includes('${CLAUDE_PLUGIN_DATA}')) {
      const d = ensureDataDir()
      if (d) out = out.replace(/\$\{CLAUDE_PLUGIN_DATA\}/g, d)
    }
    out = resolveEnvVars(out)
    return expandHome(out)
  }

  for (const [name, value] of Object.entries(root)) {
    if (name === 'mcpServers' || typeof value !== 'object' || value === null) continue

    const entry = value as Record<string, any>
    const serverType = entry.type === 'sse' ? 'sse' : entry.type === 'http' ? 'http' : entry.type === 'ws' ? 'ws' : 'stdio'
    // Per-server `disabled: true` skips the server entirely — same as if the
    // user manually toggled it off in the UI. Keeps parity with CLI which
    // honours this field on `.mcp.json` entries.
    if (entry.disabled === true) continue

    const config: ExternalMcpServerConfig = {
      id: randomUUID(),
      name,
      type: serverType,
      enabled: true,
      autoDiscovered: true,
      sourcePath,
    }

    if (serverType === 'http' || serverType === 'sse' || serverType === 'ws') {
      // Resolve env vars + plugin-root in url too (corporate deployments
      // sometimes embed ${REGION_HOST} or ${CLAUDE_PLUGIN_ROOT}-relative
      // paths in URLs).
      config.url = entry.url ? resolveAll(String(entry.url)) : entry.url
      if (entry.headers && typeof entry.headers === 'object') {
        config.headers = {}
        for (const [k, v] of Object.entries(entry.headers)) {
          config.headers[k] = resolveAll(String(v))
        }
      }
    } else {
      config.command = resolveAll(entry.command || '')
      config.args = Array.isArray(entry.args)
        ? entry.args.map((a: string) => resolveAll(String(a)))
        : entry.args
      if (entry.env && typeof entry.env === 'object') {
        const out: Record<string, string> = {}
        for (const [k, v] of Object.entries(entry.env)) out[k] = resolveAll(String(v))
        config.env = out
      }
      if (typeof entry.workingDirectory === 'string' && entry.workingDirectory.trim()) {
        ;(config as any).workingDirectory = resolveAll(entry.workingDirectory)
      }
    }

    configs.push(config)
  }

  return configs
}

/** Load plugin descriptions from marketplace.json files (async). */
async function loadMarketplaceDescriptionsAsync(pluginsDir: string): Promise<Map<string, string>> {
  const descriptions = new Map<string, string>()
  let marketplaces: string[]
  try { marketplaces = await fs.promises.readdir(pluginsDir) }
  catch { return descriptions }
  await Promise.allSettled(marketplaces.map(async (marketplace) => {
    const manifestPath = path.join(pluginsDir, marketplace, '.claude-plugin', 'marketplace.json')
    let raw: string
    try { raw = await fs.promises.readFile(manifestPath, 'utf-8') } catch { return }
    try {
      const data = JSON.parse(raw)
      if (!Array.isArray(data.plugins)) return
      for (const p of data.plugins) {
        if (p.name && p.description) {
          descriptions.set(`${p.name}@${marketplace}`, p.description)
        }
      }
    } catch { /* skip */ }
  }))
  return descriptions
}

// Discover MCP servers declared by installed+enabled plugins.
//
// Source of truth is `~/.claude/plugins/installed_plugins.json` →
// `entry.installPath`. We do NOT walk `marketplaces/<m>/external_plugins/`
// or `marketplaces/<m>/plugins/mcp/` anymore — different CLI versions
// lay plugins out differently (current versions use `plugins/cache/<m>/<p>/<version>/`),
// and the `installPath` field is the only cross-version-stable pointer.
//
// For each enabled plugin we read two places, same as the CLI:
//   1. `<installPath>/.mcp.json` (standalone)
//   2. `<installPath>/.claude-plugin/plugin.json:mcpServers` (inline, 3 shapes)
export async function discoverPluginConfigs(pluginsDir: string): Promise<ExternalMcpServerConfig[]> {
  const [plugins, marketplaceDescriptions] = await Promise.all([
    listEnabledInstalledPlugins(),
    loadMarketplaceDescriptionsAsync(pluginsDir),
  ])

  const nested = await Promise.all(plugins.map(async ({ name, marketplace, installPath }) => {
    const sources: Array<{ path: string; raw: string }> = []

    // 1. Standalone .mcp.json at the plugin root.
    const mcpJson = path.join(installPath, '.mcp.json')
    const mcpRaw = await fs.promises.readFile(mcpJson, 'utf-8').catch(() => null)
    if (mcpRaw !== null) sources.push({ path: mcpJson, raw: mcpRaw })

    // 2. `mcpServers` field inside the plugin manifest. Also grab the
    //    manifest-level `description` so it beats the marketplace.json
    //    fallback — plugin authors usually put a better blurb there.
    const manifestJson = path.join(installPath, '.claude-plugin', 'plugin.json')
    const manifestRaw = await fs.promises.readFile(manifestJson, 'utf-8').catch(() => null)
    let manifestDescription: string | undefined
    if (manifestRaw !== null) {
      try {
        const m = JSON.parse(manifestRaw)
        if (typeof m?.description === 'string' && m.description.trim()) {
          manifestDescription = m.description.trim()
        }
        const spec = m?.mcpServers
        // CLI accepts four shapes:
        //   1. string — path to a .mcp.json / .mcpb relative to the plugin
        //   2. array — mixed strings (paths) + inline server objects
        //   3. object — inline { name: config } record
        //   4. missing — no MCP servers declared via manifest
        const items: unknown[] = Array.isArray(spec) ? spec
          : (typeof spec === 'string' || (spec && typeof spec === 'object' && !Array.isArray(spec))) ? [spec]
          : []

        for (const item of items) {
          if (typeof item === 'string') {
            if (item.toLowerCase().endsWith('.mcpb') || item.toLowerCase().endsWith('.dxt')) {
              console.warn(`[MCP] ${name}: .mcpb bundles not supported (${item})`)
              continue
            }
            const resolved = path.isAbsolute(item) ? item : path.join(installPath, item)
            const raw = await fs.promises.readFile(resolved, 'utf-8').catch(() => null)
            if (raw !== null) sources.push({ path: resolved, raw })
          } else if (item && typeof item === 'object' && !Array.isArray(item)) {
            // Inline object — wrap in `mcpServers` so parseMcpJson reads it.
            // Point the sourcePath at the manifest so `${CLAUDE_PLUGIN_ROOT}`
            // resolves relative to the plugin root (parent of `.claude-plugin/`).
            sources.push({ path: manifestJson, raw: JSON.stringify({ mcpServers: item }) })
          }
        }
      } catch { /* ignore malformed manifest */ }
    }

    if (sources.length === 0) return []
    const desc = manifestDescription || marketplaceDescriptions.get(`${name}@${marketplace}`)
    const pluginId = `${name}@${marketplace}`
    const out: ExternalMcpServerConfig[] = []
    for (const src of sources) {
      let parsed: any
      try { parsed = JSON.parse(src.raw) } catch { continue }
      const configs = parseMcpJson(parsed, src.path, pluginId)
      for (const config of configs) {
        config.sourceType = 'plugin'
        config.pluginName = name
        config.marketplace = marketplace
        if (desc) config.description = desc
      }
      out.push(...configs)
    }
    return out
  }))
  return nested.flat()
}

/** Scan every registered project root for a `<cwd>/.mcp.json`. */
export async function discoverProjectConfigs(projectRoots: Iterable<string>): Promise<ExternalMcpServerConfig[]> {
  const roots = [...projectRoots]
  const nested = await Promise.all(roots.map(async (root) => {
    const mcpJson = path.join(root, '.mcp.json')
    let raw: string
    try { raw = await fs.promises.readFile(mcpJson, 'utf-8') }
    catch { return [] }
    let parsed: any
    try { parsed = JSON.parse(raw) } catch { return [] }
    const configs = parseMcpJson(parsed, mcpJson)
    for (const config of configs) {
      config.sourceType = 'project'
      config.description = `From ${path.basename(root)}/.mcp.json`
    }
    return configs
  }))
  return nested.flat()
}

/** Discover MCP servers from ~/.claude.json mcpServers section (async). */
export async function discoverFromClaudeJsonAsync(claudeDir: string): Promise<ExternalMcpServerConfig[]> {
  const claudeJsonPath = path.join(claudeDir, '.claude.json')
  const homeClaudeJson = path.join(os.homedir(), '.claude.json')

  const collected: ExternalMcpServerConfig[] = []
  const reads = await Promise.all([claudeJsonPath, homeClaudeJson].map(async (jsonPath) => {
    try { return { path: jsonPath, raw: await fs.promises.readFile(jsonPath, 'utf-8') } }
    catch { return null }
  }))

  for (const r of reads) {
    if (!r) continue
    let parsed: any
    try { parsed = JSON.parse(r.raw) } catch { continue }
    const mcpServers = parsed.mcpServers
    if (!mcpServers || typeof mcpServers !== 'object') continue

    for (const [name, entry] of Object.entries(mcpServers)) {
      if (typeof entry !== 'object' || entry === null) continue
      const e = entry as Record<string, any>
      const serverType = e.type === 'sse' ? 'sse' : e.type === 'http' ? 'http' : e.type === 'ws' ? 'ws' : 'stdio'
      const config: ExternalMcpServerConfig = {
        id: randomUUID(),
        name,
        type: serverType,
        enabled: true,
        autoDiscovered: true,
        sourcePath: r.path,
        sourceType: 'claude-config',
        description: `From ${path.basename(r.path)}`,
      }
      if (serverType === 'http' || serverType === 'sse' || serverType === 'ws') {
        config.url = e.url
        if (e.headers && typeof e.headers === 'object') {
          config.headers = {}
          for (const [k, v] of Object.entries(e.headers)) {
            config.headers[k] = resolveEnvVars(String(v))
          }
        }
        // Carry OAuth config across restarts. Without this, a server we
        // already finished DCR for would get re-discovered without its
        // clientId/secret, the first connect would see no oauth config,
        // fall through to "OAuth required" even though tokens exist.
        if (e.oauth && typeof e.oauth === 'object') {
          const o = e.oauth as Record<string, unknown>
          if (typeof o.authServerUrl === 'string' && typeof o.clientId === 'string') {
            config.oauth = {
              authServerUrl: o.authServerUrl,
              clientId: o.clientId,
              ...(typeof o.clientSecret === 'string' ? { clientSecret: o.clientSecret } : {}),
              ...(typeof o.scope === 'string' ? { scope: o.scope } : {}),
              ...(typeof o.callbackPort === 'number' ? { callbackPort: o.callbackPort } : {}),
            }
          }
        }
      } else {
        config.command = e.command
        config.args = e.args
        if (e.env && typeof e.env === 'object') {
          config.env = e.env
        }
      }
      collected.push(config)
    }
  }
  return collected
}

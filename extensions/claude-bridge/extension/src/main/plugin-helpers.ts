// Shared helpers for plugin-sourced listings (skills, agents, commands, etc.).
//
// These mirror how the Claude CLI reads the same files, so our IPC handlers
// expose the identical feature surface to the renderer. Keeping the logic in
// one place avoids drift between `skills:list`, `agents:list` and any future
// `output-styles:list` handler.

import fs from 'fs'
import path from 'path'
import os from 'os'
import YAML from 'yaml'

/** Load `~/.claude/settings.json:enabledPlugins` → Map<"name@marketplace", bool>.
 *  Missing entry defaults to enabled. Explicit `false` means user has
 *  toggled the plugin off — its commands/agents/skills must not show up. */
export async function loadEnabledPluginsMap(): Promise<Map<string, boolean>> {
  const out = new Map<string, boolean>()
  const settingsPath = path.join(os.homedir(), '.claude', 'settings.json')
  let raw: string
  try { raw = await fs.promises.readFile(settingsPath, 'utf-8') } catch { return out }
  try {
    const data = JSON.parse(raw)
    const ep = data?.enabledPlugins
    if (ep && typeof ep === 'object') {
      for (const [k, v] of Object.entries(ep)) {
        if (typeof v === 'boolean') out.set(k, v)
      }
    }
  } catch { /* malformed */ }
  return out
}

// ─── YAML frontmatter parsing ────────────────────────────────────────────
//
// Backed by the `yaml` package (full YAML 1.2 spec — block scalars `|`/`>`,
// flow `[a, b]`, anchors, multi-line strings, escapes). The previous
// hand-rolled regex implementation broke on:
//   - block scalars in nested keys
//   - flow-style maps (`{a: b}`)
//   - anchors / aliases (`*ref`)
//   - escaped unicode (`\u00xx`)
//   - integers ≤ 0 / negative numbers / floats
//
// The cache is a small bounded LRU keyed by the raw frontmatter text.
// Skills/agents callsites typically read the same frontmatter ~10× (one
// `match()` per field) — parsing once and looking up keys is much cheaper
// than re-parsing per call. Cap is intentionally generous (200) because
// the same plugin's frontmatter is identical across all its files.

const FM_CACHE_CAP = 200
const fmCache = new Map<string, Record<string, unknown>>()

function parseFmObject(fm: string): Record<string, unknown> {
  const cached = fmCache.get(fm)
  if (cached) {
    // LRU touch: re-insert to move to the end.
    fmCache.delete(fm)
    fmCache.set(fm, cached)
    return cached
  }
  let parsed: unknown
  try { parsed = YAML.parse(fm) } catch { parsed = null }
  const obj: Record<string, unknown> =
    parsed && typeof parsed === 'object' && !Array.isArray(parsed)
      ? (parsed as Record<string, unknown>)
      : {}
  if (fmCache.size >= FM_CACHE_CAP) {
    const oldest = fmCache.keys().next().value
    if (oldest !== undefined) fmCache.delete(oldest)
  }
  fmCache.set(fm, obj)
  return obj
}

/** Parse the entire YAML frontmatter block as an object. Returns an empty
 *  object on parse failure so call sites can use safe property access
 *  without null checks. New callsites should prefer this over the legacy
 *  per-field `matchYamlField`/`decodeYamlScalar`/`parseYamlList` helpers. */
export function parseFrontmatter(fm: string): Record<string, unknown> {
  return parseFmObject(fm)
}

/** Decode a YAML scalar value. Accepts either a raw YAML fragment (e.g.
 *  `"foo bar"` or `'don''t'`) or — for back-compat — a value already
 *  extracted by an earlier `matchYamlField` call. Returns the decoded
 *  string. Empty/whitespace input returns ''. */
export function decodeYamlScalar(raw: string): string {
  if (raw == null) return ''
  const v = String(raw)
  if (v.trim() === '') return ''
  try {
    const parsed = YAML.parse(v)
    if (parsed == null) return ''
    if (typeof parsed === 'string') return parsed
    if (typeof parsed === 'number' || typeof parsed === 'boolean') return String(parsed)
    // Fragment was already a scalar string from matchYamlField — fall through
    // and just return the trimmed input.
  } catch { /* fall through */ }
  return v.trim().replace(/^["']|["']$/g, '')
}

/** Parse a list value. Accepts:
 *   - flow array: `[a, b, "c d"]`
 *   - comma-/whitespace-separated: `a, b, c`
 *   - YAML block sequence (when called with a multi-line string)
 *  Returns string[] with empty entries filtered out. */
export function parseYamlList(raw: string): string[] {
  if (raw == null) return []
  const v = String(raw).trim()
  if (!v) return []
  // Try parsing as YAML first — handles `[a, b]`, block sequences, and
  // mixed quoted/unquoted entries with proper escape semantics.
  try {
    const parsed = YAML.parse(v)
    if (Array.isArray(parsed)) {
      return parsed
        .map(item => (item == null ? '' : String(item)))
        .map(s => s.trim())
        .filter(Boolean)
    }
    if (typeof parsed === 'string') {
      return parsed.split(/[,\s]+/).map(s => s.trim()).filter(Boolean)
    }
  } catch { /* fall through to comma split */ }
  return v.split(/[,\s]+/).map(s => s.trim().replace(/^["']|["']$/g, '')).filter(Boolean)
}

/** Boolean-ish: accepts `true`/`false`, `yes`/`no`, `on`/`off`, `1`/`0`.
 *  Returns null on anything else (CLI also accepts `y`/`n`). */
export function parseYamlBool(raw: string): boolean | null {
  if (raw == null) return null
  const v = String(raw).trim().toLowerCase().replace(/^["']|["']$/g, '')
  if (!v) return null
  try {
    const parsed = YAML.parse(v)
    if (typeof parsed === 'boolean') return parsed
  } catch { /* fall through */ }
  if (v === 'true' || v === 'yes' || v === 'y' || v === 'on' || v === '1') return true
  if (v === 'false' || v === 'no' || v === 'n' || v === 'off' || v === '0') return false
  return null
}

/** Parse a positive integer; returns null on failure. Negative or zero
 *  values return null too (CLI uses this for `effort`/`maxTurns` which are
 *  always ≥ 1 in practice). */
export function parseYamlInt(raw: string): number | null {
  if (raw == null) return null
  const v = String(raw).trim().replace(/^["']|["']$/g, '')
  if (!v) return null
  try {
    const parsed = YAML.parse(v)
    if (typeof parsed === 'number' && Number.isInteger(parsed) && parsed > 0) return parsed
  } catch { /* fall through */ }
  const n = Number(v)
  return Number.isInteger(n) && n > 0 ? n : null
}

/** Extract the YAML frontmatter block from a markdown file. Returns the raw
 *  block text (between the `---` markers, without markers) or null. */
export function extractFrontmatter(content: string): string | null {
  const m = content.match(/^---\s*\n([\s\S]*?)\n---/)
  return m?.[1] ?? null
}

/** First non-frontmatter line of the document, stripped of `#` heading marks
 *  and truncated at 100 chars with `...` (matches CLI's
 *  `extractDescriptionFromMarkdown` in utils/markdownConfigLoader.ts). */
export function firstBodyLine(content: string, fallback = ''): string {
  const body = content.replace(/^---[\s\S]*?---\s*/, '').trim()
  const first = body.split('\n')[0]?.replace(/^#+\s*/, '').trim() || fallback
  return first.length > 100 ? first.slice(0, 97) + '...' : first
}

/** Look up a YAML frontmatter field. Returns the decoded string value, or
 *  undefined if the key isn't present. Handles full YAML 1.2 (block scalars
 *  `|`/`>`, flow maps, anchors, escapes) via the `yaml` package — the
 *  previous regex-based implementation missed multi-line block scalars when
 *  authors put `description: >` followed by indented content. */
export function matchYamlField(fm: string, key: string): string | undefined {
  if (!fm) return undefined
  const obj = parseFmObject(fm)
  if (!(key in obj)) return undefined
  const v = obj[key]
  if (v == null) return undefined
  if (typeof v === 'string') return v
  if (typeof v === 'number' || typeof v === 'boolean') return String(v)
  if (Array.isArray(v)) {
    return v
      .map(item => (item == null ? '' : typeof item === 'string' ? item : String(item)))
      .filter(s => s !== '')
      .join(', ')
  }
  // Object → JSON for legacy callers that only stringify it.
  try { return JSON.stringify(v) } catch { return undefined }
}

/** Read `<pluginRoot>/.claude-plugin/plugin.json` if it exists. Returns the
 *  parsed manifest object or null on missing / malformed JSON. */
export async function readPluginManifest(pluginRoot: string): Promise<any | null> {
  const manifestPath = path.join(pluginRoot, '.claude-plugin', 'plugin.json')
  let raw: string
  try { raw = await fs.promises.readFile(manifestPath, 'utf-8') } catch { return null }
  try { return JSON.parse(raw) } catch { return null }
}

/** Read the plugin's entry from its configured marketplace catalog. Claude
 * Code allows manifest component fields on this entry as well as in
 * `.claude-plugin/plugin.json`, so harness discovery must not treat the local
 * manifest as the only source of truth. */
export async function readMarketplacePluginEntry(pluginName: string, marketplace: string): Promise<any | null> {
  if (!pluginName || !marketplace || marketplace === 'local') return null
  try {
    const known = JSON.parse(await fs.promises.readFile(
      path.join(os.homedir(), '.claude', 'plugins', 'known_marketplaces.json'),
      'utf-8',
    ))
    const root = known?.[marketplace]?.installLocation
    if (typeof root !== 'string') return null
    const catalog = JSON.parse(await fs.promises.readFile(path.join(root, '.claude-plugin', 'marketplace.json'), 'utf-8'))
    const entry = Array.isArray(catalog?.plugins)
      ? catalog.plugins.find((candidate: any) => candidate?.name === pluginName)
      : null
    return entry && typeof entry === 'object' && !Array.isArray(entry) ? entry : null
  } catch {
    return null
  }
}

function mergeDeclaration(entryValue: unknown, manifestValue: unknown): unknown {
  if (entryValue === undefined) return manifestValue
  if (manifestValue === undefined) return entryValue
  if (entryValue && manifestValue && typeof entryValue === 'object' && typeof manifestValue === 'object'
      && !Array.isArray(entryValue) && !Array.isArray(manifestValue)) {
    return { ...(entryValue as Record<string, unknown>), ...(manifestValue as Record<string, unknown>) }
  }
  const flatten = (value: unknown): unknown[] => Array.isArray(value) ? value : [value]
  return [...flatten(entryValue), ...flatten(manifestValue)]
}

function mergeIndependentDeclaration(entryValue: unknown, manifestValue: unknown): unknown {
  if (entryValue === undefined) return manifestValue
  if (manifestValue === undefined) return entryValue
  const flatten = (value: unknown): unknown[] => Array.isArray(value) ? value : [value]
  return [...flatten(entryValue), ...flatten(manifestValue)]
}

/** Pure form of the marketplace + local manifest merge contract. Exported so
 * discovery tests can exercise it without mutating the user's Claude config. */
export function mergePluginDeclarations(
  pluginRoot: string,
  pluginName: string,
  entry: Record<string, any> | null,
  manifest: Record<string, any> | null,
): Record<string, any> {
  const effective: Record<string, any> = { ...(entry ?? {}), ...(manifest ?? {}) }

  // These declarations are independent sources. In particular, two inline
  // hook objects must both survive even when they contain the same event key;
  // collectPluginHooks performs the event-level append later.
  for (const key of ['hooks', 'dependencies']) {
    const merged = mergeIndependentDeclaration(entry?.[key], manifest?.[key])
    if (merged !== undefined) effective[key] = merged
  }
  // Named server maps merge by key; mixed path/map declarations concatenate.
  for (const key of ['mcpServers', 'lspServers']) {
    const merged = mergeDeclaration(entry?.[key], manifest?.[key])
    if (merged !== undefined) effective[key] = merged
  }
  // These are keyed schemas/namespaces rather than replacement path fields.
  for (const key of ['userConfig', 'experimental']) {
    const merged = mergeDeclaration(entry?.[key], manifest?.[key])
    if (merged !== undefined) effective[key] = merged
  }

  if (typeof entry?.defaultEnabled === 'boolean') effective.defaultEnabled = entry.defaultEnabled
  // The marketplace entry name is the installed identity and component
  // namespace even if a bundled manifest happens to use a different name.
  effective.name = pluginName || manifest?.name || entry?.name || path.basename(pluginRoot)
  return effective
}

/** Effective plugin declaration. Plugin manifest metadata/path fields override
 * marketplace values, while declarations with independent merge semantics are
 * combined. Marketplace `defaultEnabled` is the documented exception and wins. */
export async function readEffectivePluginManifest(
  pluginRoot: string,
  pluginName: string,
  marketplace: string,
): Promise<Record<string, any>> {
  const [entry, manifest] = await Promise.all([
    readMarketplacePluginEntry(pluginName, marketplace),
    readPluginManifest(pluginRoot),
  ])
  return mergePluginDeclarations(pluginRoot, pluginName, entry, manifest)
}

/** Synchronous counterpart for subprocess hot paths that already expose a
 * synchronous API (hook/MCP command expansion). */
export function readEffectivePluginManifestSync(
  pluginRoot: string,
  pluginName: string,
  marketplace: string,
): Record<string, any> {
  let manifest: Record<string, any> | null = null
  let entry: Record<string, any> | null = null
  try {
    manifest = JSON.parse(fs.readFileSync(path.join(pluginRoot, '.claude-plugin', 'plugin.json'), 'utf-8'))
  } catch { /* optional local manifest */ }
  if (marketplace && marketplace !== 'local') {
    try {
      const known = JSON.parse(fs.readFileSync(
        path.join(os.homedir(), '.claude', 'plugins', 'known_marketplaces.json'),
        'utf-8',
      ))
      const root = known?.[marketplace]?.installLocation
      if (typeof root === 'string') {
        const catalog = JSON.parse(fs.readFileSync(path.join(root, '.claude-plugin', 'marketplace.json'), 'utf-8'))
        const match = Array.isArray(catalog?.plugins)
          ? catalog.plugins.find((candidate: any) => candidate?.name === pluginName)
          : null
        if (match && typeof match === 'object' && !Array.isArray(match)) entry = match
      }
    } catch { /* optional marketplace declaration */ }
  }
  return mergePluginDeclarations(pluginRoot, pluginName, entry, manifest)
}

/** A plugin entry from `~/.claude/plugins/installed_plugins.json` that is
 *  both installed (cache directory present) and enabled by the user's
 *  `~/.claude/settings.json:enabledPlugins` toggle. */
export interface InstalledPlugin {
  /** Plugin name as used in marketplace (no `@marketplace` suffix). */
  name: string
  /** Marketplace name (the suffix after `@` in the installed_plugins key). */
  marketplace: string
  /** Composite key `"name@marketplace"` — the form used in enabledPlugins. */
  key: string
  /** Absolute path to the plugin's cache directory (contains `.claude-plugin/`,
   *  optional `.mcp.json`, `.lsp.json`, etc.). */
  installPath: string
}

function pluginNameFromId(pluginId: string): string {
  const at = pluginId.lastIndexOf('@')
  return at > 0 ? pluginId.slice(0, at) : pluginId
}

/** Claude's runtime component/MCP namespace is the plugin name, without its
 * marketplace suffix. Refuse a proposed enabled set that would therefore be
 * ambiguous instead of silently routing tools to whichever entry was read
 * first. */
export function assertUniqueEnabledPluginNames(pluginIds: Iterable<string>): void {
  const byName = new Map<string, string[]>()
  for (const pluginId of new Set(pluginIds)) {
    const name = pluginNameFromId(pluginId)
    const bucket = byName.get(name) ?? []
    bucket.push(pluginId)
    byName.set(name, bucket)
  }
  for (const [name, ids] of [...byName].sort(([a], [b]) => a.localeCompare(b))) {
    ids.sort((a, b) => a.localeCompare(b))
    if (ids.length > 1) {
      throw new Error(
        `Plugin namespace "${name}" conflict: ${ids[0]} and ${ids[1]} cannot be enabled together. `
        + 'Disable one of them first.',
      )
    }
  }
}

/** Legacy settings may already contain two enabled `name@marketplace` keys.
 * Keep the runtime safe and deterministic: an explicit `true` beats an
 * implicit default-enabled entry, then the lexicographically smaller id wins.
 * New mutations are rejected by assertUniqueEnabledPluginNames instead. */
export function selectPluginNamespaceWinners(
  plugins: readonly InstalledPlugin[],
  explicitlyEnabledKeys: ReadonlySet<string>,
): InstalledPlugin[] {
  const ranked = [...plugins].sort((a, b) => {
    const explicitDelta = Number(explicitlyEnabledKeys.has(b.key)) - Number(explicitlyEnabledKeys.has(a.key))
    return explicitDelta || a.key.localeCompare(b.key)
  })
  const claimed = new Set<string>()
  const winners: InstalledPlugin[] = []
  for (const plugin of ranked) {
    if (claimed.has(plugin.name)) continue
    claimed.add(plugin.name)
    winners.push(plugin)
  }
  return winners.sort((a, b) => a.key.localeCompare(b.key))
}

/** List plugins that are BOTH installed (present in `installed_plugins.json`
 *  v2 with a valid `installPath` on disk) AND enabled (not explicitly set to
 *  `false` in `~/.claude/settings.json:enabledPlugins`).
 *
 *  Single source of truth for every plugin-sourced feature: MCP servers, LSP
 *  servers, skills, agents, commands. Do NOT scan `marketplaces/` subfolders
 *  yourself — different CLI versions lay them out differently (cache/ vs
 *  marketplaces/external_plugins/ vs marketplaces/plugins/mcp/), and this
 *  helper normalises over all of them by trusting `installPath`. */
export async function listEnabledInstalledPlugins(): Promise<InstalledPlugin[]> {
  const [installedMap, enabled] = await Promise.all([
    loadInstalledPluginsMap(),
    loadEnabledPluginsMap(),
  ])

  const out: InstalledPlugin[] = []
  for (const [key, entry] of installedMap) {
    if (enabled.get(key) === false) continue
    if (!entry.installPath) continue
    let exists = false
    try { exists = fs.existsSync(entry.installPath) } catch { /* EACCES etc. */ }
    if (!exists) continue

    const atIdx = key.lastIndexOf('@')
    const name = atIdx > 0 ? key.slice(0, atIdx) : key
    const marketplace = atIdx > 0 ? key.slice(atIdx + 1) : ''
    // An explicit settings value wins. Otherwise `defaultEnabled: false`
    // from the marketplace entry (which itself wins over plugin.json) keeps
    // externally-installed opt-in plugins inert until the user enables them.
    if (!enabled.has(key)) {
      const manifest = await readEffectivePluginManifest(entry.installPath, name, marketplace)
      if (manifest.defaultEnabled === false) continue
    }
    out.push({ name, marketplace, key, installPath: entry.installPath })
  }
  const explicitlyEnabled = new Set(
    [...enabled].filter(([, value]) => value === true).map(([key]) => key),
  )
  const winners = selectPluginNamespaceWinners(out, explicitlyEnabled)
  if (winners.length !== out.length) {
    const selected = new Set(winners.map(plugin => plugin.key))
    const ignored = out.filter(plugin => !selected.has(plugin.key)).map(plugin => plugin.key).sort()
    console.warn(`[plugins] Ignoring duplicate enabled plugin namespace(s): ${ignored.join(', ')}`)
  }
  return winners
}

/** Existing `bin/` directories from every enabled plugin, in deterministic
 * order. Claude Code prepends these to Bash PATH; the bridge shell runs on the
 * client host, so it must reproduce that part of the plugin harness there. */
export async function listEnabledPluginBinDirs(): Promise<string[]> {
  const plugins = await listEnabledInstalledPlugins()
  return plugins
    .map(plugin => path.join(plugin.installPath, 'bin'))
    .filter(binDir => {
      try { return fs.statSync(binDir).isDirectory() } catch { return false }
    })
    .sort()
}

/** Load saved user-configurable option values for a plugin — merges
 *  non-sensitive (from `~/.claude/settings.json:pluginConfigs.<id>.options`)
 *  with sensitive (from `~/.claude/.credentials.json:pluginSecrets.<id>`).
 *  `pluginId` is canonical `"<name>@<marketplace>"` — same key the Claude
 *  Code CLI uses. Returns empty object if nothing saved or files missing.
 *
 *  Matches CLI's `loadPluginOptions` behaviour: secure wins on key collision
 *  (the sensitive store overrides any shadow copy in plain settings), and
 *  the merged dictionary is ready for `${user_config.KEY}` substitution. */
export function loadPluginOptions(pluginId: string): Record<string, unknown> {
  const out: Record<string, unknown> = {}

  // Schema defaults participate even before the user opens Configure.
  try {
    const installedPath = path.join(os.homedir(), '.claude', 'plugins', 'installed_plugins.json')
    const installed = JSON.parse(fs.readFileSync(installedPath, 'utf-8'))
    const root = installed?.plugins?.[pluginId]?.[0]?.installPath
    if (typeof root === 'string') {
      const at = pluginId.lastIndexOf('@')
      const pluginName = at > 0 ? pluginId.slice(0, at) : pluginId
      const marketplace = at > 0 ? pluginId.slice(at + 1) : ''
      const schema = readEffectivePluginManifestSync(root, pluginName, marketplace).userConfig
      if (schema && typeof schema === 'object' && !Array.isArray(schema)) {
        for (const [key, spec] of Object.entries(schema)) {
          if (spec && typeof spec === 'object' && 'default' in spec) out[key] = (spec as { default: unknown }).default
        }
      }
    }
  } catch { /* optional manifest/defaults */ }

  const settingsPath = path.join(os.homedir(), '.claude', 'settings.json')
  try {
    const raw = fs.readFileSync(settingsPath, 'utf-8')
    const data = JSON.parse(raw)
    const opts = data?.pluginConfigs?.[pluginId]?.options
    if (opts && typeof opts === 'object' && !Array.isArray(opts)) {
      for (const [k, v] of Object.entries(opts)) out[k] = v
    }
  } catch { /* missing or malformed */ }

  const credsPath = path.join(os.homedir(), '.claude', '.credentials.json')
  try {
    const raw = fs.readFileSync(credsPath, 'utf-8')
    const data = JSON.parse(raw)
    const opts = data?.pluginSecrets?.[pluginId]
    if (opts && typeof opts === 'object' && !Array.isArray(opts)) {
      for (const [k, v] of Object.entries(opts)) out[k] = v
    }
  } catch { /* missing or malformed */ }

  return out
}

/** Substitute `${user_config.KEY}` tokens in a string with values from the
 *  plugin's saved options. Missing keys stay literal — matches CLI's
 *  content-safe variant (`substituteUserConfigInContent`), rather than the
 *  throw-on-missing variant used at MCP config parse. Plugin authors whose
 *  `${user_config.X}` lingered as literal in the final command get a clear
 *  signal something went wrong. */
export function substituteUserConfig(value: string, options: Record<string, unknown>): string {
  return value.replace(/\$\{user_config\.([^}]+)\}/g, (match, key) => {
    if (key in options) return String(options[key])
    return match
  })
}

/** Raw map loader — exported for callers that need the unfiltered set
 *  (e.g. `plugins:list-installed`, which shows disabled plugins too). */
export async function loadInstalledPluginsMap(): Promise<Map<string, { installPath: string }>> {
  const out = new Map<string, { installPath: string }>()
  const filePath = path.join(os.homedir(), '.claude', 'plugins', 'installed_plugins.json')
  let raw: string
  try { raw = await fs.promises.readFile(filePath, 'utf-8') } catch { return out }
  try {
    const data = JSON.parse(raw)
    if (data?.version !== 2 || !data?.plugins) return out
    for (const [key, entries] of Object.entries(data.plugins)) {
      const arr = entries as Array<{ installPath: string }>
      if (arr.length > 0 && arr[0]?.installPath) {
        out.set(key, { installPath: arr[0].installPath })
      }
    }
  } catch { /* malformed */ }
  return out
}

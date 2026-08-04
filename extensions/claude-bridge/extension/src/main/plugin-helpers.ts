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
    out.push({ name, marketplace, key, installPath: entry.installPath })
  }
  return out
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

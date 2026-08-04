// Manifest discovery for the extension loader. Walks the builtin
// extensions directory, validates each child has a parseable
// `package.json`, and returns the (manifest, path) pairs the loader
// will activate.
//
// Concerns intentionally kept here (and out of `loader.ts`):
//   - filesystem walking + soft error logging on per-extension failures
//   - JSON shape validation that rejects type-unsafe manifests
//   - the `manifest.main` path-traversal guard — an extension that
//     ships `"main": "../../../etc/passwd"` would otherwise win
import { readFileSync, readdirSync, statSync } from "node:fs"
import { dirname, join, resolve, sep } from "node:path"
import type { ExtensionDescriptor } from "../api/types.js"

export interface DiscoveredExtension {
  manifest: Record<string, unknown>
  path: string
}

/** Build an ExtensionDescriptor from a manifest (shared by prepare + the
 *  disabled-stub list entry). The result is a plain mutable object — `prepare`
 *  flips `active`/`activationError` on it after applying contributions. */
export function buildDescriptor(
  id: string,
  manifest: Record<string, unknown>,
  extensionPath: string,
  builtin: boolean,
  active: boolean,
  enabled: boolean,
): ExtensionDescriptor {
  return {
    id,
    displayName: coerceManifestString(manifest.displayName) ?? coerceManifestString(manifest.name) ?? id,
    version: coerceManifestString(manifest.version) ?? "0.0.0",
    active,
    builtin,
    enabled,
    packageJSON: manifest,
    extensionPath,
  }
}

/** Normalised activation events. An ABSENT `activationEvents` field is the legacy
 *  case → `*` (eager). An EXPLICIT array (even empty) is the modern auto-infer
 *  case → declared events + synthesized `onCommand:<id>` for contributed
 *  commands, and NOT eager. */
export function normalizeActivationEvents(manifest: Record<string, unknown>): string[] {
  if (!Array.isArray(manifest.activationEvents)) return ["*"]
  const declared = parseActivationEvents(manifest)
  const synthetic = contributedCommandIds(manifest).map((id) => `onCommand:${id}`)
  return [...new Set([...declared, ...synthetic])]
}

export function discoverExtensions(builtinDir: string): DiscoveredExtension[] {
  let entries: string[]
  try { entries = readdirSync(builtinDir) } catch (err) {
    console.warn(`KaminIDE: cannot read builtin extension dir ${builtinDir}:`, err)
    return []
  }
  const out: DiscoveredExtension[] = []
  for (const name of entries) {
    const extPath = join(builtinDir, name)
    let s
    try { s = statSync(extPath) } catch (err) {
      console.warn(`KaminIDE: skipping ${name} — stat failed:`, (err as Error).message)
      continue
    }
    if (!s.isDirectory()) continue
    const manifestPath = join(extPath, "package.json")
    let raw: string
    try { raw = readFileSync(manifestPath, "utf8") } catch {
      console.warn(`KaminIDE: skipping ${name} — no package.json`)
      continue
    }
    let manifest: Record<string, unknown>
    try { manifest = JSON.parse(raw) as Record<string, unknown> } catch (e) {
      console.warn(`KaminIDE: skipping ${name} — invalid package.json: ${(e as Error).message}`)
      continue
    }
    out.push({ manifest, path: extPath })
  }
  return out
}

export function coerceManifestString(v: unknown): string | undefined {
  return typeof v === "string" && v.length > 0 ? v : undefined
}

/** `publisher.name` id from a manifest — the loader's stable extension key. */
export function manifestId(manifest: Record<string, unknown>): string {
  return `${coerceManifestString(manifest.publisher) ?? "unknown"}.${coerceManifestString(manifest.name) ?? "unnamed"}`
}

/** The manifest's `activationEvents`, filtered to strings (B3). Empty when
 *  absent — the loader maps that to the legacy `*` (activate at startup). */
export function parseActivationEvents(manifest: Record<string, unknown>): string[] {
  const raw = manifest.activationEvents
  if (!Array.isArray(raw)) return []
  return raw.filter((e): e is string => typeof e === "string")
}

/** `manifest.extensionDependencies` (B-perf) — ids activated before this
 *  extension so its `activate()` can read their exports. */
export function parseExtensionDependencies(manifest: Record<string, unknown>): string[] {
  const raw = manifest.extensionDependencies
  if (!Array.isArray(raw)) return []
  return raw.filter((d): d is string => typeof d === "string")
}

/** The ids of `contributes.commands` (B3) — expanded into synthetic
 *  `onCommand:<id>` activation events so invoking a contributed command
 *  lazily activates its extension, as VS Code does implicitly. */
export function contributedCommandIds(manifest: Record<string, unknown>): string[] {
  const contributes = manifest.contributes as Record<string, unknown> | undefined
  const cmds = contributes?.commands
  if (!Array.isArray(cmds)) return []
  const ids: string[] = []
  for (const c of cmds) {
    const id = (c as { command?: unknown } | null)?.command
    if (typeof id === "string") ids.push(id)
  }
  return ids
}

/** VS Code synthesises a type-based default for a contributed config property
 *  that declares no `default` (string→"", boolean→false, number/integer→0,
 *  array→[], object→{}, null→null). Some extensions (Vue.volar) read such a key
 *  and assert the value is not `undefined`, so we mirror it. */
function implicitConfigDefault(schema: unknown): unknown {
  if (!schema || typeof schema !== "object") return undefined
  const t = (schema as { type?: unknown }).type
  switch (Array.isArray(t) ? t[0] : t) {
    case "string": return ""
    case "boolean": return false
    case "number": case "integer": return 0
    case "array": return []
    case "object": return {}
    case "null": return null
    default: return undefined
  }
}

/** Flatten `contributes.configuration` — a single block or an array of
 *  blocks, each `{ title?, properties: { [dottedKey]: { default?, type? } } }` —
 *  plus the separate `contributes.configurationDefaults` map, into a flat
 *  default-value map (B2). EVERY contributed key is registered (VS Code's config
 *  registry knows them all); keys with no `default` get the type-based default.
 *  `configurationDefaults` keys may be plain settings or language selectors
 *  (`"[markdown]"`) — stored as-is; language selectors take effect once
 *  language-scoped config lands. */
export function parseConfigDefaults(manifest: Record<string, unknown>): Record<string, unknown> {
  const contributes = manifest.contributes as Record<string, unknown> | undefined
  if (!contributes) return {}
  const defaults: Record<string, unknown> = {}
  const cfg = contributes.configuration
  if (cfg) {
    for (const block of Array.isArray(cfg) ? cfg : [cfg]) {
      const props = (block as { properties?: Record<string, unknown> } | null)?.properties
      if (!props) continue
      for (const [key, schema] of Object.entries(props)) {
        defaults[key] = schema && typeof schema === "object" && "default" in schema
          ? schema.default
          : implicitConfigDefault(schema)
      }
    }
  }
  const overrides = contributes.configurationDefaults
  if (overrides && typeof overrides === "object") {
    Object.assign(defaults, overrides as Record<string, unknown>)
  }
  return defaults
}

/** Walk up from a requiring file to its owning extension id
 *  (`publisher.name`), reading `package.json` at each level; cached per file
 *  path. Sync FS is unavoidable — this runs inside the synchronous CJS
 *  `require('vscode')` hook; the walk is bounded by extension depth and the
 *  cache means each source file walks at most once. */
export function findOwningExtension(filePath: string, cache: Map<string, string>): string | undefined {
  if (!filePath) return undefined
  const cached = cache.get(filePath)
  if (cached) return cached
  let dir = dirname(filePath)
  const root = resolve("/")
  while (dir && dir !== root) {
    try {
      const pkg = JSON.parse(readFileSync(join(dir, "package.json"), "utf8")) as { name?: string; publisher?: string }
      if (pkg.name) {
        const owner = `${pkg.publisher ?? "unknown"}.${pkg.name}`
        cache.set(filePath, owner)
        return owner
      }
    } catch { /* keep walking */ }
    const next = dirname(dir)
    if (next === dir) break
    dir = next
  }
  return undefined
}

/** Resolve `manifest.main` inside the extension directory and refuse
 *  paths that escape it. Without this, a malicious manifest declaring
 *  `"main": "../../../../tmp/payload"` would let the require hook
 *  load arbitrary files on disk during activation. Returns the
 *  absolute path on success, `undefined` if the path escaped. */
export function resolveExtensionMain(extPath: string, mainRel: string): string | undefined {
  const absExtPath = resolve(extPath)
  const candidate = resolve(absExtPath, mainRel)
  // The candidate must live inside extPath. `startsWith(extPath + sep)`
  // is the standard "is descendant" check that avoids the prefix-match
  // hole where `/foo/bar2` would falsely match `/foo/bar`.
  if (candidate !== absExtPath && !candidate.startsWith(absExtPath + sep)) {
    return undefined
  }
  return candidate
}

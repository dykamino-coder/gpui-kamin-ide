// Configuration layers backing `vscode.workspace.getConfiguration` (B2).
// Merge order: contributed default ← global (user) ← workspace. Defaults
// come from extensions' `contributes.configuration`; the global layer
// persists in `config.json`; the workspace layer is the open folder's
// `.vscode/settings.json` (VS Code's own location, so the file round-trips
// with a real editor). Flat dotted keys throughout — the vscode-api layer
// (ns-config.ts) handles section prefixing and sub-tree reconstruction.
import { existsSync, readFileSync } from "node:fs"
import { mkdir, readFile, rename, writeFile } from "node:fs/promises"
import { dirname, join } from "node:path"
import { applyEdits, modify, parse as parseJsonc, type ParseError } from "jsonc-parser"
import { JsonStore } from "../json-store.js"
import { getWorkspaceFolder, onWorkspaceChange } from "./workspace.js"

interface GlobalConfig { values: Record<string, unknown> }

const defaults = new Map<string, unknown>()
let globalStore: JsonStore<GlobalConfig> | null = null
let wsCache: Record<string, unknown> | null = null
let wsCacheFolder: string | null = null
const changeListeners = new Set<(keys: readonly string[]) => void>()

/** Called once at host boot. Backs the global layer with `config.json` and
 *  invalidates the workspace cache whenever the open folder changes.
 *  `setWorkspaceFolder` updates its `current` BEFORE firing listeners, so the
 *  `getAllConfig()` below already reads the new folder's settings. */
export function initConfig(dataDir: string): void {
  globalStore = new JsonStore<GlobalConfig>(dataDir, "config", { values: {} })
  onWorkspaceChange(() => {
    wsCache = null
    wsCacheFolder = null
    fire(Object.keys(getAllConfig()))
  })
}

/** Merge contributed defaults (flat dotted key → default value). Last wins,
 *  which matches VS Code's "later contribution overrides" for duplicates. */
export function registerConfigDefaults(values: Record<string, unknown>): void {
  for (const [k, v] of Object.entries(values)) defaults.set(k, v)
}

function settingsPath(folder: string): string {
  return join(folder, ".vscode", "settings.json")
}

/** Read + cache the open folder's `.vscode/settings.json`. The file is JSONC
 *  (comments + trailing commas) — VS Code's own format — so we parse it
 *  tolerantly; recoverable syntax issues are logged, not fatal. */
function readWorkspace(): Record<string, unknown> {
  const folder = getWorkspaceFolder().path
  if (!folder) return {}
  if (wsCache && wsCacheFolder === folder) return wsCache
  let parsed: Record<string, unknown> = {}
  const p = settingsPath(folder)
  if (existsSync(p)) {
    const errors: ParseError[] = []
    const value: unknown = parseJsonc(readFileSync(p, "utf8"), errors, { allowTrailingComma: true })
    if (errors.length > 0) console.warn(`config: ${p} has ${errors.length} JSONC issue(s); using best-effort parse`)
    if (value && typeof value === "object") parsed = value as Record<string, unknown>
  }
  wsCache = parsed
  wsCacheFolder = folder
  return parsed
}

function globalValues(): Record<string, unknown> {
  return globalStore?.get().values ?? {}
}

/** Full merged snapshot — flat dotted keys, workspace winning over global
 *  winning over default. */
export function getAllConfig(): Record<string, unknown> {
  const out: Record<string, unknown> = {}
  for (const [k, v] of defaults) out[k] = v
  Object.assign(out, globalValues(), readWorkspace())
  return out
}

export interface ConfigInspect {
  defaultValue?: unknown
  globalValue?: unknown
  workspaceValue?: unknown
}

/** Per-layer values for one dotted key (drives `WorkspaceConfiguration.inspect`). */
export function inspectConfig(key: string): ConfigInspect {
  const g = globalValues()
  const w = readWorkspace()
  return {
    defaultValue: defaults.has(key) ? defaults.get(key) : undefined,
    globalValue: key in g ? g[key] : undefined,
    workspaceValue: key in w ? w[key] : undefined,
  }
}

/** Apply one key write to the in-memory global layer, returning a fresh copy.
 *  An `undefined` value omits the key (VS Code's "reset" semantics) — done by
 *  destructuring rather than `delete` to keep the layer immutable. */
function writeGlobalLayer(layer: Record<string, unknown>, key: string, value: unknown): Record<string, unknown> {
  if (value === undefined) {
    const { [key]: _omit, ...rest } = layer
    return rest
  }
  return { ...layer, [key]: value }
}

// Serialize workspace-setting writes: each is a read-modify-write on the SAME
// `.vscode/settings.json`, so two concurrent config.update() calls (two
// extensions, or an extension racing a UI change) would both read the same
// original and the second rename would silently drop the first's key. Chain
// every write onto the previous so they apply one-at-a-time.
let wsWriteQueue: Promise<unknown> = Promise.resolve()
let wsTmpSeq = 0

async function doWriteWorkspaceSetting(folder: string, key: string, value: unknown): Promise<Record<string, unknown>> {
  const p = settingsPath(folder)
  // Settings use flat dotted keys (`"editor.fontSize"`), not nested objects,
  // so the json-path is a single segment — the literal dotted key.
  const original = existsSync(p) ? await readFile(p, "utf8") : "{}"
  const edits = modify(original, [key], value, { formattingOptions: { insertSpaces: true, tabSize: 2 } })
  const updated = applyEdits(original, edits)
  await mkdir(dirname(p), { recursive: true })
  const tmp = `${p}.${String(process.pid)}.${String(++wsTmpSeq)}.tmp` // unique — no interleave
  await writeFile(tmp, updated)
  await rename(tmp, p)
  const reparsed: unknown = parseJsonc(updated, [], { allowTrailingComma: true })
  return reparsed && typeof reparsed === "object" ? (reparsed as Record<string, unknown>) : {}
}

/** Point-write one setting into `.vscode/settings.json` via jsonc-parser's edit
 *  API (PRESERVES comments/formatting). `undefined` removes the key. Serialized
 *  + atomic. Returns the re-parsed object for cache refresh. */
function writeWorkspaceSetting(folder: string, key: string, value: unknown): Promise<Record<string, unknown>> {
  const run = wsWriteQueue.then(() => doWriteWorkspaceSetting(folder, key, value))
  wsWriteQueue = run.catch(() => { /* keep the chain alive past a failed write */ })
  return run
}

/** Write a value at a layer. target 1 = Global, ≥2 = Workspace. `undefined`
 *  resets (removes) the key from that layer. */
export async function updateConfig(key: string, value: unknown, target: number): Promise<void> {
  const WORKSPACE_TARGET_FLOOR = 2 // ConfigurationTarget.Workspace / WorkspaceFolder
  if (target >= WORKSPACE_TARGET_FLOOR) {
    const folder = getWorkspaceFolder().path
    if (!folder) throw new Error("No workspace open: cannot write workspace configuration")
    wsCache = await writeWorkspaceSetting(folder, key, value)
    wsCacheFolder = folder
  } else {
    globalStore?.set({ values: writeGlobalLayer(globalValues(), key, value) })
  }
  fire([key])
}

export function onConfigChange(fn: (keys: readonly string[]) => void): () => void {
  changeListeners.add(fn)
  return () => { changeListeners.delete(fn) }
}

function fire(keys: readonly string[]): void {
  for (const fn of changeListeners) fn(keys)
}

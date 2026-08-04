// `vscode.workspace.getConfiguration` + `onDidChangeConfiguration` (B2).
// Split from ns-workspace.ts to keep both under the 250-LOC ceiling and so
// the config logic is independently testable (ns-config.test.ts). Bridged
// to the host config layers via the injected `ConfigHost` (host-services.ts).
import { ConfigurationTarget } from "./enums.js"
import type { NsHooks } from "./ns-builders.js"
import { EventEmitter } from "./shared.js"

/** Reconstruct a nested object for keys under `prefix.` — supports
 *  `getConfiguration().get("editor")` returning `{ fontSize, … }`. Returns
 *  undefined when nothing lives under the prefix. */
function subtree(all: Record<string, unknown>, prefix: string): Record<string, unknown> | undefined {
  const dot = `${prefix}.`
  let found: Record<string, unknown> | undefined
  for (const [k, v] of Object.entries(all)) {
    if (!k.startsWith(dot)) continue
    found ??= {}
    const rest = k.slice(dot.length).split(".")
    let node = found
    for (let i = 0; i < rest.length - 1; i++) {
      const seg = rest[i] ?? ""
      const child = node[seg]
      node = typeof child === "object" && child !== null
        ? (child as Record<string, unknown>)
        : (node[seg] = {})
    }
    node[rest[rest.length - 1] ?? ""] = v
  }
  return found
}

/** VS Code accepts `ConfigurationTarget | boolean | null` for update's target:
 *  `true` = Global, `false` = Workspace, a number = that target. When omitted,
 *  default to Workspace if a folder is open, else Global. */
function normalizeTarget(target: number | boolean | null | undefined, hasFolder: boolean): number {
  if (target === true) return ConfigurationTarget.Global
  if (target === false) return ConfigurationTarget.Workspace
  if (typeof target === "number") return target
  return hasFolder ? ConfigurationTarget.Workspace : ConfigurationTarget.Global
}

// The WorkspaceConfiguration methods — anything NOT in this set is treated as
// a config key for the property-access form (`config.fontSize`), matching the
// d.ts `readonly [key: string]: any` index signature.
const CONFIG_METHODS = new Set(["get", "has", "inspect", "update"])

export function buildConfiguration(h: NsHooks) {
  const cfg = h.workspaceHost.config

  const getConfiguration = (section?: string) => {
    const prefix = section ? `${section}.` : ""
    const conf = {
      get<T>(key: string, defaultValue?: T): T | undefined {
        const all = cfg.getAll()
        const full = prefix + key
        // VS Code falls back to `defaultValue` whenever the effective value is
        // undefined — INCLUDING a key that's registered with no default (we now
        // register every contributed key). Returning the registered `undefined`
        // here broke `get(key, {})` callers (ms-azuretools.vscode-containers).
        const direct = full in all ? all[full] : undefined
        if (direct !== undefined) return direct as T
        return (subtree(all, full) ?? defaultValue) as T | undefined
      },
      has(key: string): boolean {
        const full = prefix + key
        const all = cfg.getAll()
        return full in all || subtree(all, full) !== undefined
      },
      inspect(key: string) {
        const full = prefix + key
        const r = cfg.inspect(full)
        // Language-override layers aren't implemented yet; expose the fields as
        // stubs so extensions that read them get `undefined`, not a crash.
        return {
          key: full,
          defaultValue: r.defaultValue,
          globalValue: r.globalValue,
          workspaceValue: r.workspaceValue,
          workspaceFolderValue: undefined,
          defaultLanguageValue: undefined,
          globalLanguageValue: undefined,
          workspaceLanguageValue: undefined,
          workspaceFolderLanguageValue: undefined,
          languageIds: [] as string[],
        }
      },
      // `_overrideInLanguage` is accepted to match the d.ts arity; language-
      // scoped writes are not yet routed to a distinct layer.
      update(key: string, value: unknown, target?: number | boolean | null, _overrideInLanguage?: boolean): Promise<void> {
        const hasFolder = h.workspaceHost.getFolderPath() !== null
        return cfg.update(prefix + key, value, normalizeTarget(target, hasFolder))
      },
    }
    // Proxy gives the documented property-access form: `config.fontSize` reads
    // the same value as `config.get("fontSize")`. Methods pass through.
    return new Proxy(conf, {
      get(target, prop, receiver): unknown {
        if (typeof prop === "string" && !CONFIG_METHODS.has(prop)) return target.get(prop)
        return Reflect.get(target, prop, receiver) as unknown
      },
      has(target, prop) {
        if (typeof prop === "string" && !CONFIG_METHODS.has(prop)) return target.has(prop)
        return Reflect.has(target, prop)
      },
    })
  }

  // onDidChangeConfiguration — re-fire host config changes as a vscode event
  // carrying `affectsConfiguration(section, scope?)`. `scope` is accepted for
  // arity but ignored (single-folder model; multi-root scoping comes later).
  const onChange = new EventEmitter<{ affectsConfiguration(section: string, scope?: unknown): boolean }>()
  h.workspaceHost.config.onDidChange((keys) => {
    onChange.fire({
      affectsConfiguration: (section: string, _scope?: unknown) =>
        keys.some((k) => k === section || k.startsWith(`${section}.`)),
    })
  })

  return { getConfiguration, onDidChangeConfiguration: onChange.event }
}

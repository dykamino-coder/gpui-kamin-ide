// Applying manifest `contributes` to the registry (B3/B4). Split from
// loader.ts to stay under the 250-LOC ceiling. Two phases, by timing:
//   - STATIC (menus, keybindings, submenus, view containers, views, themes)
//     — registered at load, visible without activating the extension.
//   - command titles — applied AFTER activation, once the extension has
//     registered the command handlers the titles decorate.
// STATIC registrations return their Disposables so the loader can drop them
// when the extension unloads (B4 review: otherwise they leak into the snapshot).
import { resolve, sep } from "node:path"
import type { Disposable, RegistryKeybinding, RegistryMenuItem, RegistrySubmenu, RegistryView } from "../api/types.js"
import type { Registry } from "./registry.js"
import { registerContainerRevealCommand, registerViewFocusCommand } from "./view-reveal.js"

/** STATIC contributions surfaced without activating the extension. Returns the
 *  Disposables to collect for unload. Untrusted manifest values are sanitized
 *  field-by-field (a non-string `when`/`group` is dropped, never cast through).
 *  `extPath` is the extension root, used to resolve icon-theme JSON paths. */
export function applyStaticContributions(registry: Registry, manifest: Record<string, unknown>, extPath: string): Disposable[] {
  const contributes = manifest.contributes as Record<string, unknown> | undefined
  if (!contributes) return []
  const out: Disposable[] = []
  applyMenus(registry, contributes, out)
  applyKeybindings(registry, contributes, out)
  applySubmenus(registry, contributes, out)
  applyIconThemes(registry, contributes, extPath, out)
  applyLanguages(registry, contributes, out)
  applyGrammars(registry, contributes, extPath, out)
  const containers = contributes.viewsContainers as Record<string, { id: string; title: string; icon: string }[]> | undefined
  if (containers) {
    for (const [location, list] of Object.entries(containers)) {
      for (const c of list) {
        out.push(registry.registerViewContainer({
          id: c.id, title: c.title, icon: c.icon,
          location: location === "auxiliarybar" ? "auxiliarybar" : location === "panel" ? "panel" : location === "customize" ? "customize" : "activitybar",
        }))
        // VS Code-parity: `workbench.view.extension.<id>` reveals the container.
        out.push(registerContainerRevealCommand(registry, c))
      }
    }
  }
  const views = contributes.views as Record<string, { id: string; name: string; type?: string; icon?: string }[]> | undefined
  if (views) {
    for (const [containerId, list] of Object.entries(views)) {
      for (const v of list) {
        const view: RegistryView = { id: v.id, name: v.name, containerId }
        if (v.type === "webview") view.type = "webview"
        if (typeof v.icon === "string") view.icon = v.icon
        out.push(registry.registerView(view))
        // VS Code-parity: `<viewId>.focus` reveals + focuses the view.
        out.push(registerViewFocusCommand(registry, v.id, containerId))
      }
    }
  }
  const themes = contributes.themes
  if (Array.isArray(themes)) {
    for (const raw of themes as { id?: unknown; label?: unknown; uiTheme?: unknown; path?: unknown }[]) {
      if (typeof raw.path !== "string" || typeof raw.label !== "string") continue
      const abs = resolve(extPath, raw.path)
      if (abs !== extPath && !abs.startsWith(extPath + sep)) continue // path traversal — skip
      const id = typeof raw.id === "string" ? raw.id : raw.label
      // Preserve all four VS Code kinds so the picker can split light vs dark
      // correctly (a `hc-light` theme is LIGHT, not dark).
      const uiTheme = raw.uiTheme === "vs" || raw.uiTheme === "hc-black" || raw.uiTheme === "hc-light" ? raw.uiTheme : "vs-dark"
      out.push(registry.registerTheme({ id, label: raw.label, uiTheme, path: abs }))
    }
  }
  return out
}

/** `contributes.iconThemes` — `{ id, label, path }`; path resolved to absolute
 *  against the extension dir and guarded against escaping it. */
function applyIconThemes(registry: Registry, contributes: Record<string, unknown>, extPath: string, out: Disposable[]): void {
  const themes = contributes.iconThemes
  if (!Array.isArray(themes)) return
  for (const raw of themes as { id?: unknown; label?: unknown; path?: unknown }[]) {
    if (typeof raw.id !== "string" || typeof raw.path !== "string") continue
    const abs = resolve(extPath, raw.path)
    if (abs !== extPath && !abs.startsWith(extPath + sep)) continue // path traversal — skip
    const label = typeof raw.label === "string" ? raw.label : raw.id
    out.push(registry.registerIconTheme({ id: raw.id, label, path: abs }))
  }
}

function asRecord(v: unknown): Record<string, unknown> | null {
  return v && typeof v === "object" ? (v as Record<string, unknown>) : null
}

/** Keep only the string members of an untrusted array field (manifest values
 *  cross the extension boundary — a non-string entry is dropped, never cast). */
function strArray(v: unknown): string[] | undefined {
  if (!Array.isArray(v)) return undefined
  const out = v.filter((x): x is string => typeof x === "string")
  return out.length ? out : undefined
}

/** `contributes.languages` — `{ id, extensions, filenames, aliases, firstLine }`.
 *  Feeds the renderer's Monaco language registry so contributed languages (e.g.
 *  Vue's `.vue`) resolve to their real id and their LSP selectors match. */
function applyLanguages(registry: Registry, contributes: Record<string, unknown>, out: Disposable[]): void {
  const langs = contributes.languages
  if (!Array.isArray(langs)) return
  for (const raw of langs) {
    const l = asRecord(raw)
    if (!l || typeof l.id !== "string") continue
    const extensions = strArray(l.extensions)
    const filenames = strArray(l.filenames)
    const aliases = strArray(l.aliases)
    out.push(registry.registerLanguage({
      id: l.id,
      ...(extensions ? { extensions } : {}),
      ...(filenames ? { filenames } : {}),
      ...(aliases ? { aliases } : {}),
      ...(typeof l.firstLine === "string" ? { firstLine: l.firstLine } : {}),
    }))
  }
}

/** `contributes.grammars` — `{ scopeName, language, path, embeddedLanguages }`.
 *  `path` is resolved to absolute against the extension dir (traversal-guarded,
 *  like themes); the renderer reads the grammar file via vscode-textmate. */
function applyGrammars(registry: Registry, contributes: Record<string, unknown>, extPath: string, out: Disposable[]): void {
  const grammars = contributes.grammars
  if (!Array.isArray(grammars)) return
  for (const raw of grammars) {
    const g = asRecord(raw)
    if (!g || typeof g.scopeName !== "string" || typeof g.path !== "string") continue
    const abs = resolve(extPath, g.path)
    if (abs !== extPath && !abs.startsWith(extPath + sep)) continue // path traversal — skip
    const embedded = asRecord(g.embeddedLanguages)
    const injectTo = strArray(g.injectTo)
    out.push(registry.registerGrammar({
      scopeName: g.scopeName,
      path: abs,
      ...(typeof g.language === "string" ? { language: g.language } : {}),
      ...(embedded ? { embeddedLanguages: embedded as Record<string, string> } : {}),
      ...(injectTo ? { injectTo } : {}),
    }))
  }
}

/** Sanitize a `contributes.menus` item — only string fields survive. */
function toMenuItem(raw: unknown): RegistryMenuItem | null {
  const it = asRecord(raw)
  if (!it) return null
  if (typeof it.command !== "string" && typeof it.submenu !== "string") return null
  const item: RegistryMenuItem = {}
  if (typeof it.command === "string") item.command = it.command
  if (typeof it.submenu === "string") item.submenu = it.submenu
  if (typeof it.when === "string") item.when = it.when
  if (typeof it.group === "string") item.group = it.group
  if (typeof it.alt === "string") item.alt = it.alt
  return item
}

function applyMenus(registry: Registry, contributes: Record<string, unknown>, out: Disposable[]): void {
  const menus = asRecord(contributes.menus)
  if (!menus) return
  for (const [menuId, items] of Object.entries(menus)) {
    if (!Array.isArray(items)) continue
    for (const raw of items) {
      const item = toMenuItem(raw)
      if (item) out.push(registry.registerMenuItem(menuId, item))
    }
  }
}

function applyKeybindings(registry: Registry, contributes: Record<string, unknown>, out: Disposable[]): void {
  const kbs = contributes.keybindings
  if (!Array.isArray(kbs)) return
  for (const raw of kbs) {
    const kb = asRecord(raw)
    if (!kb || typeof kb.key !== "string" || typeof kb.command !== "string") continue
    // A `-command` entry is an UNBIND directive (VS Code removal), not a real
    // binding — skip it rather than registering a phantom "-foo" command.
    // (Honouring it as a removal is a later step.)
    if (kb.command.startsWith("-")) continue
    const item: RegistryKeybinding = { key: kb.key, command: kb.command }
    if (typeof kb.mac === "string") item.mac = kb.mac
    if (typeof kb.linux === "string") item.linux = kb.linux
    if (typeof kb.win === "string") item.win = kb.win
    if (typeof kb.when === "string") item.when = kb.when
    if ("args" in kb) item.args = kb.args
    out.push(registry.registerKeybinding(item))
  }
}

function applySubmenus(registry: Registry, contributes: Record<string, unknown>, out: Disposable[]): void {
  const subs = contributes.submenus
  if (!Array.isArray(subs)) return
  for (const raw of subs) {
    const sm = asRecord(raw)
    if (!sm || typeof sm.id !== "string" || typeof sm.label !== "string") continue
    const item: RegistrySubmenu = { id: sm.id, label: sm.label }
    if (typeof sm.icon === "string") item.icon = sm.icon
    out.push(registry.registerSubmenu(item))
  }
}

/** Contributed command titles, applied post-activation when the handlers exist. */
export function applyCommandTitles(registry: Registry, manifest: Record<string, unknown>): void {
  const contributes = manifest.contributes as Record<string, unknown> | undefined
  const cmds = contributes?.commands as { command: string; title: string; category?: string }[] | undefined
  if (!Array.isArray(cmds)) return
  for (const c of cmds) {
    if (registry.hasCommand(c.command)) registry.setCommandTitle(c.command, c.title, c.category)
  }
}

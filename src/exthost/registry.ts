// Central registry. Holds commands, views, view containers, themes,
// context keys. Owned by the extension host (in-process today).
// Snapshots are pushed to the renderer via IPC.
import type {
  Disposable,
  RegistryCommand,
  RegistryGrammar,
  RegistryKeybinding,
  RegistryLanguage,
  RegistryMenuItem,
  RegistrySnapshot,
  RegistrySubmenu,
  RegistryView,
  RegistryViewContainer,
} from "../api/types.js"
import { evaluateWhen } from "./when-clause.js"

type CommandHandler = (...args: unknown[]) => unknown

interface RegisteredCommand extends RegistryCommand {
  handler: CommandHandler
  // Note: `category` is inherited from RegistryCommand as `string | undefined`.
  // Stored as undefined when missing rather than omitted, to keep the
  // shape consistent under exactOptionalPropertyTypes.
}

export class Registry {
  private readonly commands = new Map<string, RegisteredCommand>()
  private readonly views = new Map<string, RegistryView>()
  private readonly viewContainers = new Map<string, RegistryViewContainer>()
  // Colour themes (`contributes.themes`) — `path` is the ABSOLUTE path to the
  // theme JSON, so the renderer can load + apply its colours/tokenColors.
  private readonly themes = new Map<string, { id: string; label: string; uiTheme: "vs-dark" | "vs" | "hc-black" | "hc-light"; path: string }>()
  // File icon themes (`contributes.iconThemes`, B10) — `path` is the ABSOLUTE
  // path to the theme JSON (resolved at parse time from the extension dir).
  private readonly iconThemes = new Map<string, { id: string; label: string; path: string }>()
  // Contributed languages (`contributes.languages`) keyed by id; multiple
  // extensions contributing the same id merge their ext/filename/alias lists.
  private readonly languages = new Map<string, RegistryLanguage>()
  // Contributed TextMate grammars (`contributes.grammars`) keyed by scopeName.
  private readonly grammars = new Map<string, RegistryGrammar>()
  // B4 contributions — stored raw (with `when` strings); the renderer
  // resolves them against the context via the shared when-clause engine.
  private readonly menus = new Map<string, RegistryMenuItem[]>()
  private readonly keybindings: RegistryKeybinding[] = []
  private readonly submenus = new Map<string, RegistrySubmenu>()
  private contextKeys: Record<string, unknown> = {
    isWindows: process.platform === "win32",
    isMac: process.platform === "darwin",
    isLinux: process.platform === "linux",
    "kamin.workspaceTrusted": true,
    "kamin.editorFocus": false,
  }
  private readonly listeners = new Set<() => void>()
  // B3: when executeCommand misses, give the activation engine a chance to
  // lazily activate the extension that contributes the command, then retry.
  private missingCommandResolver?: (id: string) => Promise<void>

  setMissingCommandResolver(fn: (id: string) => Promise<void>): void {
    this.missingCommandResolver = fn
  }

  registerCommand(
    id: string,
    handler: CommandHandler,
    meta?: { title?: string; category?: string; source?: string },
  ): Disposable {
    if (this.commands.has(id)) {
      throw new Error(`KaminIDE: command "${id}" already registered`)
    }
    this.commands.set(id, {
      id,
      title: meta?.title ?? id,
      category: meta?.category ?? undefined,
      source: meta?.source ?? "host",
      handler,
    })
    this.notify()
    return { dispose: () => { this.commands.delete(id); this.notify() } }
  }

  setCommandTitle(id: string, title: string, category?: string): void {
    const cmd = this.commands.get(id)
    if (!cmd) return
    cmd.title = title
    if (category) cmd.category = category
    this.notify()
  }

  hasCommand(id: string): boolean {
    return this.commands.has(id)
  }

  async executeCommand(id: string, ...args: unknown[]): Promise<unknown> {
    let cmd = this.commands.get(id)
    if (!cmd && this.missingCommandResolver) {
      // The command may belong to a not-yet-activated extension — let the
      // activation engine fire `onCommand:<id>`, then look again.
      await this.missingCommandResolver(id)
      cmd = this.commands.get(id)
    }
    if (!cmd) throw new Error(`KaminIDE: command "${id}" not found`)
    return await cmd.handler(...args)
  }

  registerView(view: RegistryView): Disposable {
    this.views.set(view.id, view)
    this.notify()
    return { dispose: () => { this.views.delete(view.id); this.notify() } }
  }

  registerViewContainer(container: RegistryViewContainer): Disposable {
    this.viewContainers.set(container.id, container)
    this.notify()
    return { dispose: () => { this.viewContainers.delete(container.id); this.notify() } }
  }

  registerTheme(theme: { id: string; label: string; uiTheme: "vs-dark" | "vs" | "hc-black" | "hc-light"; path: string }): Disposable {
    this.themes.set(theme.id, theme)
    this.notify()
    return { dispose: () => { this.themes.delete(theme.id); this.notify() } }
  }

  registerIconTheme(theme: { id: string; label: string; path: string }): Disposable {
    this.iconThemes.set(theme.id, theme)
    this.notify()
    return { dispose: () => { this.iconThemes.delete(theme.id); this.notify() } }
  }

  registerLanguage(lang: RegistryLanguage): Disposable {
    const prev = this.languages.get(lang.id)
    // Merge into an existing id (VS Code lets several extensions extend one
    // language) — union the ext/filename/alias lists, deduped.
    const merge = (a?: string[], b?: string[]): string[] | undefined => {
      if (!a && !b) return undefined
      return [...new Set([...(a ?? []), ...(b ?? [])])]
    }
    const extensions = merge(prev?.extensions, lang.extensions)
    const filenames = merge(prev?.filenames, lang.filenames)
    const aliases = merge(prev?.aliases, lang.aliases)
    const firstLine = lang.firstLine ?? prev?.firstLine
    const merged: RegistryLanguage = {
      id: lang.id,
      ...(extensions ? { extensions } : {}),
      ...(filenames ? { filenames } : {}),
      ...(aliases ? { aliases } : {}),
      ...(firstLine ? { firstLine } : {}),
    }
    this.languages.set(lang.id, merged)
    this.notify()
    // Dispose removes only this contribution's fields would require ref-counting;
    // a contributed language living for the session is fine — drop the whole id.
    return { dispose: () => { this.languages.delete(lang.id); this.notify() } }
  }

  registerGrammar(grammar: RegistryGrammar): Disposable {
    this.grammars.set(grammar.scopeName, grammar)
    this.notify()
    return { dispose: () => { this.grammars.delete(grammar.scopeName); this.notify() } }
  }

  registerMenuItem(menuId: string, item: RegistryMenuItem): Disposable {
    const list = this.menus.get(menuId) ?? []
    list.push(item)
    this.menus.set(menuId, list)
    this.notify()
    return { dispose: () => {
      const cur = this.menus.get(menuId)
      if (cur) this.menus.set(menuId, cur.filter((i) => i !== item))
      this.notify()
    } }
  }

  registerKeybinding(kb: RegistryKeybinding): Disposable {
    this.keybindings.push(kb)
    this.notify()
    return { dispose: () => {
      const i = this.keybindings.indexOf(kb)
      if (i >= 0) this.keybindings.splice(i, 1)
      this.notify()
    } }
  }

  registerSubmenu(submenu: RegistrySubmenu): Disposable {
    this.submenus.set(submenu.id, submenu)
    this.notify()
    return { dispose: () => { this.submenus.delete(submenu.id); this.notify() } }
  }

  setContext(key: string, value: unknown): void {
    this.contextKeys[key] = value
    this.notify()
  }

  /** Evaluate a `when` clause against the current context keys (B4). Empty
   *  clause → true; malformed → false. Used by menu/keybinding wiring. */
  evaluateWhen(expr: string | undefined): boolean {
    return evaluateWhen(expr, this.contextKeys)
  }

  onUpdate(fn: () => void): Disposable {
    this.listeners.add(fn)
    return { dispose: () => this.listeners.delete(fn) }
  }

  snapshot(): RegistrySnapshot {
    return {
      commands: [...this.commands.values()].map(({ handler: _h, ...rest }) => rest),
      views: [...this.views.values()],
      viewContainers: [...this.viewContainers.values()],
      themes: [...this.themes.values()],
      iconThemes: [...this.iconThemes.values()],
      languages: [...this.languages.values()],
      grammars: [...this.grammars.values()],
      contextKeys: { ...this.contextKeys },
      menus: Object.fromEntries(this.menus),
      keybindings: [...this.keybindings],
      submenus: [...this.submenus.values()],
    }
  }

  private notify(): void {
    for (const fn of this.listeners) {
      try { fn() } catch (e) { console.error("Registry listener threw:", e) }
    }
  }
}

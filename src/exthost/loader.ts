// Extension loader — orchestrates discovery, the `require('vscode')`
// hook, and deferred `activate(context)` for each builtin extension.
// Per-piece concerns live in siblings:
//   - `loader-discovery.ts`   — manifest discovery + main path-traversal guard
//   - `loader-context.ts`     — ExtensionContext + Extension facade builders
//   - `loader-contributes.ts` — applying contributes to the registry
//   - `activation-manager.ts` — WHEN each extension activates
//
// Flow (B3): `prepareAll` does a config-defaults pass, then `prepare`s every
// extension (descriptor + facade + static contributions) WITHOUT running it,
// then activates only startup-triggered ones; the rest stay queued in the
// ActivationManager until their event fires (`onCommand:<id>`, …).
// Current scope: built-in extensions only (`opts.builtinDir`); persistence is
// process-local (globalState/workspaceState are no-ops, pending B9).
import { createRequire } from "node:module"
import type { Disposable, ExtensionDescriptor } from "../api/types.js"
import { ActivationManager } from "./activation-manager.js"
import type { ExtensionFacade } from "./api/types.js"
import { createKaminideApi, type KaminideApi } from "./api-kaminide.js"
import { createVscodeApi } from "./api.js"
import type { VscodeApi } from "./api.js"
import { installEsmVscodeHook, loadExtensionModule } from "./esm-vscode-hook.js"
import {
  makeContext, makeFacade, facadeSetActive, facadeSetExports,
} from "./loader-context.js"
import { applyStaticContributions, applyCommandTitles } from "./loader-contributes.js"
import {
  discoverExtensions, coerceManifestString, resolveExtensionMain, parseConfigDefaults,
  buildDescriptor, normalizeActivationEvents, findOwningExtension, parseExtensionDependencies, manifestId,
} from "./loader-discovery.js"
import { installVscodeRequireHook } from "./loader-hook.js"
import { installFromDir, uninstallSideloaded, type KnownEntry, type SideloadOps } from "./loader-install.js"
import type { LoaderOptions } from "./loader-options.js"

interface ExtModule {
  activate?: (ctx: unknown) => unknown
  deactivate?: () => unknown
}

export interface LoadedExtension {
  descriptor: ExtensionDescriptor
  subscriptions: Disposable[]
  facade: ExtensionFacade
  /** Disposables for STATIC contributions (menus/keybindings/views/…) applied
   *  at load — dropped on unload so they don't linger in the snapshot. */
  contributions: Disposable[]
  /** The required CJS module — kept so `unloadAll` can call `deactivate()`. */
  module?: ExtModule
}

export class ExtensionLoader {
  private readonly requireFromHere = createRequire(import.meta.url)
  private readonly loaded = new Map<string, LoadedExtension>()
  private readonly vscodeCache = new Map<string, VscodeApi>()
  // The `kaminide` module is shared (session events are global) — built once,
  // never per-extension, so a reload doesn't re-subscribe the host bus.
  private kaminideApi: KaminideApi | null = null
  private readonly ownerCache = new Map<string, string>()
  private readonly activation = new ActivationManager()
  private hookInstalled = false
  // Every discovered extension's manifest+path+origin, so a disabled one can be
  // re-prepared on enable without re-scanning disk, and the UI knows whether it's
  // uninstallable (sideloaded) vs built-in.
  private readonly known = new Map<string, KnownEntry>()
  // User-disabled ids (persisted) — skipped at load, listed as enabled:false.
  private readonly disabledIds = new Set<string>()

  // Reserved storage namespace for the host's own state (not a real extension).
  private static readonly CORE_ID = "kaminide.core"
  private static readonly DISABLED_KEY = "disabledExtensions"

  constructor(private readonly opts: LoaderOptions) {
    // B3: a contributed command invoked before its extension is active fires
    // `onCommand:<id>`, which activates the owning extension (whose activate()
    // then registers the real handler the registry retries against).
    this.opts.registry.setMissingCommandResolver((id) => this.activation.fireEvent(`onCommand:${id}`))
  }

  installRequireHook(): void {
    if (this.hookInstalled) return
    this.hookInstalled = true
    installVscodeRequireHook((id) => this.requireFromHere(id), (p) => this.resolveApi(p), () => this.resolveKaminideApi())
    // ESM extensions `import` vscode instead of require'ing it — a separate
    // resolver that re-exports the global namespace api.
    installEsmVscodeHook(this.resolveApi("__esm_global__"))
  }

  /** The shared `kaminide` module — built once on first `require('kaminide')`. */
  private resolveKaminideApi(): KaminideApi {
    this.kaminideApi ??= createKaminideApi(this.opts.sessionsApi)
    return this.kaminideApi
  }

  /** Cached per-extension `vscode` api for the module at `parentPath`. */
  private resolveApi(parentPath: string): VscodeApi {
    const extId = findOwningExtension(parentPath, this.ownerCache) ?? "unknown"
    let api = this.vscodeCache.get(extId)
    if (!api) {
      // Pass ONLY the NsHooks fields (not LoaderOptions' builtinDir/storage/
      // sessionsApi) — explicit so a new NsHooks field fails to compile until
      // it's wired here, rather than silently leaking a LoaderOptions value.
      const { registry, emitNotification, showMessage, showInputBox, showQuickPick, showOpenDialog, showSaveDialog, openExternal, readClipboard, emitOutputEvent, writeClipboard, workspaceHost, languageFeatures, diagnostics, webviews, treeViews, fileDecorations, statusBar } = this.opts
      api = createVscodeApi({ registry, emitNotification, showMessage, showInputBox, showQuickPick, showOpenDialog, showSaveDialog, openExternal, readClipboard, emitOutputEvent, writeClipboard, listExtensions: () => this.listFacades(), workspaceHost, languageFeatures, diagnostics, webviews, treeViews, fileDecorations, statusBar }, extId)
      this.vscodeCache.set(extId, api)
    }
    return api
  }

  /** Discovery + static contributions + config defaults + `prepare()` for every
   *  enabled extension. FAST (no `activate()` calls) — returns the descriptor
   *  list immediately so the UI shows the extension list + contributed
   *  commands/views/themes without waiting on the slow activation pass. The
   *  caller runs `activateStartup()` afterwards (in the background). */
  prepareAll(): ExtensionDescriptor[] {
    this.installRequireHook()
    // Restore the user's disabled set so those extensions stay unloaded.
    const saved = this.opts.storage.globalGet(ExtensionLoader.CORE_ID, ExtensionLoader.DISABLED_KEY)
    if (Array.isArray(saved)) for (const id of saved) if (typeof id === "string") this.disabledIds.add(id)
    // Built-ins first, then sideloaded (.vsix/folder) from the user dir. A
    // sideloaded id with the same key as a built-in overrides it (later wins).
    const discovered = [
      ...discoverExtensions(this.opts.builtinDir).map((d) => ({ ...d, builtin: true })),
      ...discoverExtensions(this.opts.userExtDir).map((d) => ({ ...d, builtin: false })),
    ]
    // Remember every discovered extension (so a disabled one can be re-prepared
    // on enable) and its origin.
    for (const { manifest, path, builtin } of discovered) this.known.set(manifestId(manifest), { manifest, path, builtin })
    const enabled = discovered.filter(({ manifest }) => !this.disabledIds.has(manifestId(manifest)))
    // Register every ENABLED extension's contributed configuration defaults
    // BEFORE any activate(): VS Code parses all static contributions up front, so
    // an extension reading `getConfiguration` at activation sees the full
    // default set (its own and others'). Disabled extensions contribute nothing.
    for (const { manifest } of enabled) {
      const defaults = parseConfigDefaults(manifest)
      if (Object.keys(defaults).length > 0) this.opts.workspaceHost.config.registerDefaults(defaults)
    }
    // Prepare each enabled extension (descriptor + facade + static contributions),
    // deferring `activate()` to the activation engine.
    for (const { manifest, path, builtin } of enabled) this.prepare(manifest, path, builtin)
    return this.list()
  }


  /** Build the descriptor/facade and apply STATIC contributions (views,
   *  containers, themes — visible without activation), then either mark a
   *  declarative-only extension active or queue it for lazy activation. */
  private prepare(manifest: Record<string, unknown>, extPath: string, builtin: boolean): void {
    const id = manifestId(manifest)
    const descriptor = buildDescriptor(id, manifest, extPath, builtin, false, true)
    const facade = makeFacade(descriptor, extPath, manifest)
    const contributions = applyStaticContributions(this.opts.registry, manifest, extPath)
    this.loaded.set(id, { descriptor, subscriptions: [], facade, contributions })

    const mainRel = coerceManifestString(manifest.main)
    if (!mainRel) {
      // Declarative-only (themes/grammars) — no code to run, active now.
      descriptor.active = true
      facadeSetActive(facade, true)
      return
    }
    const mainPath = resolveExtensionMain(extPath, mainRel)
    if (!mainPath) {
      descriptor.activationError = `manifest.main "${mainRel}" escapes ${extPath}`
      console.error(`KaminIDE: refusing to load ${id} — ${descriptor.activationError}`)
      return
    }
    this.activation.register({
      id,
      events: normalizeActivationEvents(manifest),
      deps: parseExtensionDependencies(manifest),
      activate: () => this.activateOne(id, manifest, extPath, mainPath),
    })
  }

  /** Idempotently run an extension's `activate(context)`. Called by the
   *  activation engine when a trigger fires. The `entry.descriptor.active`
   *  guard is belt-and-suspenders — the ActivationManager already dedups by
   *  removing from `pending` before awaiting `activate`. */
  private async activateOne(id: string, manifest: Record<string, unknown>, extPath: string, mainPath: string): Promise<void> {
    const entry = this.loaded.get(id)
    if (!entry || entry.descriptor.active) return
    try {
      // ESM extensions load via import(); CJS via require — see loadExtensionModule.
      const ext = await loadExtensionModule(mainPath, manifest, (p) => this.requireFromHere(p)) as ExtModule
      const subscriptions: Disposable[] = []
      const context = makeContext(extPath, subscriptions, entry.facade, this.opts.storage, this.opts.env, id)
      let exports: unknown
      if (typeof ext.activate === "function") {
        exports = await Promise.resolve(ext.activate(context))
      }
      entry.module = ext
      entry.subscriptions = subscriptions
      entry.descriptor.active = true
      facadeSetActive(entry.facade, true)
      facadeSetExports(entry.facade, exports)
      applyCommandTitles(this.opts.registry, manifest)
    } catch (err) {
      entry.descriptor.active = false
      // Extensions can `throw` ANYTHING — undefined (golang.Go), a plain object
      // with a `message` (redhat.java's "needs a JDK"), a string. Reading
      // `.stack`/`.message` off a non-object crashes THIS handler, and that
      // crash escapes activateOne and aborts the whole load — so be defensive.
      const detail = err instanceof Error ? (err.stack ?? err.message)
        : (typeof err === "object" && err !== null && "message" in err) ? String(err.message)
          : String(err)
      entry.descriptor.activationError = detail
      console.error(`KaminIDE: failed to activate ${id}:`, err)
      // Surface lazy-activation failures too — without this they were only
      // visible in DevTools (the startup notification covers startup only).
      this.opts.emitNotification({
        severity: "error",
        message: `Extension ${id} failed to activate: ${detail.split("\n")[0]}`,
      })
    }
  }

  unloadAll(): void {
    for (const [id, ext] of this.loaded) {
      // VS Code calls the extension's deactivate() before disposing the
      // context subscriptions — external resources get a chance to close.
      try { ext.module?.deactivate?.() } catch (err) {
        console.warn(`KaminIDE: deactivate threw for ${id}:`, err)
      }
      for (const s of [...ext.subscriptions, ...ext.contributions]) {
        try { s.dispose() } catch (err) {
          console.warn(`KaminIDE: dispose threw for ${id}:`, err)
        }
      }
    }
    this.loaded.clear()
    // Drop cached api shims + owner lookups so a re-load yields fresh
    // `extensions.all` closures rather than serving stale facades.
    this.vscodeCache.clear()
    this.ownerCache.clear()
  }

  /** Enable or disable an extension at runtime — no app restart. Disabling
   *  deactivates it and disposes all its contributions live; enabling re-prepares
   *  + activates it. The choice persists across restarts. */
  async setExtensionEnabled(id: string, enabled: boolean): Promise<void> {
    if (enabled) {
      if (!this.disabledIds.has(id)) return
      this.disabledIds.delete(id)
      this.persistDisabled()
      const k = this.known.get(id)
      if (!k) return
      const defaults = parseConfigDefaults(k.manifest)
      if (Object.keys(defaults).length > 0) this.opts.workspaceHost.config.registerDefaults(defaults)
      this.prepare(k.manifest, k.path, k.builtin)
      // Activate it if its triggers say so (idempotent for the already-active rest).
      await this.activateStartup()
    } else {
      if (this.disabledIds.has(id)) return
      this.unloadOne(id)
      this.disabledIds.add(id)
      this.persistDisabled()
    }
  }

  /** Tear one extension down: deactivate, dispose its subscriptions + static
   *  contributions (which removes its commands/views/providers live), and drop
   *  its cached api so a later enable rebuilds fresh. */
  private unloadOne(id: string): void {
    const ext = this.loaded.get(id)
    if (!ext) return
    try { ext.module?.deactivate?.() } catch (err) { console.warn(`KaminIDE: deactivate threw for ${id}:`, err) }
    for (const s of [...ext.subscriptions, ...ext.contributions]) {
      try { s.dispose() } catch (err) { console.warn(`KaminIDE: dispose threw for ${id}:`, err) }
    }
    this.loaded.delete(id)
    this.vscodeCache.delete(id)
    for (const [path, owner] of this.ownerCache) if (owner === id) this.ownerCache.delete(path)
    this.activation.unregister(id)
  }

  private persistDisabled(): void {
    this.opts.storage.globalUpdate(ExtensionLoader.CORE_ID, ExtensionLoader.DISABLED_KEY, [...this.disabledIds])
  }

  /** Loaded (enabled) descriptors + a stub for each disabled-but-known one so
   *  the UI can list them and offer to re-enable. */
  list(): ExtensionDescriptor[] {
    const out = [...this.loaded.values()].map((l) => l.descriptor)
    for (const id of this.disabledIds) {
      const k = this.known.get(id)
      if (k) out.push(buildDescriptor(id, k.manifest, k.path, k.builtin, false, false))
    }
    return out
  }

  /** Public-shape Extension list — what `vscode.extensions.all` returns. */
  listFacades(): readonly ExtensionFacade[] {
    return [...this.loaded.values()].map((l) => l.facade)
  }

  /** Install (or reinstall) a sideloaded extension already extracted at `extDir`,
   *  live (see loader-install.ts). */
  installFromDir(extDir: string): Promise<ExtensionDescriptor> {
    return installFromDir(this.sideloadOps(), extDir)
  }

  /** Uninstall a sideloaded extension live; returns its dir to delete. */
  uninstall(id: string): string {
    return uninstallSideloaded(this.sideloadOps(), id)
  }

  /** The narrow capability loader-install.ts operates over. */
  private sideloadOps(): SideloadOps {
    return {
      loaded: this.loaded,
      known: this.known,
      registerDefaults: (v) => { this.opts.workspaceHost.config.registerDefaults(v) },
      prepare: (m, p, b) => { this.prepare(m, p, b) },
      unload: (id) => { this.unloadOne(id) },
      clearDisabled: (id) => { if (this.disabledIds.delete(id)) this.persistDisabled() },
      activate: () => this.activateStartup(),
    }
  }

  /** Activate any extension waiting on `onLanguage:<id>` — fired when a document
   *  of that language opens (many language extensions — redhat.java, angular,
   *  fwcd.kotlin — activate on their language, not workspaceContains). */
  async activateByLanguage(languageId: string): Promise<void> {
    await this.activation.fireEvent(`onLanguage:${languageId}`)
  }

  /** Run the activation pass. `*` and `onStartupFinished` fire IMMEDIATELY and
   *  do NOT wait for the workspace listing: extensions that activate on those
   *  events (claude-bridge and its webview views among them) have nothing to do
   *  with the file list, while `listFiles()` on a huge root (a home directory)
   *  can take minutes — that used to leave the Bridge panels stuck on their
   *  placeholders and the extension `idle` forever.
   *  `workspaceContains:<glob>` genuinely needs the listing, so it runs in the
   *  background once files arrive; `activateStartup` is idempotent (`run`
   *  removes from `pending` before awaiting), so the second pass only picks up
   *  what the first one could not match. */
  async activateStartup(): Promise<void> {
    await this.activation.activateStartup([])
    await this.activation.fireEvent("onStartupFinished")
    void this.opts.workspaceHost
      .listFiles()
      .then((entries) => this.activation.activateStartup(entries.map((f) => f.rel)))
      .catch(() => { /* listing is best-effort: it only gates workspaceContains */ })
  }
}

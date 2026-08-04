// Host-side webview registry + panel factory (B7a). A single host-wide
// instance routes messages between extensions and the renderer's sandboxed
// iframes and tracks panel lifecycle. An extension's `createWebviewPanel`
// returns a WebviewPanel whose `webview.html =` / `postMessage` broadcast to
// the renderer, and whose `onDidReceiveMessage` fires when the renderer relays
// a message back from the iframe. Resource loading (asWebviewUri / local roots)
// and state persistence are deferred to B7c.
import type { URI } from "vscode-uri"
import { EventEmitter } from "./shared.js"
import { splitCreateOptions } from "./webview-options.js"
import { WebviewPostQueue } from "./webview-post-queue.js"
import { asWebviewUriFor, resolveRoots, WEBVIEW_CSP_SOURCE } from "./webview-uris.js"

// NOTE(multi-window): `broadcast` fans out to every connected renderer, so a
// panel renders in all windows. Single-window is the architectural invariant
// today (same as the B5b-2b editor write path); revisit for multi-window.
type Broadcast = (channel: string, payload: unknown) => void

/** `webview.options` slice (d.ts WebviewOptions). */
export interface WebviewOptions {
  enableScripts?: boolean
  enableForms?: boolean
  enableCommandUris?: boolean | readonly string[]
  localResourceRoots?: readonly URI[] // B7c-2: honored for asWebviewUri/CSP
  portMapping?: readonly unknown[]
}

/** `panel.options` slice (d.ts WebviewPanelOptions) — distinct from the webview
 *  slice. `createWebviewPanel` receives the intersection of both. */
export interface WebviewPanelOptions {
  enableFindWidget?: boolean
  retainContextWhenHidden?: boolean
}

export type CreateWebviewOptions = WebviewPanelOptions & WebviewOptions

const VIEW_COLUMN_ONE = 1

/** vscode ShowOptions: a ViewColumn number or `{ viewColumn, preserveFocus }`. */
function viewColumnOf(showOptions: unknown): number {
  if (typeof showOptions === "number") return showOptions
  if (showOptions && typeof showOptions === "object" && "viewColumn" in showOptions) {
    const v = (showOptions).viewColumn
    if (typeof v === "number") return v
  }
  return VIEW_COLUMN_ONE
}

/** Registry-facing controls for one live panel — how inbound renderer events
 *  reach the extension-facing emitters. */
interface PanelControls {
  receiveMessage: (msg: unknown) => void
  setViewState: (active: boolean, visible: boolean) => void
  fireDispose: () => void
  /** Host-initiated close (e.g. owning extension disabled): disposes the panel
   *  AND broadcasts the dispose so the renderer drops the tab. Views omit it. */
  hostDispose?: () => void
}

/** The `webview-${n}` id used as the panel's renderer tab key (`webview://id`). */
const idOf = (n: number): string => `webview-${String(n)}`

/** A `WebviewViewProvider` (B7b) — resolveWebviewView is called lazily when the
 *  view first becomes visible in the renderer. */
interface WebviewViewProvider {
  resolveWebviewView: (view: unknown, context: { state: unknown }, token: unknown) => void | Promise<void>
}

/** Per-id provider + the extension root used for default localResourceRoots. */
interface ViewRegistration {
  provider: WebviewViewProvider
  extensionRoot: string | undefined
}

/** Read live workspace folder paths for default localResourceRoots. */
interface WorkspaceRootsHost {
  getFolderPath: () => string | null
}

/** `WebviewPanelSerializer` — restores a panel's content across restarts. */
interface WebviewPanelSerializerLike {
  deserializeWebviewPanel: (panel: unknown, state: unknown) => void | Promise<void>
}

/** One persisted open panel (for cross-restart restore). */
export interface PersistedPanel { viewType: string; title: string; state: unknown }

/** Disk persistence of open panels (injected; backed by a JsonStore in the host). */
export interface WebviewStore {
  load: () => PersistedPanel[]
  save: (panels: PersistedPanel[]) => void
}

export class Webviews {
  private seq = 0
  private readonly panels = new Map<string, PanelControls>()
  // B7b: registered `registerWebviewViewProvider` providers + the ids already
  // resolved (so a re-show doesn't re-run resolve).
  private readonly viewProviders = new Map<string, ViewRegistration>()
  private readonly resolvedViews = new Set<string>()
  // Last html per resolved view → re-sent on a repeat resolveView (reload/remount
  // = empty renderer cache) so the iframe isn't blank.
  private readonly resolvedHtml = new Map<string, { html: string; roots: string[] }>()
  // WebviewPanelSerializer: viewType → serializer (+ the owning extension root).
  private readonly serializers = new Map<string, { serializer: WebviewPanelSerializerLike; extensionRoot: string | undefined }>()
  // Live panel metadata for cross-restart persistence (id → {viewType,title,state}).
  private readonly panelMeta = new Map<string, PersistedPanel>()
  // id → owning extension id, so disabling an extension closes its panels.
  private readonly panelOwner = new Map<string, string>()
  private restored = false

  // Relay queue for postMessage — coalescing, frame cap, ordering vs other
  // broadcasts, purge-on-close. See webview-post-queue.ts.
  private readonly posts: WebviewPostQueue

  // workspaceHost is optional so unit tests can `new Webviews(broadcast)`; when
  // absent, default localResourceRoots simply omit the workspace folder. store
  // is optional too (no cross-restart persistence in tests).
  constructor(private readonly broadcast: Broadcast, private readonly workspaceHost?: WorkspaceRootsHost, private readonly store?: WebviewStore) {
    this.posts = new WebviewPostQueue(broadcast)
  }

  /** Write all live panels' {viewType,title,state} to disk (best-effort). */
  private persist(): void {
    this.store?.save([...this.panelMeta.values()])
  }

  /** `window.registerWebviewPanelSerializer(viewType, serializer)`. */
  registerSerializer(viewType: string, serializer: WebviewPanelSerializerLike, extensionRoot?: string): { dispose: () => void } {
    this.serializers.set(viewType, { serializer, extensionRoot })
    return { dispose: () => { this.serializers.delete(viewType) } }
  }

  /** Renderer reports a panel's latest getState/setState value → persist it. */
  persistPanelState(id: string, state: unknown): void {
    const meta = this.panelMeta.get(id)
    if (!meta) return
    meta.state = state
    this.persist()
  }

  /** Re-create panels saved from a previous run whose viewType now has a
   *  serializer, then hand each to its deserializer. Runs once, on the first
   *  renderer connect (so the create broadcasts reach a live renderer). */
  restore(): void {
    if (this.restored) return
    this.restored = true
    const saved = this.store?.load() ?? []
    if (saved.length === 0) return
    for (const entry of saved) {
      const reg = this.serializers.get(entry.viewType)
      if (!reg) continue
      const panel = this.createPanel(entry.viewType, entry.title, 1, { enableScripts: true }, reg.extensionRoot, entry.state)
      void reg.serializer.deserializeWebviewPanel(panel, entry.state)
    }
  }

  private workspaceRoots(): string[] {
    const p = this.workspaceHost?.getFolderPath()
    return p ? [p] : []
  }

  /** `window.registerWebviewViewProvider(viewId, provider)` — store the provider;
   *  it's invoked when the renderer first shows the view (resolveView). */
  registerWebviewViewProvider(viewId: string, provider: WebviewViewProvider, extensionRoot?: string): { dispose: () => void } {
    this.viewProviders.set(viewId, { provider, extensionRoot })
    return { dispose: () => { this.viewProviders.delete(viewId); this.resolvedViews.delete(viewId); this.resolvedHtml.delete(viewId) } }
  }

  /** Renderer asks to show a webview VIEW (its sidebar body mounted). Resolve it
   *  once: build a webview keyed by `viewId`, call the provider (which sets
   *  `.webview.html`), and route inbound messages/lifecycle via the panels Map. */
  resolveView(viewId: string): void {
    if (this.resolvedViews.has(viewId)) {
      const c = this.resolvedHtml.get(viewId) // provider won't re-run → re-send cached html so a reloaded view isn't blank
      if (c) this.posts.broadcastAfterPosts("kamin:webviewView:html", { id: viewId, html: c.html, localResourceRoots: c.roots })
      return
    }
    const registration = this.viewProviders.get(viewId)
    if (!registration) return
    this.resolvedViews.add(viewId)
    const { resolvedHtml } = this
    const broadcast = this.posts.broadcastAfterPosts.bind(this.posts) // ordered vs queued posts
    const enqueuePost = this.posts.enqueue.bind(this.posts)
    const roots = resolveRoots(undefined, registration.extensionRoot, this.workspaceRoots())
    const recv = new EventEmitter<unknown>()
    const onDispose = new EventEmitter<void>()
    const onVisibility = new EventEmitter<void>()
    const state = { html: "", visible: true }
    const webview = {
      options: { localResourceRoots: roots.uris } as WebviewOptions,
      get html(): string { return state.html },
      set html(v: string) { state.html = v; resolvedHtml.set(viewId, { html: v, roots: roots.fsPaths }); broadcast("kamin:webviewView:html", { id: viewId, html: v, localResourceRoots: roots.fsPaths }) },
      // d.ts (twice): "You cannot send messages to a hidden webview, even with
      // retainContextWhenHidden enabled" — so a hidden VIEW always refuses.
      postMessage: (msg: unknown): Promise<boolean> =>
        state.visible ? enqueuePost(viewId, msg) : Promise.resolve(false),
      onDidReceiveMessage: recv.event,
      asWebviewUri: asWebviewUriFor(viewId),
      cspSource: WEBVIEW_CSP_SOURCE,
    }
    const view = {
      webview,
      get visible(): boolean { return state.visible },
      title: undefined as string | undefined,
      description: undefined as string | undefined,
      badge: undefined as unknown,
      onDidDispose: onDispose.event,
      onDidChangeVisibility: onVisibility.event,
      show: (): void => { broadcast("kamin:webviewView:reveal", { id: viewId }) },
    }
    this.panels.set(viewId, {
      receiveMessage: (msg) => { recv.fire(msg) },
      setViewState: (_active, visible) => {
        if (visible === state.visible) return
        state.visible = visible
        onVisibility.fire()
      },
      fireDispose: () => {
        this.posts.purge(viewId) // renderer already dropped the iframe
        this.panels.delete(viewId)
        this.resolvedViews.delete(viewId)
        this.resolvedHtml.delete(viewId)
        onDispose.fire()
      },
    })
    const noopToken = { isCancellationRequested: false, onCancellationRequested: () => ({ dispose: () => { /* never fires */ } }) }
    void registration.provider.resolveWebviewView(view, { state: undefined }, noopToken)
  }

  createPanel(viewType: string, title: string, showOptions: unknown, options: CreateWebviewOptions, extensionRoot?: string, restoreState?: unknown, ownerExtId?: string) {
    const id = idOf(++this.seq)
    // Capture so the panel's closures never depend on `this`. `broadcast` is the
    // ORDERED wrapper: html/title/reveal/dispose must not overtake queued posts.
    const broadcast = this.posts.broadcastAfterPosts.bind(this.posts)
    const enqueuePost = this.posts.enqueue.bind(this.posts)
    // Track for cross-restart persistence; seed the saved state on restore.
    this.panelMeta.set(id, { viewType, title, state: restoreState })
    if (ownerExtId !== undefined) this.panelOwner.set(id, ownerExtId)
    const updateMeta = (patch: Partial<PersistedPanel>): void => {
      const meta = this.panelMeta.get(id)
      if (meta) { Object.assign(meta, patch); this.persist() }
    }
    const dropMeta = (): void => { this.panelMeta.delete(id); this.panelOwner.delete(id); this.persist() }
    const roots = resolveRoots(options.localResourceRoots, extensionRoot, this.workspaceRoots())
    const recv = new EventEmitter<unknown>()
    const onDispose = new EventEmitter<void>()
    const onViewState = new EventEmitter<unknown>()
    const state = { html: "", title, active: true, visible: true, disposed: false, viewColumn: viewColumnOf(showOptions) }
    const { webviewOptions, panelOptions } = splitCreateOptions(options, roots.uris)

    const webview = {
      options: webviewOptions,
      get html(): string { return state.html },
      set html(v: string) { state.html = v; broadcast("kamin:webview:html", { id, html: v }) },
      postMessage: (msg: unknown): Promise<boolean> => {
        // false when the panel can't receive (disposed, or hidden with no
        // retainContextWhenHidden — its iframe is unmounted). A RETAINED hidden
        // panel keeps its iframe mounted, so it still receives: index.d.ts
        // contradicts itself here (postMessage@10018 allows "hidden webviews
        // that set retainContextWhenHidden"; the option note@10076 says never).
        // We follow the postMessage doc, which is what real VS Code does and
        // what retaining is FOR. Views differ (no retained DOM) and refuse.
        if (state.disposed || (!state.visible && !panelOptions.retainContextWhenHidden)) return Promise.resolve(false)
        return enqueuePost(id, msg)
      },
      onDidReceiveMessage: recv.event,
      asWebviewUri: asWebviewUriFor(id),
      cspSource: WEBVIEW_CSP_SOURCE,
    }

    const dispose = (): void => {
      if (state.disposed) return
      state.disposed = true
      this.panels.delete(id)
      dropMeta()
      broadcast("kamin:webview:dispose", { id })
      onDispose.fire()
    }

    const panel = {
      webview,
      viewType,
      get title(): string { return state.title },
      set title(v: string) { state.title = v; updateMeta({ title: v }); broadcast("kamin:webview:title", { id, title: v }) },
      iconPath: undefined as unknown,
      options: panelOptions,
      get viewColumn(): number { return state.viewColumn },
      get active(): boolean { return state.active },
      get visible(): boolean { return state.visible },
      onDidChangeViewState: onViewState.event,
      onDidDispose: onDispose.event,
      reveal: (viewColumn?: number, preserveFocus?: boolean): void => {
        if (typeof viewColumn === "number") state.viewColumn = viewColumn
        broadcast("kamin:webview:reveal", { id, viewColumn: state.viewColumn, preserveFocus: preserveFocus ?? false })
      },
      dispose,
    }

    this.panels.set(id, {
      receiveMessage: (msg) => { recv.fire(msg) },
      hostDispose: dispose,
      setViewState: (active, visible) => {
        if (active === state.active && visible === state.visible) return
        state.active = active; state.visible = visible
        onViewState.fire({ webviewPanel: panel })
      },
      fireDispose: () => {
        // Renderer-initiated close (user shut the tab): fire onDidDispose
        // WITHOUT re-broadcasting a dispose back to the renderer.
        if (state.disposed) return
        state.disposed = true
        this.posts.purge(id) // its iframe is already gone — don't broadcast into a dead id
        this.panels.delete(id)
        dropMeta()
        onDispose.fire()
      },
    })

    // Send a serializable payload (raw `options` holds Uri[] roots that don't
    // survive the WS hop): the renderer needs the resolved root fs paths to
    // forward to the Rust resource handler, plus retainContextWhenHidden (B7c-2c).
    broadcast("kamin:webview:create", {
      id, viewType, title, ownerExtId, // ownerExtId → tab icon (renderer)
      localResourceRoots: roots.fsPaths,
      retainContextWhenHidden: panelOptions.retainContextWhenHidden ?? false,
      // Seed getState across a cross-restart restore (undefined for fresh panels).
      initialState: restoreState,
    })
    this.persist()
    return panel
  }

  /** A message the renderer relayed from the iframe → the panel's emitter. */
  deliverMessage(id: string, msg: unknown): void { this.panels.get(id)?.receiveMessage(msg) }
  /** The renderer reported the panel's active/visible state changed. */
  updateViewState(id: string, active: boolean, visible: boolean): void { this.panels.get(id)?.setViewState(active, visible) }
  /** The user closed the panel's tab in the renderer. */
  disposeFromRenderer(id: string): void { this.panels.get(id)?.fireDispose() }

  /** Close every panel an extension opened — called when it's disabled so its
   *  webviews don't linger as orphaned, script-running tabs. */
  disposeForExtension(extId: string): void {
    for (const [id, owner] of [...this.panelOwner]) {
      if (owner === extId) this.panels.get(id)?.hostDispose?.()
    }
  }
}

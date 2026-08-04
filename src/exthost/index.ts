// Extension host entry. Phase A runs in the same process as Electron
// main; Phase B will fork it to a worker_thread. Either way, this module
// owns the registry and the loader.
import type { Notification } from "../api/types.js"
import { Diagnostics } from "./api/diagnostics.js"
import { FileDecorations } from "./api/file-decorations.js"
import { LanguageFeatures } from "./api/language-features.js"
import { SessionsApi } from "./api/sessions.js"
import { StatusBar } from "./api/status-bar.js"
import { TreeViews } from "./api/tree-views.js"
import { Webviews } from "./api/webview.js"
import type { ExtHost, StartOptions } from "./exthost-contract.js"
import { ExtensionLoader } from "./loader.js"
import { registerLspCommands } from "./lsp-commands.js"
import { Registry } from "./registry.js"
import { setRevealBroadcast } from "./view-reveal.js"
export type { ExtHost } from "./exthost-contract.js"

// Grace period before `restartExtensionHost` exits the child, so the command's
// RPC response reaches the caller before the process dies.
const RESTART_EXIT_DELAY_MS = 150

// Stays `async` (stable boundary callers `await`); extension activation now runs
// in the BACKGROUND (deferred), so the body itself no longer awaits anything.
// eslint-disable-next-line @typescript-eslint/require-await -- async contract is intentional; activation is deferred
export async function startExtHost(opts: StartOptions): Promise<ExtHost> {
  const registry = new Registry()
  const languageFeatures = new LanguageFeatures()
  setRevealBroadcast(opts.broadcast)
  const diagnostics = new Diagnostics(opts.broadcast)
  const webviews = new Webviews(opts.broadcast, opts.workspaceHost, opts.webviewStore)
  const treeViews = new TreeViews(opts.broadcast)
  const fileDecorations = new FileDecorations(opts.broadcast)
  const statusBar = new StatusBar(opts.broadcast)
  const sessionsApi = new SessionsApi(opts.sessionsHost)
  const emitNotification = (n: { severity: "info" | "warning" | "error"; message: string }) => {
    const payload: Notification = { ...n, timestamp: Date.now() }
    opts.broadcast("kamin:notification:show", payload)
  }
  const loader = new ExtensionLoader({
    builtinDir: opts.builtinDir,
    userExtDir: opts.userExtDir,
    registry,
    emitNotification,
    showMessage: opts.showMessage,
    showInputBox: opts.showInputBox,
    showQuickPick: opts.showQuickPick,
    showOpenDialog: opts.showOpenDialog,
    showSaveDialog: opts.showSaveDialog,
    openExternal: opts.openExternal,
    readClipboard: opts.readClipboard,
    emitOutputEvent: opts.emitOutputEvent,
    writeClipboard: (text: string) => { opts.broadcast("kamin:clipboard:write", text); },
    workspaceHost: opts.workspaceHost,
    storage: opts.storage,
    env: opts.env,
    languageFeatures,
    diagnostics,
    webviews,
    treeViews,
    fileDecorations,
    statusBar,
    sessionsApi,
  })

  // Canonical VS Code workbench command IDs (per
  // research/vscode-extension-api/16-open-questions-resolved.md §1).
  // Marketplace extensions reference these by literal string —
  // `commands.executeCommand("workbench.action.toggleSidebarVisibility")`
  // is one of the most-used patterns. Lock at the unprefixed name.
  registry.registerCommand("workbench.action.showCommands", () => {
    opts.broadcast("kamin:command-palette:open", null)
  }, { title: "Show All Commands", category: "View" })

  registry.registerCommand("workbench.action.toggleAuxiliaryBar", () => {
    opts.broadcast("kamin:layout:toggle", "auxiliaryBar")
  }, { title: "Toggle Auxiliary Bar", category: "View" })

  registry.registerCommand("workbench.action.togglePanel", () => {
    opts.broadcast("kamin:layout:toggle", "panel")
  }, { title: "Toggle Panel", category: "View" })

  registry.registerCommand("workbench.action.toggleSidebarVisibility", () => {
    opts.broadcast("kamin:layout:toggle", "primarySideBar")
  }, { title: "Toggle Primary Side Bar", category: "View" })

  // Reload the renderer (full re-init). Extensions prompt this after install/update.
  registry.registerCommand("workbench.action.reloadWindow", () => {
    opts.broadcast("kamin:window:reload", null)
  }, { title: "Reload Window", category: "Developer" })

  // VS Code's built-in "Reveal in File Explorer/Finder". Extensions (incl. the
  // Bridge's "Show in Explorer" on Skills/Agents) call it with a Uri; without it
  // registered the executeCommand rejected → surfaced as an "Extension crashed"
  // toast. Extract the path and hand it to the host's OS-reveal via the renderer.
  registry.registerCommand("revealFileInOS", (arg: unknown) => {
    const u = arg as { fsPath?: string; path?: string } | string | undefined
    const p = typeof u === "string" ? u : (u?.fsPath ?? u?.path)
    if (p) opts.broadcast("kamin:fs:reveal", p)
  }, { title: "Reveal in File Explorer", category: "File" })

  // Restart the ext-host: this child exits cleanly → the parent supervisor
  // respawns it + re-seeds the renderer mirror. Many extensions (Vue/volar,
  // C#) need this to activate language support after install. Deferred so the
  // command's RPC response returns before the process dies.
  registry.registerCommand("workbench.action.restartExtensionHost", () => {
    opts.broadcast("kamin:exthost:restarting", null)
    setTimeout(() => process.exit(0), RESTART_EXIT_DELAY_MS)
  }, { title: "Restart Extension Host", category: "Developer" })

  // `setContext` (B4) — extensions set context keys via
  // `commands.executeCommand("setContext", key, value)` to drive `when`
  // clauses on their own menus/views.
  registry.registerCommand("setContext", (key, value) => {
    registry.setContext(String(key), value)
  }, { title: "Set Context", category: "Developer" })

  // Built-in commands extensions fire-and-forget at activation for UI we don't
  // implement yet (Testing UI, …). VS Code ships these, so their absence is our
  // gap — registering no-ops keeps activate() from throwing "command not found".
  // Add ids here as host.log surfaces them. (e.g. Shopify.ruby-lsp #23)
  for (const id of ["testing.clearTestResults"]) {
    registry.registerCommand(id, () => undefined)
  }

  // `vscode.execute*Provider` commands — the claude-bridge VSIX's MCP LSP tools
  // (LspHover/LspDefinition/LspReferences) route through these to reach the live
  // language providers hosted here.
  registerLspCommands(registry, languageFeatures, opts.workspaceHost)

  // Push registry updates to the renderer — COALESCED. Activation registers
  // hundreds of commands/menus/keybindings per extension, each firing onUpdate;
  // broadcasting the full (growing) snapshot per item was O(n²) structured-clone
  // over IPC at every boot. A 0ms timer collapses each burst into one snapshot.
  let broadcastPending: ReturnType<typeof setTimeout> | null = null
  const subUpdate = registry.onUpdate(() => {
    if (broadcastPending) return
    broadcastPending = setTimeout(() => {
      broadcastPending = null
      opts.broadcast("kamin:registry:update", registry.snapshot())
    }, 0)
  })

  // Host services are wired; from here on an uncaught error is an extension's,
  // not ours — let crash-containment contain it instead of respawning.
  opts.onHostReady?.()

  // Discover + prepare extensions (descriptors + static contributions: commands,
  // views, themes, grammars, languages). FAST — no `activate()` yet. Returning
  // here lets the child register its invoke handler + signal ready immediately,
  // so the renderer shows the extension list + contributions at once instead of
  // waiting on the slow activation pass (gitlens/gopls/volar startup).
  loader.prepareAll()
  // Fire `onLanguage:<id>` when a document of that language opens (and for any
  // already open) — language extensions like redhat.java / angular / fwcd.kotlin
  // activate on their language, not workspaceContains. Deduped: idempotent + no
  // log spam. (workspaceContains/* activation is the startup pass below.)
  const firedLangs = new Set<string>()
  const fireLang = (languageId: string): void => {
    if (languageId && languageId !== "plaintext" && !firedLangs.has(languageId)) {
      firedLangs.add(languageId)
      void loader.activateByLanguage(languageId)
    }
  }
  opts.workspaceHost.documents.onDidOpen((d) => { fireLang(d.languageId) })
  for (const d of opts.workspaceHost.documents.list()) fireLang(d.languageId)
  // Activation runs in the BACKGROUND; extensions flip to active + stream their
  // runtime contributions via registry broadcasts, and we re-announce the list
  // when the pass settles so the UI refreshes active states.
  void (async () => {
    await loader.activateStartup()
    const descriptors = loader.list()
    console.info(`KaminIDE: activated ${descriptors.filter((d) => d.active).length}/${descriptors.length} extensions`)
    for (const d of descriptors) {
      if (!d.active && d.activationError) console.error(`  ✗ ${d.id}: ${d.activationError.split("\n")[0]}`)
      else if (d.active) console.info(`  ✓ ${d.id}@${d.version}`)
      else console.info(`  ⏳ ${d.id}@${d.version} (deferred)`)
    }
    opts.broadcast("kamin:extensions:changed", { activated: true })
  })()

  const langOf = (uri: string): string => opts.workspaceHost.documents.get(uri)?.languageId ?? "plaintext"
  return {
    snapshot: () => registry.snapshot(),
    executeCommand: (id, ...args) => registry.executeCommand(id, ...args),
    listExtensions: () => loader.list(),
    setExtensionEnabled: async (id, enabled) => {
      // Close the extension's open webview panels first (they aren't in its
      // subscriptions), then unload/reload it.
      if (!enabled) webviews.disposeForExtension(id)
      await loader.setExtensionEnabled(id, enabled)
      opts.broadcast("kamin:extensions:changed", { id, enabled })
    },
    installExtension: async (extDir) => {
      const descriptor = await loader.installFromDir(extDir)
      opts.broadcast("kamin:extensions:changed", { id: descriptor.id, enabled: true })
      return descriptor
    },
    uninstallExtension: (id) => {
      // Close the extension's open webview panels (not in its subscriptions),
      // then unload it and hand back its dir for deletion.
      webviews.disposeForExtension(id)
      const dir = loader.uninstall(id)
      opts.broadcast("kamin:extensions:changed", { id, enabled: false })
      return dir
    },
    provideCompletionItems: (uri, line, character, triggerKind, triggerCharacter) =>
      languageFeatures.provideCompletionItems(uri, langOf(uri), line, character, triggerKind, triggerCharacter),
    provideHover: (uri, line, character) =>
      languageFeatures.provideHover(uri, langOf(uri), line, character),
    provideDefinition: (uri, line, character) =>
      languageFeatures.provideDefinition(uri, langOf(uri), line, character),
    provideDocumentFormattingEdits: (uri, options) =>
      languageFeatures.provideDocumentFormattingEdits(uri, langOf(uri), options),
    provideReferences: (uri, line, character, includeDeclaration) =>
      languageFeatures.provideReferences(uri, langOf(uri), line, character, includeDeclaration),
    provideDocumentHighlights: (uri, line, character) =>
      languageFeatures.provideDocumentHighlights(uri, langOf(uri), line, character),
    provideFoldingRanges: (uri) =>
      languageFeatures.provideFoldingRanges(uri, langOf(uri)),
    provideDeclaration: (uri, line, character) =>
      languageFeatures.provideDeclaration(uri, langOf(uri), line, character),
    provideTypeDefinition: (uri, line, character) =>
      languageFeatures.provideTypeDefinition(uri, langOf(uri), line, character),
    provideImplementation: (uri, line, character) =>
      languageFeatures.provideImplementation(uri, langOf(uri), line, character),
    provideSignatureHelp: (uri, line, character, triggerCharacter) =>
      languageFeatures.provideSignatureHelp(uri, langOf(uri), line, character, triggerCharacter),
    provideDocumentSymbols: (uri) => languageFeatures.provideDocumentSymbols(uri, langOf(uri)),
    provideDocumentLinks: (uri) => languageFeatures.provideDocumentLinks(uri, langOf(uri)),
    provideInlayHints: (uri, sl, sc, el, ec) => languageFeatures.provideInlayHints(uri, langOf(uri), sl, sc, el, ec),
    provideSelectionRanges: (uri, positions) => languageFeatures.provideSelectionRanges(uri, langOf(uri), positions),
    provideCodeLenses: (uri) => languageFeatures.provideCodeLenses(uri, langOf(uri)),
    provideDocumentColors: (uri) => languageFeatures.provideDocumentColors(uri, langOf(uri)),
    provideColorPresentations: (uri, color, range) => languageFeatures.provideColorPresentations(uri, langOf(uri), color, range),
    provideRename: (uri, line, character, newName) => languageFeatures.provideRename(uri, langOf(uri), line, character, newName),
    provideCodeActions: (uri, range) => languageFeatures.provideCodeActions(uri, langOf(uri), range),
    provideDocumentSemanticTokens: (uri) => languageFeatures.provideDocumentSemanticTokens(uri, langOf(uri)),
    provideWorkspaceSymbols: (query) => languageFeatures.provideWorkspaceSymbols(query),
    webviewMessage: (id, msg) => { webviews.deliverMessage(id, msg) },
    webviewViewState: (id, active, visible) => { webviews.updateViewState(id, active, visible) },
    webviewDisposed: (id) => { webviews.disposeFromRenderer(id) },
    resolveWebviewView: (viewId) => { webviews.resolveView(viewId) },
    restoreWebviews: () => { webviews.restore() },
    webviewPersistState: (id, state) => { webviews.persistPanelState(id, state) },
    treeGetChildren: (viewId, handle) => treeViews.getChildren(viewId, handle),
    treeReportSelection: (viewId, handles) => { treeViews.reportSelection(viewId, handles) },
    treeReportExpansion: (viewId, handle, expanded) => { treeViews.reportExpansion(viewId, handle, expanded) },
    treeReportVisibility: (viewId, visible) => { treeViews.reportVisibility(viewId, visible) },
    treeReportCheckbox: (viewId, handle, state) => { treeViews.reportCheckbox(viewId, handle, state) },
    treeHasDnd: (viewId) => treeViews.hasDnd(viewId),
    treeHandleDrag: (viewId, sourceHandles) => treeViews.handleDrag(viewId, sourceHandles),
    treeHandleDrop: (viewId, targetHandle) => treeViews.handleDrop(viewId, targetHandle),
    treeGetMeta: (viewId) => treeViews.getMeta(viewId),
    fileDecoration: (fsPath) => fileDecorations.provide(fsPath),
    statusBarSnapshot: () => statusBar.snapshot(),
    diagnosticsSnapshot: () => diagnostics.snapshotDtos(),
    dispose() {
      subUpdate.dispose()
      sessionsApi.dispose()
      loader.unloadAll()
    },
  }
}

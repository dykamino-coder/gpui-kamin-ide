// Data namespaces — languages, env, extensions, debug, tasks, auth, l10n.
// `workspace` moved to ns-workspace.ts (it grew real in B1). Each builder
// takes hooks + extensionId (unused for some) and returns the namespace.
import { BUILTIN_EXTENSION_FACADES } from "./builtin-extensions.js"
import type { NsHooks } from "./ns-builders.js"
import type { DocumentsApi } from "./ns-documents.js"
import { scoreSelector } from "./selector-score.js"
import { Disposable, noopEvent } from "./shared.js"

export function buildLanguages(h: NsHooks, docs: DocumentsApi) {
  return {
    // vscode.languages.getLanguages() — the ids Monaco/host actually know:
    // the always-present base set plus every `contributes.languages` id the
    // registry has collected. Deduped, order-stable.
    getLanguages: () => Promise.resolve([
      ...new Set<string>([
        "plaintext", "json", "jsonc", "markdown", "javascript", "typescript",
        ...h.registry.snapshot().languages.map((l) => l.id),
      ]),
    ]),
    setLanguageConfiguration: () => new Disposable(() => {}),
    setTextDocumentLanguage: <T>(d: T) => Promise.resolve(d),
    // vscode.languages.match — vscode-languageclient gates didOpen/didChange on
    // this score; a stub `0` silently breaks document sync for EVERY LSP-backed
    // extension (gopls, clangd, jdt.ls, rust-analyzer, volar) so the server
    // never receives the file and returns nothing.
    match: (selector: unknown, document: unknown) => {
      const doc = document as { languageId?: unknown; uri?: { scheme?: unknown } }
      const lang = typeof doc.languageId === "string" ? doc.languageId : ""
      const scheme = typeof doc.uri?.scheme === "string" ? doc.uri.scheme : "file"
      return scoreSelector(selector as never, lang, scheme)
    },
    // B6: completion providers are real — registered into the shared
    // host-wide LanguageFeatures registry the renderer's Monaco queries.
    // reason: the extension-facing `provider` is untyped at this API boundary;
    // `query()` guards each call (matchesSelector + try/catch) at runtime.
    registerCompletionItemProvider: (selector: unknown, provider: unknown) =>
      h.languageFeatures.registerCompletionItemProvider(selector as never, provider as never, docs.docFor),
    registerDefinitionProvider: (selector: unknown, provider: unknown) =>
      h.languageFeatures.registerDefinitionProvider(selector as never, provider as never, docs.docFor),
    registerImplementationProvider: (selector: unknown, provider: unknown) =>
      h.languageFeatures.registerImplementationProvider(selector as never, provider as never, docs.docFor),
    registerTypeDefinitionProvider: (selector: unknown, provider: unknown) =>
      h.languageFeatures.registerTypeDefinitionProvider(selector as never, provider as never, docs.docFor),
    registerDeclarationProvider: (selector: unknown, provider: unknown) =>
      h.languageFeatures.registerDeclarationProvider(selector as never, provider as never, docs.docFor),
    registerHoverProvider: (selector: unknown, provider: unknown) =>
      h.languageFeatures.registerHoverProvider(selector as never, provider as never, docs.docFor),
    registerEvaluatableExpressionProvider: () => new Disposable(() => {}),
    registerInlineValuesProvider: () => new Disposable(() => {}),
    registerDocumentHighlightProvider: (selector: unknown, provider: unknown) =>
      h.languageFeatures.registerDocumentHighlightProvider(selector as never, provider as never, docs.docFor),
    registerDocumentSymbolProvider: (selector: unknown, provider: unknown) =>
      h.languageFeatures.registerDocumentSymbolProvider(selector as never, provider as never, docs.docFor),
    registerWorkspaceSymbolProvider: (provider: unknown) =>
      h.languageFeatures.registerWorkspaceSymbolProvider(provider as never),
    registerReferenceProvider: (selector: unknown, provider: unknown) =>
      h.languageFeatures.registerReferenceProvider(selector as never, provider as never, docs.docFor),
    registerRenameProvider: (selector: unknown, provider: unknown) =>
      h.languageFeatures.registerRenameProvider(selector as never, provider as never, docs.docFor),
    registerDocumentSemanticTokensProvider: (selector: unknown, provider: unknown, legend: unknown) =>
      h.languageFeatures.registerDocumentSemanticTokensProvider(selector as never, provider as never, legend as never, docs.docFor),
    registerDocumentRangeSemanticTokensProvider: () => new Disposable(() => {}),
    registerDocumentFormattingEditProvider: (selector: unknown, provider: unknown) =>
      h.languageFeatures.registerDocumentFormattingEditProvider(selector as never, provider as never, docs.docFor),
    registerDocumentRangeFormattingEditProvider: () => new Disposable(() => {}),
    registerOnTypeFormattingEditProvider: () => new Disposable(() => {}),
    registerSignatureHelpProvider: (selector: unknown, provider: unknown) =>
      h.languageFeatures.registerSignatureHelpProvider(selector as never, provider as never, docs.docFor),
    registerCodeActionsProvider: (selector: unknown, provider: unknown) =>
      h.languageFeatures.registerCodeActionsProvider(selector as never, provider as never, docs.docFor),
    registerCodeLensProvider: (selector: unknown, provider: unknown) =>
      h.languageFeatures.registerCodeLensProvider(selector as never, provider as never, docs.docFor),
    registerDocumentLinkProvider: (selector: unknown, provider: unknown) =>
      h.languageFeatures.registerDocumentLinkProvider(selector as never, provider as never, docs.docFor),
    registerColorProvider: (selector: unknown, provider: unknown) =>
      h.languageFeatures.registerColorProvider(selector as never, provider as never, docs.docFor),
    registerInlayHintsProvider: (selector: unknown, provider: unknown) =>
      h.languageFeatures.registerInlayHintsProvider(selector as never, provider as never, docs.docFor),
    registerFoldingRangeProvider: (selector: unknown, provider: unknown) =>
      h.languageFeatures.registerFoldingRangeProvider(selector as never, provider as never, docs.docFor),
    registerSelectionRangeProvider: (selector: unknown, provider: unknown) =>
      h.languageFeatures.registerSelectionRangeProvider(selector as never, provider as never, docs.docFor),
    registerCallHierarchyProvider: () => new Disposable(() => {}),
    registerTypeHierarchyProvider: () => new Disposable(() => {}),
    registerLinkedEditingRangeProvider: () => new Disposable(() => {}),
    registerDocumentDropEditProvider: () => new Disposable(() => {}),
    registerDocumentPasteEditProvider: () => new Disposable(() => {}),
    registerInlineCompletionItemProvider: () => new Disposable(() => {}),
    createLanguageStatusItem: () => ({ dispose() {} }),
    // B6c: diagnostics are real — collections push markers to the renderer's
    // Monaco via the shared host-wide registry; getDiagnostics aggregates.
    createDiagnosticCollection: (name?: string) => h.diagnostics.createCollection(name),
    // reason: `uri` is an untyped vscode.Uri at this boundary; the registry's
    // keyOf() reads `.fsPath`/`.toString()` defensively.
    getDiagnostics: (uri?: unknown) =>
      uri === undefined ? h.diagnostics.getDiagnostics() : h.diagnostics.getDiagnostics(uri as never),
    onDidChangeDiagnostics: h.diagnostics.onDidChangeDiagnostics,
  }
}

export function buildEnv(h: NsHooks) {
  return {
    appName: "KaminIDE", appHost: "desktop", appRoot: "", language: "en",
    machineId: "kamin-ide-machine", sessionId: "kamin-ide-session", uriScheme: "kamin-ide",
    shell: process.env.SHELL ?? (process.platform === "win32" ? "powershell.exe" : "/bin/bash"),
    uiKind: 1, remoteName: undefined, isTelemetryEnabled: false, isNewAppInstall: false, logLevel: 3,
    onDidChangeTelemetryEnabled: noopEvent, onDidChangeShell: noopEvent, onDidChangeLogLevel: noopEvent,
    clipboard: {
      readText: () => h.readClipboard(),
      // `unknown`, not `string`: this is the extension-facing boundary, so the
      // value is whatever was actually passed — typing it `string` made the
      // guard look redundant while it was the only thing handling a non-string.
      writeText: (text: unknown) => { h.writeClipboard(typeof text === "string" ? text : ""); return Promise.resolve() },
    },
    openExternal: (uri: unknown) => {
      // A vscode Uri stringifies to its external form (http/https/mailto keep the
      // scheme; file → file://). The renderer/host shell-open handles all of them.
      // Only a string or a Uri-like with its OWN toString is usable — a bare
      // object would fall through Object.prototype.toString to "[object Object]"
      // and we'd hand the shell that literal garbage.
      const target = typeof uri === "string" ? uri : (uri as { toString?: () => string } | null)?.toString?.() ?? ""
      return target && target !== "[object Object]" ? h.openExternal(target) : Promise.resolve(false)
    },
    asExternalUri: <T>(uri: T) => Promise.resolve(uri),
    createTelemetryLogger: () => ({ logUsage() { /**/ }, logError() { /**/ }, dispose() { /**/ }, onDidChangeEnableStates: noopEvent }),
  }
}

export function buildExtensions(h: NsHooks) {
  return {
    // Installed extensions + built-in facades (#22); a real extension with the
    // same id always wins over its builtin facade.
    get all() {
      const real = h.listExtensions()
      const realIds = new Set(real.map((e) => e.id))
      return [...real, ...BUILTIN_EXTENSION_FACADES.filter((e) => !realIds.has(e.id))]
    },
    getExtension(id: string) {
      return h.listExtensions().find((e) => e.id === id) ?? BUILTIN_EXTENSION_FACADES.find((e) => e.id === id)
    },
    // Fires on install/uninstall/enable/disable. Stubbed never-firing for now —
    // extensions (ms-toolsai.jupyter) subscribe at activation and need it to be
    // a function; live firing can be wired to the registry later.
    onDidChange: noopEvent,
  }
}

export function buildDebug() {
  return {
    activeDebugSession: undefined, activeDebugConsole: { append() {}, appendLine() {} },
    breakpoints: [] as unknown[],
    registerDebugConfigurationProvider: () => new Disposable(() => {}),
    registerDebugAdapterDescriptorFactory: () => new Disposable(() => {}),
    registerDebugAdapterTrackerFactory: () => new Disposable(() => {}),
    startDebugging: () => Promise.resolve(false),
    stopDebugging: () => Promise.resolve(),
    addBreakpoints: () => {}, removeBreakpoints: () => {},
    onDidChangeActiveDebugSession: noopEvent, onDidChangeBreakpoints: noopEvent,
    onDidStartDebugSession: noopEvent, onDidTerminateDebugSession: noopEvent,
    onDidReceiveDebugSessionCustomEvent: noopEvent,
  }
}

export function buildTasks() {
  return {
    taskExecutions: [] as unknown[],
    registerTaskProvider: () => new Disposable(() => {}),
    fetchTasks: () => Promise.resolve([] as unknown[]),
    executeTask: () => Promise.resolve({ task: undefined, terminate() {} }),
    onDidStartTask: noopEvent, onDidEndTask: noopEvent,
    onDidStartTaskProcess: noopEvent, onDidEndTaskProcess: noopEvent,
  }
}

export function buildAuthentication() {
  return {
    getSession: () => Promise.resolve(undefined),
    getAccounts: () => Promise.resolve([] as unknown[]),
    registerAuthenticationProvider: () => new Disposable(() => {}),
    onDidChangeSessions: noopEvent,
  }
}


// Renderer-facing method table for the ext-host surface, in the PARENT. Every
// entry routes into the forked child via `exthost.invoke(...)` (the child holds
// the registry/loader/webviews/trees/diagnostics). The few that wrap child
// calls with parent-side fs work (vsix install/uninstall/icon) and the
// parent-owned theme reader live here too. The renderer's doc/editor sync is
// forwarded 1:1 (the child owns documents/editors under crash isolation).
import type { ExtensionDescriptor } from "../../api/types.js"
import type { Handler } from "../services/index.js"
import * as themeSvc from "../services/theme.js"
import * as vsix from "../services/vsix.js"
import type { ExtHostHandle } from "./parent.js"

export function exthostMethods(exthost: ExtHostHandle, userExtDir: string): [string, Handler][] {
  const invoke = exthost.invoke
  return [
    ["kamin:registry:snapshot", () => invoke("snapshot")],
    // Raw args — the child revives marshalled Uris AFTER the IPC hop (a real
    // Uri instance would not survive structured clone).
    ["kamin:command:execute", (id, ...args) => invoke("executeCommand", id, ...args)],
    ["kamin:extensions:list", () => invoke("listExtensions")],
    ["kamin:extensions:setEnabled", (id, enabled) => invoke("setExtensionEnabled", id, enabled)],
    ["kamin:extensions:installVsix", async (p) => {
      const { dir } = await vsix.installVsixArchive(p as string, userExtDir)
      return await invoke("installExtension", dir)
    }],
    ["kamin:extensions:uninstall", async (id) => {
      const dir = await invoke<string>("uninstallExtension", id)
      await vsix.removeExtensionDir(dir, userExtDir)
      return { id }
    }],
    ["kamin:extensions:icon", async (id) => {
      const list = await invoke<ExtensionDescriptor[]>("listExtensions")
      const d = list.find((x) => x.id === id)
      const icon = (d?.packageJSON as { icon?: unknown } | undefined)?.icon
      return d && typeof icon === "string" ? await vsix.readExtensionIcon(d.extensionPath, icon) : null
    }],
    ["kamin:lang:completion", (uri, line, character, triggerKind, triggerCharacter) =>
      invoke("provideCompletionItems", uri, line, character, triggerKind, triggerCharacter)],
    ["kamin:lang:hover", (uri, line, character) => invoke("provideHover", uri, line, character)],
    ["kamin:lang:definition", (uri, line, character) => invoke("provideDefinition", uri, line, character)],
    ["kamin:lang:formatting", (uri, options) => invoke("provideDocumentFormattingEdits", uri, options)],
    ["kamin:lang:references", (uri, line, character, includeDeclaration) => invoke("provideReferences", uri, line, character, includeDeclaration)],
    ["kamin:lang:documentHighlight", (uri, line, character) => invoke("provideDocumentHighlights", uri, line, character)],
    ["kamin:lang:foldingRange", (uri) => invoke("provideFoldingRanges", uri)],
    ["kamin:lang:declaration", (uri, line, character) => invoke("provideDeclaration", uri, line, character)],
    ["kamin:lang:typeDefinition", (uri, line, character) => invoke("provideTypeDefinition", uri, line, character)],
    ["kamin:lang:implementation", (uri, line, character) => invoke("provideImplementation", uri, line, character)],
    ["kamin:lang:signatureHelp", (uri, line, character, triggerCharacter) => invoke("provideSignatureHelp", uri, line, character, triggerCharacter)],
    ["kamin:lang:documentSymbol", (uri) => invoke("provideDocumentSymbols", uri)],
    ["kamin:lang:documentLink", (uri) => invoke("provideDocumentLinks", uri)],
    ["kamin:lang:inlayHints", (uri, sl, sc, el, ec) => invoke("provideInlayHints", uri, sl, sc, el, ec)],
    ["kamin:lang:selectionRange", (uri, positions) => invoke("provideSelectionRanges", uri, positions)],
    ["kamin:lang:codeLens", (uri) => invoke("provideCodeLenses", uri)],
    ["kamin:lang:documentColor", (uri) => invoke("provideDocumentColors", uri)],
    ["kamin:lang:colorPresentations", (uri, color, range) => invoke("provideColorPresentations", uri, color, range)],
    ["kamin:lang:rename", (uri, line, character, newName) => invoke("provideRename", uri, line, character, newName)],
    ["kamin:lang:codeAction", (uri, range) => invoke("provideCodeActions", uri, range)],
    ["kamin:lang:semanticTokens", (uri) => invoke("provideDocumentSemanticTokens", uri)],
    ["kamin:lang:workspaceSymbol", (query) => invoke("provideWorkspaceSymbols", query)],
    ["kamin:webview:inbound", (id, msg) => invoke("webviewMessage", id, msg)],
    ["kamin:webview:viewState", (id, active, visible) => invoke("webviewViewState", id, active, visible)],
    ["kamin:webview:closed", (id) => invoke("webviewDisposed", id)],
    ["kamin:webviewView:resolve", (viewId) => invoke("resolveWebviewView", viewId)],
    ["kamin:webview:restore", () => invoke("restoreWebviews")],
    ["kamin:webview:persistState", (id, state) => invoke("webviewPersistState", id, state)],
    ["kamin:tree:getChildren", (viewId, handle) => invoke("treeGetChildren", viewId, handle)],
    ["kamin:tree:reportSelection", (viewId, handles) => invoke("treeReportSelection", viewId, handles)],
    ["kamin:tree:reportExpansion", (viewId, handle, expanded) => invoke("treeReportExpansion", viewId, handle, expanded)],
    ["kamin:tree:reportVisibility", (viewId, visible) => invoke("treeReportVisibility", viewId, visible)],
    ["kamin:tree:reportCheckbox", (viewId, handle, state) => invoke("treeReportCheckbox", viewId, handle, state)],
    ["kamin:tree:hasDnd", (viewId) => invoke("treeHasDnd", viewId)],
    ["kamin:tree:handleDrag", (viewId, handles) => invoke("treeHandleDrag", viewId, handles)],
    ["kamin:tree:handleDrop", (viewId, target) => invoke("treeHandleDrop", viewId, target)],
    ["kamin:tree:getMeta", (viewId) => invoke("treeGetMeta", viewId)],
    ["kamin:fileDecoration:get", (fsPath) => invoke("fileDecoration", fsPath)],
    ["kamin:statusBar:snapshot", () => invoke("statusBarSnapshot")],
    ["kamin:diag:snapshot", () => invoke("diagnosticsSnapshot")],
    // Renderer Monaco doc/editor sync forwarded to the child (owns documents/editors).
    ["kamin:doc:open", (doc) => invoke("doc:open", doc)],
    ["kamin:doc:change", (uri, changes, version) => invoke("doc:change", uri, changes, version)],
    ["kamin:doc:setLanguage", (uri, lang) => invoke("doc:setLanguage", uri, lang)],
    ["kamin:doc:close", (uri) => invoke("doc:close", uri)],
    ["kamin:doc:save", (uri) => invoke("doc:save", uri)],
    ["kamin:editor:active", (uri) => invoke("editor:active", uri)],
    ["kamin:editor:selections", (uri, sels) => invoke("editor:selections", uri, sels)],
    // Parent-owned: theme JSON reader (fs, not an extension concern).
    ["kamin:theme:read", (path) => themeSvc.readThemeFile(path as string)],
  ]
}

// Dispatch table for CHILD_INVOKE — every renderer-facing call the parent
// routes into the child. Keys are the ExtHost method names plus the Monaco
// doc/editor sync channels; the parent maps its renderer channel names onto
// these (see exthost-bridge/parent.ts INVOKE_ROUTES).
import { join as joinPath } from "node:path"
import type { ExtHost } from "../../exthost/index.js"
import * as documents from "../services/documents.js"
import * as editors from "../services/editors.js"

type Fn = (...params: unknown[]) => unknown

export function buildInvokeTable(host: ExtHost, revive: (arg: unknown) => unknown): Record<string, Fn> {
  const s = (v: unknown) => v as string
  const n = (v: unknown) => v as number
  return {
    // ── диагностика памяти (#74) — зовётся родителем из kamin:diag:* ──
    "diag:memory": () => ({ pid: process.pid, ...process.memoryUsage() }),
    "diag:heapSnapshot": async (dir) =>
      (await import("node:v8")).writeHeapSnapshot(joinPath(s(dir), `exthost-${process.pid}.heapsnapshot`)),
    // ── registry / commands / extensions ──
    snapshot: () => host.snapshot(),
    executeCommand: (id, ...args) => host.executeCommand(s(id), ...args.map(revive)),
    listExtensions: () => host.listExtensions(),
    setExtensionEnabled: (id, enabled) => host.setExtensionEnabled(s(id), enabled as boolean),
    installExtension: (extDir) => host.installExtension(s(extDir)),
    uninstallExtension: (id) => host.uninstallExtension(s(id)),
    // ── language features (B6) ──
    provideCompletionItems: (uri, line, char, tk, tc) =>
      host.provideCompletionItems(s(uri), n(line), n(char), tk as number | undefined, tc as string | undefined),
    provideHover: (uri, line, char) => host.provideHover(s(uri), n(line), n(char)),
    provideDefinition: (uri, line, char) => host.provideDefinition(s(uri), n(line), n(char)),
    provideDocumentFormattingEdits: (uri, options) => host.provideDocumentFormattingEdits(s(uri), options),
    provideReferences: (uri, line, char, incl) => host.provideReferences(s(uri), n(line), n(char), incl as boolean),
    provideDocumentHighlights: (uri, line, char) => host.provideDocumentHighlights(s(uri), n(line), n(char)),
    provideFoldingRanges: (uri) => host.provideFoldingRanges(s(uri)),
    provideDeclaration: (uri, line, char) => host.provideDeclaration(s(uri), n(line), n(char)),
    provideTypeDefinition: (uri, line, char) => host.provideTypeDefinition(s(uri), n(line), n(char)),
    provideImplementation: (uri, line, char) => host.provideImplementation(s(uri), n(line), n(char)),
    provideSignatureHelp: (uri, line, char, tc) => host.provideSignatureHelp(s(uri), n(line), n(char), tc as string | undefined),
    provideDocumentSymbols: (uri) => host.provideDocumentSymbols(s(uri)),
    provideDocumentLinks: (uri) => host.provideDocumentLinks(s(uri)),
    provideInlayHints: (uri, sl, sc, el, ec) => host.provideInlayHints(s(uri), n(sl), n(sc), n(el), n(ec)),
    provideSelectionRanges: (uri, positions) => host.provideSelectionRanges(s(uri), positions as { line: number; character: number }[]),
    provideCodeLenses: (uri) => host.provideCodeLenses(s(uri)),
    provideDocumentColors: (uri) => host.provideDocumentColors(s(uri)),
    provideColorPresentations: (uri, color, range) => host.provideColorPresentations(s(uri), color as never, range as never),
    provideRename: (uri, line, char, newName) => host.provideRename(s(uri), n(line), n(char), s(newName)),
    provideCodeActions: (uri, range) => host.provideCodeActions(s(uri), range as never),
    provideDocumentSemanticTokens: (uri) => host.provideDocumentSemanticTokens(s(uri)),
    provideWorkspaceSymbols: (query) => host.provideWorkspaceSymbols(s(query)),
    // ── webviews (B7) ──
    webviewMessage: (id, msg) => { host.webviewMessage(s(id), msg) },
    webviewViewState: (id, active, visible) => { host.webviewViewState(s(id), active as boolean, visible as boolean) },
    webviewDisposed: (id) => { host.webviewDisposed(s(id)) },
    resolveWebviewView: (viewId) => { host.resolveWebviewView(s(viewId)) },
    restoreWebviews: () => { host.restoreWebviews() },
    webviewPersistState: (id, state) => { host.webviewPersistState(s(id), state) },
    // ── tree views (B7b/B10) ──
    treeGetChildren: (viewId, handle) => host.treeGetChildren(s(viewId), handle as string | undefined),
    treeReportSelection: (viewId, handles) => { host.treeReportSelection(s(viewId), handles as string[]) },
    treeReportExpansion: (viewId, handle, expanded) => { host.treeReportExpansion(s(viewId), s(handle), expanded as boolean) },
    treeReportVisibility: (viewId, visible) => { host.treeReportVisibility(s(viewId), visible as boolean) },
    treeReportCheckbox: (viewId, handle, state) => { host.treeReportCheckbox(s(viewId), s(handle), n(state)) },
    treeHasDnd: (viewId) => host.treeHasDnd(s(viewId)),
    treeHandleDrag: (viewId, handles) => host.treeHandleDrag(s(viewId), handles as string[]),
    treeHandleDrop: (viewId, target) => host.treeHandleDrop(s(viewId), target as string | null),
    treeGetMeta: (viewId) => host.treeGetMeta(s(viewId)),
    // ── decorations / status bar / diagnostics ──
    fileDecoration: (fsPath) => host.fileDecoration(s(fsPath)),
    statusBarSnapshot: () => host.statusBarSnapshot(),
    diagnosticsSnapshot: () => host.diagnosticsSnapshot(),
    // ── renderer Monaco doc/editor sync (B5/B5b) — fed straight to the
    //    in-child mirror services so textDocuments/activeTextEditor track ──
    "doc:open": (doc) => { documents.syncDocOpen(doc as documents.DocState) },
    "doc:change": (uri, changes, version) => { documents.syncDocChange(s(uri), changes as documents.DocChange[], n(version)) },
    "doc:setLanguage": (uri, lang) => { documents.syncDocSetLanguage(s(uri), s(lang)) },
    "doc:close": (uri) => { documents.syncDocClose(s(uri)); editors.dropEditor(s(uri)) },
    "doc:save": (uri) => { documents.syncDocSave(s(uri)) },
    "editor:active": (uri) => { editors.setActiveEditor(uri as string | null) },
    "editor:selections": (uri, sels) => { editors.setEditorSelections(s(uri), sels as editors.HostSelection[]) },
  }
}

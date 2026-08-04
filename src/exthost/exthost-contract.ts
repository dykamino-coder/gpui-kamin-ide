// Boot contract for the ext-host (extracted from index.ts, which had outgrown
// the 250-line ceiling carrying both the interface and the whole boot body).
// Types only — no runtime. index.ts re-exports ExtHost, so importers are unchanged.
import type { Disposable, ExtensionDescriptor, QuickPickItemDto, QuickPickOptionsDto, OpenDialogOptionsDto, SaveDialogOptionsDto, RegistrySnapshot, TreeNodeDto, TreeViewMetaDto, FileDecorationDto } from "../api/types.js"
import type { DiagnosticDto } from "./api/diagnostics.js"
import type { CodeActionDto, CodeLensDto, ColorDto, ColorInformationDto, ColorPresentationDto, CompletionResultDto, DocumentHighlightDto, DocumentLinkDto, DocumentSymbolDto, FoldingRangeDto, HoverDto, InlayHintDto, LocationDto, SemanticTokensDto, SignatureHelpDto, TextEditDto, WorkspaceEditDto, WorkspaceSymbolDto } from "./api/language-features.js"
import type { RangeDto } from "./api/range-dto.js"
import type { StatusBarItemState } from "./api/status-bar.js"
import type { WebviewStore } from "./api/webview.js"
import type { EnvHost, SessionsHost, StorageHost, WorkspaceHost } from "./host-services.js"

export interface StartOptions {
  builtinDir: string
  /** Writable dir for sideloaded (.vsix / folder) extensions. */
  userExtDir: string
  broadcast: (channel: string, payload: unknown) => void
  /** Called once the host's own services are up but BEFORE extensions activate,
   *  so the crash-containment can stop escalating an extension's async error
   *  (during its activate()) into a respawn. Only OUR pre-activation failures
   *  should respawn; an extension that throws is contained, not fatal. */
  onHostReady?: () => void
  /** Promise-returning UX hooks. Host implements them by routing
   *  through IPC into the renderer's overlay system. We require them
   *  rather than synthesising fallbacks so a missing wiring fails
   *  loud instead of silently swallowing extension prompts. */
  showMessage: (severity: "info" | "warning" | "error", message: string, items: string[]) => Promise<string | undefined>
  showInputBox: (opts: { prompt?: string; placeHolder?: string; value?: string }) => Promise<string | undefined>
  /** `vscode.window.showQuickPick` — returns the chosen item INDICES (so the
   *  caller maps back to the original string/QuickPickItem), or null on cancel. */
  showQuickPick: (items: QuickPickItemDto[], options: QuickPickOptionsDto) => Promise<number[] | null>
  /** `vscode.window.showOpenDialog` — renderer opens the native file dialog and
   *  returns chosen absolute paths, or null on cancel. */
  showOpenDialog: (options: OpenDialogOptionsDto) => Promise<string[] | null>
  /** `vscode.window.showSaveDialog` — renderer opens the native save dialog and
   *  returns the chosen absolute path, or null on cancel. */
  showSaveDialog: (options: SaveDialogOptionsDto) => Promise<string | null>
  /** `vscode.env.openExternal(uri)` — renderer opens the target in the OS default
   *  app (routes to the host shell-open); resolves the success flag. */
  openExternal: (target: string) => Promise<boolean>
  /** `vscode.env.clipboard.readText` — renderer reads the system clipboard. */
  readClipboard: () => Promise<string>
  emitOutputEvent: (event: { channel: string; op: "append" | "replace" | "clear" | "dispose" | "show"; text?: string; extensionId: string }) => void
  /** Host workspace surface (folder + fs) backing `vscode.workspace` (B1). */
  workspaceHost: WorkspaceHost
  /** Extension persistence backing globalState/workspaceState/secrets (B9). */
  storage: StorageHost
  /** environmentVariableCollection sink — applied to integrated-terminal spawns (#11). */
  env: EnvHost
  /** Disk persistence of open webview panels (WebviewPanelSerializer restore). */
  webviewStore: WebviewStore
  /** Projects + Sessions store backing `require('kaminide')` (Phase 3). */
  sessionsHost: SessionsHost
}

export interface ExtHost extends Disposable {
  snapshot(): RegistrySnapshot
  executeCommand(id: string, ...args: unknown[]): Promise<unknown>
  listExtensions(): ExtensionDescriptor[]
  /** Enable/disable an extension at runtime (no restart); persists + broadcasts. */
  setExtensionEnabled(id: string, enabled: boolean): Promise<void>
  /** Install (or reinstall) a sideloaded extension already extracted at `extDir`,
   *  live. Returns its descriptor. */
  installExtension(extDir: string): Promise<ExtensionDescriptor>
  /** Uninstall a sideloaded extension live; returns its on-disk dir to delete. */
  uninstallExtension(id: string): string
  /** B6: language-feature queries from all matching providers (renderer Monaco
   *  calls these over the WS). languageId is resolved from the doc mirror. */
  provideCompletionItems(uri: string, line: number, character: number, triggerKind?: number, triggerCharacter?: string): Promise<CompletionResultDto>
  provideHover(uri: string, line: number, character: number): Promise<HoverDto[]>
  provideDefinition(uri: string, line: number, character: number): Promise<LocationDto[]>
  /** B6d: document formatting — renderer Monaco "Format Document" → first
   *  matching extension formatter's edits. */
  provideDocumentFormattingEdits(uri: string, options: unknown): Promise<TextEditDto[]>
  /** B6e: find-all-references / document-highlight / folding — all matching
   *  providers merged (renderer Monaco peek / occurrence highlight / gutter). */
  provideReferences(uri: string, line: number, character: number, includeDeclaration: boolean): Promise<LocationDto[]>
  provideDocumentHighlights(uri: string, line: number, character: number): Promise<DocumentHighlightDto[]>
  provideFoldingRanges(uri: string): Promise<FoldingRangeDto[]>
  /** B6f: declaration / type-definition / implementation — all Location-based,
   *  merged across providers (renderer Monaco Go to Declaration/Type Def/Impl). */
  provideDeclaration(uri: string, line: number, character: number): Promise<LocationDto[]>
  provideTypeDefinition(uri: string, line: number, character: number): Promise<LocationDto[]>
  provideImplementation(uri: string, line: number, character: number): Promise<LocationDto[]>
  /** B6g: signature help (parameter hints popup). */
  provideSignatureHelp(uri: string, line: number, character: number, triggerCharacter?: string): Promise<SignatureHelpDto | null>
  /** B6h: document symbols (outline / Go to Symbol) + document links. */
  provideDocumentSymbols(uri: string): Promise<DocumentSymbolDto[]>
  provideDocumentLinks(uri: string): Promise<DocumentLinkDto[]>
  /** B6i: inlay hints (range-scoped) + selection ranges (per cursor position). */
  provideInlayHints(uri: string, startLine: number, startChar: number, endLine: number, endChar: number): Promise<InlayHintDto[]>
  provideSelectionRanges(uri: string, positions: { line: number; character: number }[]): Promise<RangeDto[][]>
  /** B6j: code lenses (clickable annotations above lines). */
  provideCodeLenses(uri: string): Promise<CodeLensDto[]>
  /** B6k: document colors (swatches) + the picker's format presentations. */
  provideDocumentColors(uri: string): Promise<ColorInformationDto[]>
  provideColorPresentations(uri: string, color: ColorDto, range: RangeDto): Promise<ColorPresentationDto[]>
  /** B6l: rename → a workspace edit the renderer applies. */
  provideRename(uri: string, line: number, character: number, newName: string): Promise<WorkspaceEditDto | null>
  /** B6m: code actions (lightbulb) over a range. */
  provideCodeActions(uri: string, range: RangeDto): Promise<CodeActionDto[]>
  /** B6n: semantic tokens (remapped to the standard legend). */
  provideDocumentSemanticTokens(uri: string): Promise<SemanticTokensDto | null>
  /** Workspace symbols (Go to Symbol in Workspace, Ctrl+T) — query, not per-doc. */
  provideWorkspaceSymbols(query: string): Promise<WorkspaceSymbolDto[]>
  /** B7a: inbound webview events relayed from the renderer's iframes. */
  webviewMessage(id: string, msg: unknown): void
  webviewViewState(id: string, active: boolean, visible: boolean): void
  webviewDisposed(id: string): void
  /** B7b: renderer asks to resolve a webview view (its sidebar body mounted). */
  resolveWebviewView(viewId: string): void
  /** Re-create + deserialize webview panels saved from a prior run (once, on the
   *  first renderer connect so the create broadcasts reach a live renderer). */
  restoreWebviews(): void
  /** Renderer reports a panel's latest webview state → persist for restore. */
  webviewPersistState(id: string, state: unknown): void
  /** Tree views: renderer pulls a node's children (root when handle omitted),
   *  and reports user interaction back for createTreeView events. */
  treeGetChildren(viewId: string, handle?: string): Promise<TreeNodeDto[]>
  treeReportSelection(viewId: string, handles: string[]): void
  treeReportExpansion(viewId: string, handle: string, expanded: boolean): void
  treeReportVisibility(viewId: string, visible: boolean): void
  treeReportCheckbox(viewId: string, handle: string, state: number): void
  /** Drag-and-drop: handleDrag/handleDrop run host-side; renderer sends handles. */
  treeHasDnd(viewId: string): boolean
  treeHandleDrag(viewId: string, sourceHandles: string[]): Promise<void>
  treeHandleDrop(viewId: string, targetHandle: string | null): Promise<void>
  /** createTreeView display props (message/title/description/badge) — pulled on mount. */
  treeGetMeta(viewId: string): TreeViewMetaDto
  /** FileDecorationProvider: decoration for one fs path (file tree rows). */
  fileDecoration(fsPath: string): Promise<FileDecorationDto | null>
  /** B8: current status-bar items, pulled by a renderer on (re)connect. */
  statusBarSnapshot(): StatusBarItemState[]
  /** Problems panel: every diagnostic currently held, pulled on (re)connect
   *  ( `kamin:diag:set` only carries deltas). */
  diagnosticsSnapshot(): { owner: string; uri: string; diagnostics: DiagnosticDto[] }[]
}

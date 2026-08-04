// Per-extension namespace builders. Each takes the hook bag (registry,
// notification emitter, extensions accessor) and the calling extension's
// id, and returns the bound namespace object that goes onto `vscode.X`.
//
// Why per-extension instead of singletons: `commands.registerCommand`
// wants to record `source: extensionId` in the registry, so the
// namespace closure captures `extensionId`. Other namespaces (window,
// workspace, languages…) currently don't need the id, but we shape
// them the same way for symmetry — Phase B will need it for things
// like `extensionContributesView` checks.
import type { QuickPickItemDto, QuickPickOptionsDto, OpenDialogOptionsDto, SaveDialogOptionsDto } from "../../api/types.js"
import type { DecorationRenderOptionsDto, WorkspaceHost } from "../host-services.js"
import type { Registry } from "../registry.js"
import { createDecorationKey, disposeDecorationKey } from "./decoration-registry.js"
import type { Diagnostics } from "./diagnostics.js"
import type { FileDecorations, FileDecorationProviderLike } from "./file-decorations.js"
import type { LanguageFeatures } from "./language-features.js"
import type { EditorsApi } from "./ns-editor.js"
import { makeInputBox, makeQuickPick } from "./ns-quick-input.js"
import { Uri, Disposable, EventEmitter, noopEvent  } from "./shared.js"
import type { StatusBar } from "./status-bar.js"
import type { TreeViews, TreeDataProviderLike, ViewEmitters, DragController } from "./tree-views.js"
import type { ExtensionFacade } from "./types.js"
import type { Webviews, CreateWebviewOptions } from "./webview.js"

type CommandHandler = (...args: unknown[]) => unknown

/** A `showQuickPick` item as the caller passes it — a bare string or a
 *  QuickPickItem (we read the subset we render + round-trip). */
type QuickPickItemLike = string | { label: string; description?: string; detail?: string; kind?: number; picked?: boolean; alwaysShow?: boolean }

/** A `show*Message` item: a bare string or a `vscode.MessageItem`. Extensions
 *  may attach extra props (even callbacks) to the item; those must NOT cross the
 *  IPC boundary (structured-clone throws "could not be cloned" on functions). */
type MessageItemLike = string | { title?: string; isCloseAffordance?: boolean; [k: string]: unknown }

/** `vscode.window.show{Information,Warning,Error}Message`. Only item TITLES are
 *  forwarded to the renderer (sanitized — an item may carry callbacks/options
 *  that can't be serialized). The chosen title is mapped back to the ORIGINAL
 *  item object so the caller gets its callbacks intact (vscode returns the
 *  picked MessageItem). Bare MessageOptions (no `title`) are not button items. */
function showMessageItems(
  h: NsHooks, severity: "info" | "warning" | "error", message: string, items: MessageItemLike[],
): Promise<MessageItemLike | undefined> {
  const buttons = items.filter((it) => typeof it === "string" || typeof it.title === "string")
  const titles = buttons.map((it) => (typeof it === "string" ? it : (it.title ?? "")))
  return h.showMessage(severity, message, titles).then((chosen) => {
    if (chosen === undefined) return undefined
    const idx = titles.indexOf(chosen)
    return idx >= 0 ? buttons[idx] : chosen
  })
}

const DECO_STRING_FIELDS = ["backgroundColor", "border", "borderRadius", "color", "fontStyle", "fontWeight", "textDecoration", "outline", "opacity", "cursor", "overviewRulerColor"] as const

/** Map vscode DecorationRenderOptions → the string-only DTO the renderer turns
 *  into a CSS class. ThemeColor objects (non-string colors) are dropped. */
function toDecoOptions(o: unknown): DecorationRenderOptionsDto {
  const opt = (o ?? {}) as Record<string, unknown>
  const out: DecorationRenderOptionsDto = {}
  if (typeof opt.isWholeLine === "boolean") out.isWholeLine = opt.isWholeLine
  if (typeof opt.overviewRulerLane === "number") out.overviewRulerLane = opt.overviewRulerLane
  for (const k of DECO_STRING_FIELDS) {
    const v = opt[k]
    if (typeof v === "string") out[k] = v
  }
  return out
}

export interface NsHooks {
  registry: Registry
  /** Fire-and-forget toast — used for activation errors and log noise.
   *  No buttons, no return value. */
  emitNotification: (n: { severity: "info" | "warning" | "error"; message: string }) => void
  /** Real `vscode.window.show*Message` implementation. Returns the
   *  label of the action button the user clicked, or undefined on
   *  dismissal. With no items the toast auto-dismisses and resolves
   *  undefined. With items, the toast is sticky until clicked. */
  showMessage: (
    severity: "info" | "warning" | "error",
    message: string,
    items: string[],
  ) => Promise<string | undefined>
  /** Real `vscode.window.showInputBox` — returns user-entered text
   *  or undefined on dismissal. */
  showInputBox: (opts: { prompt?: string; placeHolder?: string; value?: string }) => Promise<string | undefined>
  /** Real `vscode.window.showQuickPick` — returns chosen item indices (caller
   *  maps back to the original items), or null on dismissal. */
  showQuickPick: (items: QuickPickItemDto[], options: QuickPickOptionsDto) => Promise<number[] | null>
  /** Real `vscode.window.showOpenDialog` — native file dialog, returns chosen
   *  absolute paths (mapped to Uri[] by the caller), or null on cancel. */
  showOpenDialog: (options: OpenDialogOptionsDto) => Promise<string[] | null>
  /** Real `vscode.window.showSaveDialog` — native save dialog, returns the
   *  chosen absolute path (mapped to Uri by the caller), or null on cancel. */
  showSaveDialog: (options: SaveDialogOptionsDto) => Promise<string | null>
  /** Output channel events — host broadcasts to renderer where the
   *  Logs panel collects per-name buffers. */
  emitOutputEvent: (event: { channel: string; op: "append" | "replace" | "clear" | "dispose" | "show"; text?: string; extensionId: string }) => void
  /** `vscode.env.clipboard.writeText` — the ext-host is a Node child with no DOM,
   *  so it broadcasts to the renderer (non-sandboxed main window) which does the
   *  real `navigator.clipboard.writeText`. */
  writeClipboard: (text: string) => void
  /** `vscode.env.clipboard.readText` — the ext-host has no DOM, so it asks the
   *  renderer (main window) to do the real `navigator.clipboard.readText`. */
  readClipboard: () => Promise<string>
  /** `vscode.env.openExternal(uri)` — the ext-host asks the renderer to open the
   *  target (http/https/mailto/file) in the OS default app; resolves the success
   *  flag. The renderer routes it to the host's shell-open. */
  openExternal: (target: string) => Promise<boolean>
  listExtensions: () => readonly ExtensionFacade[]
  /** Host workspace surface (folder + fs) backing `vscode.workspace` (B1). */
  workspaceHost: WorkspaceHost
  /** Shared host-wide language-feature provider registry (B6). */
  languageFeatures: LanguageFeatures
  /** Shared host-wide diagnostic registry (B6c). */
  diagnostics: Diagnostics
  /** Shared host-wide webview registry (B7a). */
  webviews: Webviews
  /** Shared host-wide status-bar registry (B8). */
  statusBar: StatusBar
  /** Shared host-wide tree-view registry (TreeDataProvider). */
  treeViews: TreeViews
  /** Shared host-wide file-decoration registry (FileDecorationProvider). */
  fileDecorations: FileDecorations
}

export function buildCommands(h: NsHooks, extId: string) {
  return {
    registerCommand(id: string, callback: CommandHandler, thisArg?: unknown): Disposable {
      const bound = thisArg !== undefined ? callback.bind(thisArg) : callback
      return h.registry.registerCommand(id, bound, { source: extId }) as Disposable
    },
    registerTextEditorCommand(): Disposable { return new Disposable(() => {}) },
    executeCommand<T = unknown>(id: string, ...args: unknown[]): Promise<T> {
      return h.registry.executeCommand(id, ...args) as Promise<T>
    },
    getCommands(filterInternal?: boolean): Promise<string[]> {
      const all = h.registry.snapshot().commands.map((c) => c.id)
      return Promise.resolve(filterInternal ? all.filter((id) => !id.startsWith("_")) : all)
    },
  }
}

export function buildWindow(h: NsHooks, extId: string, editors: EditorsApi) {
  // The calling extension's install dir — used as the default localResourceRoot
  // for webviews (B7c-2), so bundled assets load via asWebviewUri without the
  // extension having to pass localResourceRoots explicitly.
  const extensionRoot = (): string | undefined => h.listExtensions().find((e) => e.id === extId)?.extensionPath
  return {
    showInformationMessage: (m: string, ...items: MessageItemLike[]) => showMessageItems(h, "info", m, items),
    showWarningMessage: (m: string, ...items: MessageItemLike[]) => showMessageItems(h, "warning", m, items),
    showErrorMessage: (m: string, ...items: MessageItemLike[]) => showMessageItems(h, "error", m, items),
    showQuickPick: async (
      items: readonly QuickPickItemLike[] | Promise<readonly QuickPickItemLike[]>,
      options?: QuickPickOptionsDto,
      token?: { isCancellationRequested?: boolean },
    ): Promise<unknown> => {
      // Already-cancelled token → resolve without ever opening the picker. Full
      // mid-flight cancellation isn't wired (showQuickPick is fire-and-forget;
      // cancellable flows use createQuickPick), so this covers the real case.
      if (token?.isCancellationRequested) return undefined
      const list = await Promise.resolve(items)
      const dto: QuickPickItemDto[] = list.map((it) => typeof it === "string"
        ? { label: it }
        : { label: it.label, description: it.description, detail: it.detail, kind: it.kind, picked: it.picked, alwaysShow: it.alwaysShow })
      const picks = await h.showQuickPick(dto, options ?? {})
      if (picks === null) return undefined // dismissed/cancelled
      const chosen = picks.map((i) => list[i]).filter((x): x is QuickPickItemLike => x !== undefined)
      // canPickMany resolves to the array (possibly empty when OK'd with no
      // ticks — VS Code returns [], NOT undefined); single resolves to one item.
      return options?.canPickMany ? chosen : chosen[0]
    },
    showInputBox: (opts?: { prompt?: string; placeHolder?: string; value?: string }) => h.showInputBox(opts ?? {}),
    showOpenDialog: async (options?: {
      canSelectFiles?: boolean; canSelectFolders?: boolean; canSelectMany?: boolean
      filters?: Record<string, string[]>; defaultUri?: { fsPath?: string }; title?: string
    }): Promise<unknown[] | undefined> => {
      const paths = await h.showOpenDialog({
        canSelectFiles: options?.canSelectFiles,
        canSelectFolders: options?.canSelectFolders,
        canSelectMany: options?.canSelectMany,
        filters: options?.filters,
        title: options?.title,
        defaultPath: options?.defaultUri?.fsPath,
      })
      return paths ? paths.map((p) => Uri.file(p)) : undefined
    },
    showSaveDialog: async (options?: {
      defaultUri?: { fsPath?: string }
      filters?: Record<string, string[]>
      title?: string
    }) => {
      const path = await h.showSaveDialog({
        filters: options?.filters,
        title: options?.title,
        defaultPath: options?.defaultUri?.fsPath,
      })
      return path ? Uri.file(path) : undefined
    },
    // Single-folder workspace (Phase A): resolve the open folder directly rather
    // than popping a picker with one option. Undefined when no folder is open —
    // matching vscode's "cancelled / nothing to pick" result.
    showWorkspaceFolderPick: (): Promise<{ uri: unknown; name: string; index: number } | undefined> => {
      const p = h.workspaceHost.getFolderPath()
      if (!p) return Promise.resolve(undefined)
      const name = p.replace(/[\\/]+$/, "").split(/[\\/]/).pop() ?? p
      return Promise.resolve({ uri: Uri.file(p), name, index: 0 })
    },
    showTextDocument: (target: unknown) => editors.showTextDocument(target),
    createOutputChannel: (name: string) => {
      // Buffer kept locally in the extension process AND mirrored to
      // the renderer Logs panel via `emitOutputEvent`. Renderer is the
      // visible source of truth; the local copy lets `replace()` /
      // `clear()` work even if no renderer is attached (headless tests).
      const state = { buf: "" }
      const emit = (op: "append" | "replace" | "clear" | "dispose" | "show", text?: string): void => {
        h.emitOutputEvent(text === undefined
          ? { channel: name, op, extensionId: extId }
          : { channel: name, op, text, extensionId: extId })
      }
      return {
        name,
        append(v: string) { state.buf += v; emit("append", v) },
        appendLine(v: string) { state.buf += `${v}\n`; emit("append", `${v}\n`) },
        replace(v: string) { state.buf = v; emit("replace", v) },
        clear() { state.buf = ""; emit("clear") },
        show() { emit("show") }, hide() {}, dispose() { emit("dispose") },
        trace() {}, debug() {}, info() {}, warn() {}, error() {},
        logLevel: 3, onDidChangeLogLevel: noopEvent,
      }
    },
    // Overloads: (id, alignment?, priority?) | (alignment?, priority?).
    createStatusBarItem: (a?: unknown, b?: unknown, c?: unknown) => {
      const idGiven = typeof a === "string"
      const alignment = (idGiven ? b : a)
      const priority = (idGiven ? c : b)
      return h.statusBar.createItem(extId, typeof alignment === "number" ? alignment : 1, typeof priority === "number" ? priority : 0)
    },
    createTerminal: () => ({ name: "", sendText() {}, show() {}, hide() {}, dispose() {} }),
    // B7a: real webview panels — html/postMessage broadcast to the renderer's
    // sandboxed iframe; onDidReceiveMessage fires on relayed iframe messages.
    createWebviewPanel: (viewType: string, title: string, showOptions: unknown, options?: CreateWebviewOptions) =>
      h.webviews.createPanel(viewType, title, showOptions, options ?? {}, extensionRoot(), undefined, extId),
    createInputBox: () => makeInputBox(h),
    createQuickPick: () => makeQuickPick(h),
    get activeTextEditor() { return editors.activeTextEditor() },
    get visibleTextEditors() { return editors.visibleTextEditors() },
    activeTerminal: undefined,
    terminals: [] as unknown[],
    activeNotebookEditor: undefined,
    visibleNotebookEditors: [] as unknown[],
    tabGroups: { all: [], activeTabGroup: undefined, onDidChangeTabs: noopEvent, onDidChangeTabGroups: noopEvent, close: () => Promise.resolve(true) },
    state: { focused: true, active: true },
    onDidChangeActiveTextEditor: editors.onDidChangeActiveTextEditor,
    onDidChangeVisibleTextEditors: editors.onDidChangeVisibleTextEditors,
    onDidChangeTextEditorSelection: editors.onDidChangeTextEditorSelection,
    onDidChangeTextEditorVisibleRanges: noopEvent, onDidChangeTextEditorOptions: noopEvent,
    onDidChangeTextEditorViewColumn: noopEvent, onDidChangeActiveTerminal: noopEvent,
    onDidOpenTerminal: noopEvent, onDidCloseTerminal: noopEvent, onDidChangeTerminalState: noopEvent,
    // Terminal shell integration (1.93+) — we have no shell-integration data
    // yet, so these never fire; extensions (ms-python.vscode-python-envs) only
    // need them to BE functions at activation.
    onDidChangeTerminalShellIntegration: noopEvent, onDidStartTerminalShellExecution: noopEvent, onDidEndTerminalShellExecution: noopEvent,
    onDidChangeWindowState: noopEvent, onDidChangeActiveColorTheme: noopEvent,
    registerTreeDataProvider: (viewId: string, provider: TreeDataProviderLike) =>
      h.treeViews.register(viewId, provider, null),
    // B7b/B7c stubs — correct signatures so the args aren't silently dropped at
    // the call site; the provider/serializer callbacks aren't invoked yet.
    registerWebviewViewProvider: (viewId: string, provider: { resolveWebviewView: (view: unknown, ctx: { state: unknown }, token: unknown) => void | Promise<void> }, _options?: unknown) => {
      const reg = h.webviews.registerWebviewViewProvider(viewId, provider, extensionRoot())
      return new Disposable(() => { reg.dispose() })
    },
    registerWebviewPanelSerializer: (viewType: string, serializer: { deserializeWebviewPanel: (panel: unknown, state: unknown) => void | Promise<void> }) => {
      const reg = h.webviews.registerSerializer(viewType, serializer, extensionRoot())
      return new Disposable(() => { reg.dispose() })
    },
    registerCustomEditorProvider: () => new Disposable(() => {}),
    registerTerminalLinkProvider: () => new Disposable(() => {}),
    registerTerminalProfileProvider: () => new Disposable(() => {}),
    registerFileDecorationProvider: (provider: FileDecorationProviderLike) => h.fileDecorations.register(provider),
    registerUriHandler: () => new Disposable(() => {}),
    createTreeView: (viewId: string, options: { treeDataProvider: TreeDataProviderLike; dragAndDropController?: DragController }) => {
      const emitters: ViewEmitters = {
        selection: new EventEmitter(), expand: new EventEmitter(),
        collapse: new EventEmitter(), visibility: new EventEmitter(),
        checkbox: new EventEmitter(),
      }
      let selection: readonly unknown[] = []
      let visible = true
      emitters.selection.event((e) => { selection = e.selection })
      emitters.visibility.event((e) => { visible = e.visible })
      const reg = h.treeViews.register(viewId, options.treeDataProvider, emitters, options.dragAndDropController ?? null)
      // message/title/description/badge are live: a setter merges the patch into
      // the host's per-view meta and pushes it to the renderer (header + message
      // line). Closure vars back the getters so reads return what was set.
      let message: string | undefined
      let title: string | undefined
      let description: string | undefined
      // ViewBadge.tooltip is required in the VS Code API (not optional).
      let badge: { value: number; tooltip: string } | undefined
      return {
        onDidChangeSelection: emitters.selection.event,
        onDidExpandElement: emitters.expand.event,
        onDidCollapseElement: emitters.collapse.event,
        onDidChangeVisibility: emitters.visibility.event,
        onDidChangeCheckboxState: emitters.checkbox.event,
        get selection() { return selection },
        get visible() { return visible },
        get message() { return message },
        set message(v: string | undefined) { message = v; h.treeViews.setMeta(viewId, { message: v }) },
        get title() { return title },
        set title(v: string | undefined) { title = v; h.treeViews.setMeta(viewId, { title: v }) },
        get description() { return description },
        set description(v: string | undefined) { description = v; h.treeViews.setMeta(viewId, { description: v }) },
        get badge() { return badge },
        set badge(v: { value: number; tooltip: string } | undefined) {
          badge = v
          h.treeViews.setMeta(viewId, { badge: v ? { value: v.value, tooltip: v.tooltip } : undefined })
        },
        reveal: (element: unknown, opts?: { select?: boolean; focus?: boolean; expand?: boolean | number }) =>
          h.treeViews.reveal(viewId, element, opts),
        dispose: () => { reg.dispose() },
      }
    },
    createTextEditorDecorationType: (options: unknown) => {
      const key = createDecorationKey(extId, toDecoOptions(options))
      return { key, dispose: () => { disposeDecorationKey(key); h.workspaceHost.editors.disposeDecorationType(key) } }
    },
    withProgress: (_o: unknown, task: (progress: { report(): void }, token: { isCancellationRequested: boolean; onCancellationRequested: () => { dispose(): void } }) => unknown) =>
      Promise.resolve(task({ report() {} }, { isCancellationRequested: false, onCancellationRequested: () => ({ dispose() {} }) })),
    setStatusBarMessage: (text: string, hideAfterMs?: number) => {
      const handle = h.statusBar.setStatusBarMessage(text, hideAfterMs)
      return new Disposable(() => { handle.dispose() })
    },
    activeColorTheme: { kind: 2 },
  }
}

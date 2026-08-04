// Dependency-injection contract between the extension host and the
// kamin-host services it bridges the `vscode.*` runtime onto (Phase B).
//
// exthost runs in-process with the host's fs/index/watcher services, but
// we inject them (rather than import the service modules directly) so the
// api builders stay testable and the dependency direction stays explicit —
// same pattern as the existing showMessage/showInputBox/broadcast hooks.

import type { KaminSession, SessionsSnapshot } from "../api/types.js"

export type HostFileType = "file" | "dir" | "symlink" | "other"

/** Projects + Sessions surface backing the `kaminide.sessions` API (Phase 3).
 *  Delegates to the host's sessions store; the in-process change buses return a
 *  Disposable so the api layer can re-fire them as real vscode `Event<T>`. */
export interface SessionsHost {
  /** Current projects + sessions + active id. */
  list(): SessionsSnapshot
  /** The active session, or null. */
  getActive(): KaminSession | null
  /** Fire on any session/project mutation (full snapshot). */
  onChange(listener: (snap: SessionsSnapshot) => void): { dispose(): void }
  /** Fire when the active session changes (full session, or null). */
  onActiveChange(listener: (session: KaminSession | null) => void): { dispose(): void }
  // Mutators round-trip the host's sessions store. When the ext-host runs in a
  // forked child (crash isolation) the store lives in the parent process, so the
  // value-returning mutators are async (RPC); the void ones fire-and-forget.
  /** Create + activate a session (in `projectId`, or the active/No-folder project). */
  createSession(opts: { projectId?: string; name?: string }): Promise<KaminSession>
  /** Make `id` the active session (re-roots the tree to its project). */
  setActiveSession(id: string): void
  /** Shallow-merge name/metadata; resolves the updated session or null if absent. */
  updateSession(id: string, patch: { name?: string; metadata?: Record<string, unknown> }): Promise<KaminSession | null>
  /** Remove a session. */
  deleteSession(id: string): void
}

export interface HostFileStat {
  type: HostFileType
  size: number
  ctimeMs: number
  mtimeMs: number
}

/** Raw filesystem ops over ABSOLUTE paths. The `vscode.workspace.fs`
 *  layer converts `Uri` → fsPath before calling these. */
export interface HostFs {
  stat(absPath: string): Promise<HostFileStat>
  readFile(absPath: string): Promise<Uint8Array>
  writeFile(absPath: string, data: Uint8Array): Promise<void>
  readDirectory(absPath: string): Promise<[string, HostFileType][]>
  createDirectory(absPath: string): Promise<void>
  delete(absPath: string, recursive: boolean, useTrash: boolean): Promise<void>
  rename(srcAbs: string, dstAbs: string, overwrite: boolean): Promise<void>
  copy(srcAbs: string, dstAbs: string, overwrite: boolean): Promise<void>
}

/** One filesystem change, as the host watcher reports it. */
export interface WatchEvent {
  kind: "add" | "change" | "unlink" | "addDir" | "unlinkDir"
  /** Absolute path. */
  path: string
}

/** A workspace-relative file the index knows about (for findFiles). */
export interface IndexedFileRef {
  /** POSIX, workspace-relative — matched against the glob. */
  rel: string
  /** Absolute OS path. */
  abs: string
}

/** Per-layer values for one dotted config key (`inspect`). */
export interface ConfigInspect {
  defaultValue?: unknown
  globalValue?: unknown
  workspaceValue?: unknown
}

/** Configuration surface backing `vscode.workspace.getConfiguration` (B2).
 *  All keys are flat + dotted; section prefixing/sub-tree reconstruction is
 *  the vscode-api layer's job. Merge order: default ← global ← workspace. */
export interface ConfigHost {
  /** Register contributed defaults (flat dotted key → default value). */
  registerDefaults(values: Record<string, unknown>): void
  /** Merged snapshot, workspace > global > default. */
  getAll(): Record<string, unknown>
  /** Per-layer values for one key. */
  inspect(key: string): ConfigInspect
  /** Write at a layer (target 1 = Global, 2|3 = Workspace; `undefined` resets). */
  update(key: string, value: unknown, target: number): Promise<void>
  /** Fire on any change; payload = the affected dotted keys. */
  onDidChange(listener: (keys: readonly string[]) => void): () => void
}

/** Extension persistence (B9) backing ExtensionContext.globalState /
 *  workspaceState / secrets + the storage directories. All operations are
 *  per-extension (the caller passes its extId). */
export interface StorageHost {
  globalGet(extId: string, key: string): unknown
  globalUpdate(extId: string, key: string, value: unknown): void
  globalKeys(extId: string): readonly string[]
  workspaceGet(extId: string, key: string): unknown
  workspaceUpdate(extId: string, key: string, value: unknown): void
  workspaceKeys(extId: string): readonly string[]
  // Async: secret values are sealed/unsealed via the renderer→Rust DPAPI relay.
  secretGet(extId: string, key: string): Promise<string | undefined>
  secretSet(extId: string, key: string, value: string): Promise<void>
  secretDelete(extId: string, key: string): void
  secretKeys(extId: string): readonly string[]
  onSecretChange(extId: string, listener: (key: string) => void): () => void
  /** context.globalStorageUri dir (always present). */
  globalStorageDir(extId: string): string
  /** context.logUri dir (always present). */
  logDir(extId: string): string
  /** context.storageUri dir, or null when no workspace folder is open. */
  storageDir(extId: string): string | null
}

/** One mirrored open document (B5) — content is the live, possibly-dirty
 *  editor buffer, not the on-disk file. */
export interface HostDocument {
  /** Absolute fsPath. */
  uri: string
  languageId: string
  version: number
  content: string
}

/** One incremental content change (B5b), 0-based range. Mirrors vscode's
 *  TextDocumentContentChangeEvent minus the Range-class wrapping. */
export interface HostDocChange {
  range: { startLine: number; startChar: number; endLine: number; endChar: number }
  rangeOffset: number
  rangeLength: number
  text: string
}

/** Open-document surface backing `vscode.workspace.textDocuments` (B5). Fed by
 *  the renderer's Monaco doc-sync; the host never reads disk for this. */
export interface DocumentHost {
  list(): readonly HostDocument[]
  get(uri: string): HostDocument | undefined
  /** Register a document in the mirror (e.g. `openTextDocument` of a file not
   *  in any editor) so it participates in change/close events. */
  open(doc: HostDocument): void
  onDidOpen(listener: (d: HostDocument) => void): () => void
  onDidChange(listener: (d: HostDocument, changes: readonly HostDocChange[]) => void): () => void
  onDidClose(listener: (d: HostDocument) => void): () => void
  onDidSave(listener: (d: HostDocument) => void): () => void
}

/** A selection mirrored from the renderer (B5b-2a) — 0-based anchor + active. */
export interface HostSelection {
  anchor: { line: number; character: number }
  active: { line: number; character: number }
}

/** One text replacement for `TextEditor.edit` (B5b-2b), 0-based range. An
 *  insert is a zero-width range; a delete is an empty `text`. */
export interface HostTextEdit {
  range: { startLine: number; startChar: number; endLine: number; endChar: number }
  text: string
}

/** Subset of vscode DecorationRenderOptions we render (B5b-2c). String colors
 *  only — ThemeColor objects are dropped (we have no theme-color table yet). */
export interface DecorationRenderOptionsDto {
  isWholeLine?: boolean
  backgroundColor?: string
  border?: string
  borderRadius?: string
  color?: string
  fontStyle?: string
  fontWeight?: string
  textDecoration?: string
  outline?: string
  opacity?: string
  cursor?: string
  overviewRulerColor?: string
  /** vscode OverviewRulerLane (1 Left, 2 Center, 4 Right, 7 Full). */
  overviewRulerLane?: number
}

/** One decoration target (vscode Range or DecorationOptions), 0-based. */
export interface DecorationItemDto {
  range: HostTextEdit["range"]
  hoverMessage?: string
}

/** Active-editor + selection surface backing `window.activeTextEditor` /
 *  `onDidChangeTextEditorSelection` (B5b-2a). Fed by the renderer's Monaco.
 *  The write methods (B5b-2b/2c) reach the renderer's Monaco via host→renderer
 *  RPC and resolve once it has applied them. */
export interface EditorHost {
  getActive(): string | null
  getSelections(uri: string): readonly HostSelection[]
  onDidChangeActive(listener: (uri: string | null) => void): () => void
  onDidChangeSelection(listener: (uri: string, sels: readonly HostSelection[]) => void): () => void
  /** Apply edits to the document's Monaco model as one undo step; resolves
   *  `false` if no model is open for `uri`. `eol` (vscode.EndOfLine: LF=1,
   *  CRLF=2) optionally changes the model's line ending in the same edit
   *  (backs TextEditorEdit.setEndOfLine). */
  applyEdits(uri: string, edits: readonly HostTextEdit[], eol?: number): Promise<boolean>
  /** Scroll the editor so `range` is visible (vscode TextEditorRevealType). */
  revealRange(uri: string, range: HostTextEdit["range"], revealType: number): Promise<void>
  /** Open `uri` in the editor (backing `window.showTextDocument`); resolves
   *  once the model is loaded so the document mirror is populated. */
  showDocument(uri: string): Promise<void>
  /** Apply a decoration type's ranges to `uri`'s Monaco model (B5b-2c).
   *  Replaces any previous set for the same (uri, key). */
  setDecorations(uri: string, key: string, options: DecorationRenderOptionsDto, items: readonly DecorationItemDto[]): void
  /** Drop a decoration type everywhere (its `dispose()`). */
  disposeDecorationType(key: string): void
  /** Insert a snippet (tab-stops honoured) at the editor's selection, or at
   *  `location` if given. Resolves `false` if no model is open. */
  insertSnippet(uri: string, snippet: string, location?: HostTextEdit["range"]): Promise<boolean>
  /** Move the editor's selection(s) — backs `editor.selection`/`selections` setters. */
  setSelections(uri: string, selections: readonly HostSelection[]): void
}

/** One variable mutation. `type` is vscode.EnvironmentVariableMutatorType
 *  (Replace 1 / Append 2 / Prepend 3). */
export interface EnvVarMutationDto { type: number; value: string }
/** An extension's whole `environmentVariableCollection` as the child holds it. */
export interface EnvCollectionSnapshot { persistent: boolean; description?: string; vars: Record<string, EnvVarMutationDto> }

/** Backs `vscode.ExtensionContext.environmentVariableCollection` (#11). The
 *  collection lives in the child (vscode's get/forEach are synchronous); the
 *  child pushes a full snapshot to the parent on every change, where it is
 *  applied to integrated-terminal spawns and the persistent part is saved. */
export interface EnvHost {
  sync(extId: string, snapshot: EnvCollectionSnapshot): void
  /** Drop an extension's session contribution on disable/unload. Persistent
   *  mutations survive on disk and reapply next boot (VS Code parity). */
  drop(extId: string): void
}

/** Workspace-facing host surface backing `vscode.workspace`. */
export interface WorkspaceHost {
  /** The single open folder's absolute path, or null. Read live on each
   *  `workspace.workspaceFolders` access so it tracks Open Folder. */
  getFolderPath(): string | null
  /** Notify on open-folder change → `onDidChangeWorkspaceFolders` (B1b). */
  onDidChangeFolder(listener: (path: string | null) => void): () => void
  fs: HostFs
  /** Snapshot of indexed files for `findFiles` glob matching (B1b). Async: the
   *  file index lives in the parent process, reached over RPC from the child. */
  listFiles(): Promise<readonly IndexedFileRef[]>
  /** Subscribe to fs-event batches for `createFileSystemWatcher` (B1b). */
  watchFiles(listener: (events: readonly WatchEvent[]) => void): () => void
  /** Configuration layers backing `getConfiguration` (B2). */
  config: ConfigHost
  /** Open-document mirror backing `textDocuments` (B5). */
  documents: DocumentHost
  /** Active-editor + selection mirror backing `window.activeTextEditor` (B5b). */
  editors: EditorHost
}

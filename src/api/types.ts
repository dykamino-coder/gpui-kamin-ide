// Shared types for the KaminIDE host. These are NOT the public vscode.*
// surface — those live in `src/exthost/api.ts` and mirror @types/vscode.
// This file holds plumbing types used between main, exthost, and preload.

export interface Disposable {
  dispose(): void
}

/** `vscode.window.showQuickPick` plumbing (B8). Items are normalised to this
 *  shape over the host↔renderer boundary; the renderer returns chosen INDICES
 *  so the exthost can map back to the caller's original string/QuickPickItem. */
export interface QuickPickItemDto {
  label: string
  description?: string | undefined
  detail?: string | undefined
  /** `QuickPickItemKind` — Separator (-1) renders as a non-interactive divider. */
  kind?: number | undefined
  /** Pre-checked when `canPickMany`. */
  picked?: boolean | undefined
  /** Always shown regardless of the filter query. */
  alwaysShow?: boolean | undefined
}
export interface QuickPickOptionsDto {
  placeHolder?: string | undefined
  /** Second line of guidance shown below the filter input. */
  prompt?: string | undefined
  canPickMany?: boolean | undefined
  title?: string | undefined
  matchOnDescription?: boolean | undefined
  matchOnDetail?: boolean | undefined
  /** When true a backdrop click does NOT dismiss the pick. */
  ignoreFocusOut?: boolean | undefined
  // NOTE: `onDidSelectItem` (a callback) is intentionally NOT carried — it can't
  // cross the host↔renderer WS boundary. Extensions relying on live select-hover
  // side-effects get no callbacks (documented Phase-B omission).
}

/** `vscode.window.showOpenDialog` plumbing. A serialisable subset of vscode's
 *  OpenDialogOptions (the Uri `defaultUri` is flattened to `defaultPath`); the
 *  renderer opens the native Tauri file dialog and returns the chosen absolute
 *  paths (or null on cancel), which the exthost maps back to `Uri[]`. */
export interface OpenDialogOptionsDto {
  canSelectFiles?: boolean | undefined
  canSelectFolders?: boolean | undefined
  canSelectMany?: boolean | undefined
  /** name → allowed extensions WITHOUT the dot, e.g. { Images: ["png","jpg"] }. */
  filters?: Record<string, string[]> | undefined
  title?: string | undefined
  defaultPath?: string | undefined
}

export interface SaveDialogOptionsDto {
  /** name → allowed extensions WITHOUT the dot, e.g. { JSONL: ["jsonl"] }. */
  filters?: Record<string, string[]> | undefined
  title?: string | undefined
  defaultPath?: string | undefined
}

export interface RegistryCommand {
  id: string
  title: string
  category: string | undefined
  // "host" for built-in commands; extension id (publisher.name) for
   // contributed ones. Using an unbranded string keeps the union open
   // — the literal "host" got collapsed by typescript-eslint's
   // redundant-constituent check, so we annotate intent in prose.
  source: string
}

export interface RegistryView {
  id: string
  name: string
  containerId: string
  /** "tree" (default) or "webview" — webview views resolve via
   *  `registerWebviewViewProvider` and render a sandboxed iframe (B7b). */
  type?: "tree" | "webview"
  /** Optional codicon id (or image URL) for the view's nav entry. Used when a
   *  `customize`-located container renders its views as a TOC tree (each view is
   *  a separate page) — KaminIDE extension over stock `contributes.views`. */
  icon?: string
}

export interface RegistryViewContainer {
  id: string
  title: string
  icon: string // codicon id
  location: "activitybar" | "panel" | "auxiliarybar" | "customize"
}

/** A `TreeItem.command` flattened for the renderer (B-tree). */
export interface TreeCommandDto {
  command: string
  title?: string
  arguments?: unknown[]
}

/** A `FileDecoration` flattened for the renderer. `color` is a ThemeColor id
 *  (e.g. "gitDecoration.modifiedResourceForeground") the renderer maps to CSS. */
export interface FileDecorationDto {
  badge?: string
  tooltip?: string
  color?: string
  propagate?: boolean
}

/** One TreeDataProvider node serialized for the renderer. `handle` is the
 *  host-side opaque id the renderer passes back to fetch this node's children
 *  (the real element object can't cross the WS). */
export interface TreeNodeDto {
  handle: string
  label: string
  description?: string
  tooltip?: string
  /** TreeItemCollapsibleState: 0 None, 1 Collapsed, 2 Expanded. */
  collapsibleState: number
  /** ThemeIcon id → a codicon class. */
  codicon?: string
  /** resourceUri fsPath → file-icon resolution + label fallback. */
  resourceUri?: string
  command?: TreeCommandDto
  contextValue?: string
  /** TreeItemCheckboxState (0 Unchecked, 1 Checked); undefined = no checkbox. */
  checkboxState?: number
  /** Optional tooltip for the checkbox. */
  checkboxTooltip?: string
}

/** createTreeView display props (message/title/description/badge). Pushed to the
 *  renderer via `kamin:tree:meta` and pulled on mount via `kamin:tree:getMeta`. */
export interface TreeViewMetaDto {
  message?: string
  title?: string
  description?: string
  badge?: { value: number; tooltip: string }
}

/** One `contributes.menus` entry (B4). `menuId` is the slot, e.g.
 *  "commandPalette", "editor/context", "view/title". Exactly one of
 *  command/submenu is set. `group` drives ordering ("navigation@1"). */
export interface RegistryMenuItem {
  command?: string
  submenu?: string
  group?: string
  when?: string
  /** Alternate command shown when Alt is held. */
  alt?: string
}

/** One `contributes.keybindings` entry (B4). Platform-specific chords fall
 *  back to `key` when absent; the resolver picks by `process.platform`. */
export interface RegistryKeybinding {
  key: string
  mac?: string
  linux?: string
  win?: string
  command: string
  when?: string
  args?: unknown
}

/** One `contributes.submenus` entry (B4) — a named menu referenced by a
 *  menu item's `submenu`. */
export interface RegistrySubmenu {
  id: string
  label: string
  icon?: string
}

/** One `contributes.languages` entry — the ext/filename → language-id map the
 *  renderer feeds into Monaco so files like `.vue` resolve to their real id
 *  (and their LSP DocumentSelectors match) instead of falling back to plaintext. */
export interface RegistryLanguage {
  id: string
  extensions?: string[]
  filenames?: string[]
  aliases?: string[]
  firstLine?: string
}

/** One `contributes.grammars` entry — a TextMate grammar the renderer loads
 *  (via vscode-textmate) to tokenise/syntax-highlight a contributed language.
 *  `path` is absolute (resolved from the extension dir at parse time). */
export interface RegistryGrammar {
  scopeName: string
  language?: string
  path: string
  embeddedLanguages?: Record<string, string>
  injectTo?: string[]
}

export interface RegistrySnapshot {
  commands: RegistryCommand[]
  views: RegistryView[]
  viewContainers: RegistryViewContainer[]
  themes: { id: string; label: string; uiTheme: "vs-dark" | "vs" | "hc-black" | "hc-light"; path: string }[]
  /** File icon themes (`contributes.iconThemes`, B10). `path` is absolute. */
  iconThemes: { id: string; label: string; path: string }[]
  /** Contributed languages (`contributes.languages`). */
  languages: RegistryLanguage[]
  /** Contributed TextMate grammars (`contributes.grammars`). `path` is absolute. */
  grammars: RegistryGrammar[]
  contextKeys: Record<string, unknown>
  /** Contributed menus keyed by menu id (B4). */
  menus: Record<string, RegistryMenuItem[]>
  keybindings: RegistryKeybinding[]
  submenus: RegistrySubmenu[]
}

/** A contributed colour theme parsed by the host (kamin:theme:read) for the
 *  renderer to apply to Monaco + the `--vscode-*` surfaces. */
export interface ContributedThemeData {
  type: "dark" | "light"
  /** package.json `uiTheme` — drives the Monaco base (incl. high-contrast). Set by
   *  the renderer from the registry; absent in pre-existing caches (→ base from `type`). */
  uiTheme?: "vs" | "vs-dark" | "hc-black" | "hc-light"
  colors: Record<string, string>
  tokenColors: { scope?: string | string[]; settings: { foreground?: string; background?: string; fontStyle?: string } }[]
}

/** A project = an explicitly opened folder (Projects/Sessions, Phase 2). Sessions
 *  live inside a project; the file tree follows the active session's project.
 *  `folderPath: null` is the synthetic "No folder" bucket. The display name is
 *  derived from `basename(folderPath)` (host doesn't store it). */
export interface KaminProject {
  id: string
  folderPath: string | null
  createdAt: number
}

/** A session = a named, persisted context belonging to a project. Renamable +
 *  colourable by the user; `pinned` shows it in the top pinned-sessions bar.
 *  `metadata` is the Bridge-VSIX seam (Phase 3) — opaque to the core. */
export interface KaminSession {
  id: string
  name: string
  projectId: string
  /** User-assignable accent colour (a hex string from the renderer palette). */
  color?: string
  /** Pinned to the titlebar tab strip — stays a tab even when deactivated
   *  ("sleeping"), and is restored on restart. */
  pinned?: boolean
  /** Active = loaded in memory, shown as a titlebar tab. Deactivating frees it
   *  (the session stays saved in the sidebar's inactive group). */
  open?: boolean
  lastOpened: number
  createdAt: number
  /** Per-session editor snapshot — open file tabs + the active one. Captured on
   *  switch-away, restored on activate (Stage B). */
  editorState?: { openFiles: { path: string; pinned: boolean }[]; activeFile: string | null }
  /** Per-session panel layout — panel visibility/sizes/splits + which tools are
   *  pinned/active in each zone + Files-vs-Web mode. A `LayoutSnapshot` minus
   *  `themeChoice` (theme stays a global preference). Captured/restored on switch. */
  layout?: LayoutSnapshot
  /** Per-session embedded-browser URL (restored into the Web panel on activate). */
  webUrl?: string
  metadata?: Record<string, unknown>
}

/** Full sessions/projects state — broadcast to the renderer as `kamin:sessions:changed`
 *  and returned by `kamin:sessions:list`. */
export interface SessionsSnapshot {
  projects: KaminProject[]
  sessions: KaminSession[]
  activeSessionId: string | null
}

export interface Notification {
  /** Unique id for dismissal — set by `pushToast` if absent. */
  id?: string
  /** "success" is a Bridge-style severity (build done, sync done) —
   *  not in VS Code's `showInformationMessage` family but useful for
   *  in-app feedback. Renders with the accent-green border + check
   *  glyph; ToastKind for external windows mirrors this. */
  severity: "info" | "success" | "warning" | "error"
  message: string
  timestamp: number
  /** Optional title above the message. Bridge-style accent line on
   *  the left uses severity colour; the title is bolder than the body. */
  title?: string
  /** Action button labels. Each button is rendered as a chip; click
   *  resolves the promise returned by `pushToast` with the chosen
   *  label. Skipping the buttons keeps the auto-dismiss behaviour. */
  actions?: string[]
  /** When true, the toast will not auto-dismiss; user has to click
   *  an action / the close button. Used by `withProgress`/`question`-
   *  type prompts where missing the answer is worse than the noise. */
  sticky?: boolean
  /** Client-only, set while the exit animation plays before the toast is
   *  removed from the stack (see `removeWithExit`). Not carried over IPC. */
  leaving?: boolean
}

export interface ExtensionDescriptor {
  id: string
  displayName: string
  version: string
  active: boolean
  builtin: boolean
  /** false = user-disabled (not loaded; no contributions/activation). Persisted
   *  across restarts. Toggled live via window's extension enable/disable. */
  enabled: boolean
  activationError?: string
  packageJSON: Record<string, unknown>
  extensionPath: string
}

// Persisted layout snapshot read by the renderer at boot via IPC.
// Keep loosely-typed so a malformed/legacy on-disk value can be
// validated key-by-key in `signals/persistence.ts` without an
// all-or-nothing reject.
export interface LayoutSnapshot {
  sidebarVisible?: boolean
  // Absolute pixels — sidebar/right don't scale with viewport.
  sidebarWidthPx?: number
  filePanelVisible?: boolean
  // Top-card mode: editor ("files") or embedded browser ("web").
  filePanelMode?: "files" | "web"
  // Ratio — file panel scales with viewport (one of two central panels).
  filePanelWidthRatio?: number
  filePanelBottomVisible?: boolean
  filePanelBottomHeightRatio?: number
  rightPanelVisible?: boolean
  // Absolute pixels — right panel doesn't scale with viewport.
  rightPanelWidthPx?: number
  rightPanelSplit?: number
  rightPanelBottomVisible?: boolean
  mainVisible?: boolean
  mainBottomVisible?: boolean
  mainSplit?: number
  themeChoice?: "dark" | "light" | "system"
  // Per-panel activity state — what's pinned + which is active in
  // each slot. Optional so older layout.json files (pre-activity-bar)
  // hydrate cleanly via key-by-key validation.
  activitySidebar?: { pinned: string[]; active: string | null }
  activityRightTop?: { pinned: string[]; active: string | null }
  activityRightBottom?: { pinned: string[]; active: string | null }
  activityCentralTop?: { pinned: string[]; active: string | null }
  activityCentralBottom?: { pinned: string[]; active: string | null }
  activityMain?: { pinned: string[]; active: string | null }
  activityMainBottom?: { pinned: string[]; active: string | null }
}

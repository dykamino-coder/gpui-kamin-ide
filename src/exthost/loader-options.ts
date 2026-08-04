// Construction contract for ExtensionLoader (extracted from loader.ts, which had
// outgrown the 250-line ceiling carrying this alongside the loading logic).
// Types only — no runtime.
import type { QuickPickItemDto, QuickPickOptionsDto, OpenDialogOptionsDto, SaveDialogOptionsDto } from "../api/types.js"
import type { Diagnostics } from "./api/diagnostics.js"
import type { FileDecorations } from "./api/file-decorations.js"
import type { LanguageFeatures } from "./api/language-features.js"
import type { SessionsApi } from "./api/sessions.js"
import type { StatusBar } from "./api/status-bar.js"
import type { TreeViews } from "./api/tree-views.js"
import type { Webviews } from "./api/webview.js"
import type { EnvHost, StorageHost, WorkspaceHost } from "./host-services.js"
import type { Registry } from "./registry.js"

export interface LoaderOptions {
  builtinDir: string
  /** Writable dir for sideloaded (.vsix / folder) extensions — scanned
   *  alongside builtinDir; these are `builtin: false` and uninstallable. */
  userExtDir: string
  registry: Registry
  emitNotification: (n: { severity: "info" | "warning" | "error"; message: string }) => void
  /** Promise-returning equivalent for `vscode.window.show*Message`. */
  showMessage: (severity: "info" | "warning" | "error", message: string, items: string[]) => Promise<string | undefined>
  /** `vscode.window.showInputBox` implementation. */
  showInputBox: (opts: { prompt?: string; placeHolder?: string; value?: string }) => Promise<string | undefined>
  /** `vscode.window.showQuickPick` — returns chosen item indices, null on cancel. */
  showQuickPick: (items: QuickPickItemDto[], options: QuickPickOptionsDto) => Promise<number[] | null>
  /** `vscode.window.showOpenDialog` — native file dialog, returns paths or null. */
  showOpenDialog: (options: OpenDialogOptionsDto) => Promise<string[] | null>
  /** `vscode.window.showSaveDialog` — native save dialog, returns a path or null. */
  showSaveDialog: (options: SaveDialogOptionsDto) => Promise<string | null>
  /** `vscode.env.openExternal(uri)` — open target in OS default app, resolves success. */
  openExternal: (target: string) => Promise<boolean>
  /** `vscode.env.clipboard.readText` — read the system clipboard via the renderer. */
  readClipboard: () => Promise<string>
  /** Output channel events — passthrough to renderer's Logs panel. */
  emitOutputEvent: (event: { channel: string; op: "append" | "replace" | "clear" | "dispose" | "show"; text?: string; extensionId: string }) => void
  /** `vscode.env.clipboard.writeText` — broadcast to the renderer to do the real write. */
  writeClipboard: (text: string) => void
  /** Host workspace surface (folder + fs) backing `vscode.workspace` (B1). */
  workspaceHost: WorkspaceHost
  /** Extension persistence backing globalState/workspaceState/secrets (B9). */
  storage: StorageHost
  /** environmentVariableCollection sink — applied to terminal spawns (#11). */
  env: EnvHost
  /** Shared language-feature provider registry (B6). */
  languageFeatures: LanguageFeatures
  /** Shared diagnostic registry (B6c). */
  diagnostics: Diagnostics
  /** Shared webview registry (B7a). */
  webviews: Webviews
  /** Shared tree-view registry (TreeDataProvider). */
  treeViews: TreeViews
  /** Shared file-decoration registry (FileDecorationProvider). */
  fileDecorations: FileDecorations
  /** Shared status-bar registry (B8). */
  statusBar: StatusBar
  /** Projects + Sessions registry backing `require('kaminide')` (Phase 3). */
  sessionsApi: SessionsApi
}

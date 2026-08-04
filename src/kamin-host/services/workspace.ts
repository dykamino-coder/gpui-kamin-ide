// Workspace folder state — kamin-host owns the single source of truth
// (and its persistence). Split from `host/fs/workspace-folder.ts` in
// R1b: the OS folder-picker dialog stays in the shell (it needs a
// parent window); the shell calls `setWorkspaceFolder` with the picked
// path. This split also fixes the dual-LayoutStore race: workspace
// state now lives in its own `workspace.json` with exactly one owner.
import { existsSync } from "node:fs"
import { normalize } from "node:path"
import { JsonStore } from "../json-store.js"

export interface WorkspaceFolderState {
  /** Absolute, normalised. `null` = no folder open. */
  path: string | null
}

let store: JsonStore<WorkspaceFolderState> | null = null
let current: WorkspaceFolderState = { path: null }
// Listeners notified when the open folder changes — backs
// `vscode.workspace.onDidChangeWorkspaceFolders` (B1b).
const changeListeners = new Set<(path: string | null) => void>()

/** Subscribe to folder changes. Returns an unsubscribe. */
export function onWorkspaceChange(fn: (path: string | null) => void): () => void {
  changeListeners.add(fn)
  return () => { changeListeners.delete(fn) }
}

/** Called once at host boot.
 *  - `openFolder` (Explorer "Open with KaminIDE" → `--open-folder`) has the
 *    HIGHEST priority: it's an explicit user intent for THIS launch, so it
 *    overrides the persisted folder and is itself persisted.
 *  - `legacyPath` migrates the value that used to live in the shell's
 *    layout.json; wins only when our own store is empty.
 */
export function initWorkspace(
  dataDir: string,
  legacyPath: string | null,
  openFolder?: string | null,
): WorkspaceFolderState {
  store = new JsonStore<WorkspaceFolderState>(dataDir, "workspace", { path: null })
  if (openFolder && existsSync(openFolder)) {
    current = { path: normalize(openFolder) }
    store.set({ path: current.path })
    return current
  }
  const persisted = store.get().path ?? legacyPath
  current = persisted && existsSync(persisted) ? { path: normalize(persisted) } : { path: null }
  return current
}

export function getWorkspaceFolder(): WorkspaceFolderState {
  return current
}

/** Mirror the folder from the parent process (ext-host child) WITHOUT
 *  persisting — the parent owns `workspace.json`. Skips the existsSync gate
 *  (the parent already validated) and just updates state + notifies, so the
 *  child's config/storage services follow the open folder like in-process. */
export function setWorkspaceFolderMirror(path: string | null): void {
  if (path === current.path) return
  current = { path }
  for (const fn of changeListeners) fn(current.path)
}

/** Set (or clear with `null`) the open folder. Validation is the
 *  caller's job for dialogs; we still refuse paths that don't exist. */
export function setWorkspaceFolder(path: string | null): WorkspaceFolderState {
  if (path !== null && !existsSync(path)) return current
  const next = path === null ? null : normalize(path)
  const changed = next !== current.path
  current = { path: next }
  store?.set({ path: current.path })
  if (changed) {
    for (const fn of changeListeners) fn(current.path)
  }
  return current
}

// Active-editor + selection mirror (B5b-2a). The renderer (Monaco) is the
// source of truth for which document is focused and where the cursor/selection
// is; it reports over the WS and this store mirrors it so the extension host's
// `window.activeTextEditor` / `onDidChangeTextEditorSelection` reflect reality.
// Single-editor layout for now: the active doc is the visible one.

/** A selection as the renderer reports it — 0-based, anchor + active. */
export interface HostSelection {
  anchor: { line: number; character: number }
  active: { line: number; character: number }
}

let activeUri: string | null = null
const selections = new Map<string, HostSelection[]>()
const activeListeners = new Set<(uri: string | null) => void>()
const selectionListeners = new Set<(uri: string, sels: readonly HostSelection[]) => void>()

/** Renderer focused an editor (or cleared focus with `null`). */
export function setActiveEditor(uri: string | null): void {
  if (uri === activeUri) return
  activeUri = uri
  for (const fn of activeListeners) fn(uri)
}

/** Renderer's cursor/selection moved in `uri`. */
export function setEditorSelections(uri: string, sels: HostSelection[]): void {
  selections.set(uri, sels)
  for (const fn of selectionListeners) fn(uri, sels)
}

/** Drop a closed editor's tracked selection (and clear active if it was it). */
export function dropEditor(uri: string): void {
  selections.delete(uri)
  if (activeUri === uri) setActiveEditor(null)
}

export function getActiveEditor(): string | null { return activeUri }
export function getEditorSelections(uri: string): readonly HostSelection[] { return selections.get(uri) ?? [] }

export function onActiveEditorChange(fn: (uri: string | null) => void): () => void {
  activeListeners.add(fn); return () => { activeListeners.delete(fn) }
}
export function onEditorSelectionChange(fn: (uri: string, sels: readonly HostSelection[]) => void): () => void {
  selectionListeners.add(fn); return () => { selectionListeners.delete(fn) }
}

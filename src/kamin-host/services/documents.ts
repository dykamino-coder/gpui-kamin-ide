// Open-document mirror (B5). Monaco in the renderer is the source of truth for
// open editors and their (possibly dirty) contents; it reports lifecycle over
// the WS and this store mirrors it so the extension host's
// `vscode.workspace.textDocuments` / `TextDocument` can read live content
// without touching disk. Keyed by the document's fsPath.
//
// B5b: edits arrive as INCREMENTAL deltas (Monaco's content-change array), not
// the whole file — applied by offset to the stored string. The delta array is
// forwarded to listeners so `onDidChangeTextDocument.contentChanges` is real.

/** One content change, renderer→host (0-based range, matches vscode's
 *  TextDocumentContentChangeEvent minus the Range class wrapping). */
export interface DocChange {
  range: { startLine: number; startChar: number; endLine: number; endChar: number }
  rangeOffset: number
  rangeLength: number
  text: string
}

export interface DocState {
  /** Absolute fsPath — the renderer's editor key. */
  uri: string
  languageId: string
  version: number
  content: string
}

type DocListener = (doc: DocState) => void
type ChangeListener = (doc: DocState, changes: readonly DocChange[]) => void

const docs = new Map<string, DocState>()
const openListeners = new Set<DocListener>()
const changeListeners = new Set<ChangeListener>()
const closeListeners = new Set<DocListener>()
const saveListeners = new Set<DocListener>()

/** Renderer opened a document (or first reported an already-open one). */
export function syncDocOpen(doc: DocState): void {
  const stored = { ...doc }
  docs.set(doc.uri, stored)
  for (const fn of openListeners) fn(stored)
}

/** Apply incremental edits (by offset) + bump version. Changes are applied
 *  in descending-offset order so each splice leaves earlier offsets valid. */
export function syncDocChange(uri: string, changes: readonly DocChange[], version: number): void {
  const d = docs.get(uri)
  if (!d) return
  let content = d.content
  for (const c of [...changes].sort((a, b) => b.rangeOffset - a.rangeOffset)) {
    content = content.slice(0, c.rangeOffset) + c.text + content.slice(c.rangeOffset + c.rangeLength)
  }
  const updated = { ...d, content, version }
  docs.set(uri, updated)
  for (const fn of changeListeners) fn(updated, changes)
}

/** Renderer relanguaged a model (plaintext → vue/go/… once the contributed
 *  language registered). Re-fire close→open so language clients re-register the
 *  doc under its REAL language — LSP selectors like `{language:'go'}` then match.
 *  Restored tabs mount + sync their doc before the language registers, so without
 *  this they stay `plaintext` forever and get no hover/definition/completion.
 *  Editor mirror is untouched (only doc:close drops it). No-op if unchanged. */
export function syncDocSetLanguage(uri: string, languageId: string): void {
  const d = docs.get(uri)
  if (!d || d.languageId === languageId) return
  for (const fn of closeListeners) fn(d) // LSP didClose under the old (wrong) language
  const updated = { ...d, languageId }
  docs.set(uri, updated)
  for (const fn of openListeners) fn(updated) // LSP didOpen under the real language
}

export function syncDocClose(uri: string): void {
  const d = docs.get(uri)
  if (!d) return
  docs.delete(uri)
  for (const fn of closeListeners) fn(d)
}

export function syncDocSave(uri: string): void {
  const d = docs.get(uri)
  if (d) for (const fn of saveListeners) fn(d)
}

export function listDocs(): DocState[] {
  return [...docs.values()]
}

export function getDoc(uri: string): DocState | undefined {
  return docs.get(uri)
}

export function onDocOpen(fn: DocListener): () => void { openListeners.add(fn); return () => { openListeners.delete(fn) } }
export function onDocChange(fn: ChangeListener): () => void { changeListeners.add(fn); return () => { changeListeners.delete(fn) } }
export function onDocClose(fn: DocListener): () => void { closeListeners.add(fn); return () => { closeListeners.delete(fn) } }
export function onDocSave(fn: DocListener): () => void { saveListeners.add(fn); return () => { saveListeners.delete(fn) } }

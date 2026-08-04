// `vscode.window` editor surface — activeTextEditor / visibleTextEditors +
// onDidChange* (read, B5b-2a) and TextEditor.edit / revealRange +
// showTextDocument (write, B5b-2b) + setDecorations / insertSnippet / selection
// write (B5b-2c). The read side is fed by the renderer's Monaco reports through
// the EditorHost mirror; the write side reaches the renderer's Monaco via the
// EditorHost's host→renderer RPC methods.
import type { EditorHost, HostSelection, HostTextEdit } from "../host-services.js"
import { Position, Selection } from "./classes-core.js"
import { decorationOptions } from "./decoration-registry.js"
import type { NsHooks } from "./ns-builders.js"
import type { DocumentsApi } from "./ns-documents.js"
import { EventEmitter, noopEvent } from "./shared.js"
import type { TextDocument } from "./text-document.js"

function toSelection(s: HostSelection): Selection {
  return new Selection(new Position(s.anchor.line, s.anchor.character), new Position(s.active.line, s.active.character))
}

/** vscode Selection (anchor/active Positions) → host wire form. */
function toHostSelection(s: { anchor: { line: number; character: number }; active: { line: number; character: number } }): HostSelection {
  return {
    anchor: { line: s.anchor.line, character: s.anchor.character },
    active: { line: s.active.line, character: s.active.character },
  }
}

interface PosLike { line: number; character: number }
interface RangeLike { start: PosLike; end: PosLike }
// vscode locations: replace accepts Position | Range | Selection, delete accepts
// Range | Selection. Position is a zero-width range (insert at point).
type LocLike = RangeLike | PosLike

/** Reduce a vscode hoverMessage (string | MarkdownString | array) to plain text. */
function hoverText(h: unknown): string | undefined {
  if (h == null) return undefined
  if (typeof h === "string") return h || undefined
  if (Array.isArray(h)) return h.map(hoverText).filter(Boolean).join("\n") || undefined
  if (typeof h === "object" && "value" in h) {
    const v: unknown = h.value
    if (typeof v === "string") return v || undefined
  }
  return undefined
}

function posRange(p: PosLike): HostTextEdit["range"] {
  return { startLine: p.line, startChar: p.character, endLine: p.line, endChar: p.character }
}
function locRange(loc: LocLike): HostTextEdit["range"] {
  if ("start" in loc) return { startLine: loc.start.line, startChar: loc.start.character, endLine: loc.end.line, endChar: loc.end.character }
  return posRange(loc) // a bare Position → zero-width range
}

/** A vscode.TextEditorEdit that records replace/insert/delete as flat edits the
 *  renderer applies as one undo step. setEndOfLine records the requested EOL
 *  (vscode.EndOfLine: LF=1, CRLF=2) on the same transaction so the renderer
 *  changes the model's EOL as part of that one undoable edit. */
function makeEditBuilder(ops: HostTextEdit[], eolBox: { eol?: number }) {
  return {
    replace: (loc: LocLike, value: string) => { ops.push({ range: locRange(loc), text: value }) },
    insert: (pos: PosLike, value: string) => { ops.push({ range: posRange(pos), text: value }) },
    delete: (loc: LocLike) => { ops.push({ range: locRange(loc), text: "" }) },
    setEndOfLine: (eol: number) => { eolBox.eol = eol },
  }
}

/** `selection`/`selections` are LIVE getters reading the mirror, so one stable
 *  editor object per uri stays valid as the cursor moves (VS Code keeps the
 *  same TextEditor identity until the active editor changes). */
function makeTextEditor(uri: string, document: TextDocument, liveSelections: () => readonly Selection[], editorHost: EditorHost) {
  return {
    document,
    get selection(): Selection { return liveSelections()[0] ?? new Selection(0, 0, 0, 0) },
    set selection(v: { anchor: PosLike; active: PosLike }) { editorHost.setSelections(uri, [toHostSelection(v)]) },
    get selections(): readonly Selection[] { const s = liveSelections(); return s.length > 0 ? s : [new Selection(0, 0, 0, 0)] },
    set selections(v: readonly { anchor: PosLike; active: PosLike }[]) { editorHost.setSelections(uri, v.map(toHostSelection)) },
    visibleRanges: [] as Range[],
    options: { tabSize: 4, indentSize: 4, insertSpaces: true, cursorStyle: 1, lineNumbers: 1 },
    viewColumn: 1,
    // The `options` (undoStopBefore/After) arg is accepted for call-shape
    // fidelity (d.ts) but not yet forwarded to Monaco's undo grouping.
    edit: (callback: (builder: ReturnType<typeof makeEditBuilder>) => void, _options?: { undoStopBefore?: boolean; undoStopAfter?: boolean }) => {
      const ops: HostTextEdit[] = []
      const eolBox: { eol?: number } = {}
      callback(makeEditBuilder(ops, eolBox))
      return editorHost.applyEdits(uri, ops, eolBox.eol)
    },
    revealRange: (range: RangeLike, revealType?: number) => {
      void editorHost.revealRange(uri, locRange(range), revealType ?? 0)
    },
    // B5b-2c: setDecorations applies a decoration type's ranges; insertSnippet
    // inserts a tab-stop snippet at the selection (or `location`).
    setDecorations: (type: { key?: string }, rangesOrOptions: readonly (RangeLike | { range: RangeLike; hoverMessage?: unknown })[]) => {
      if (!type.key) return
      const items = (rangesOrOptions ?? []).map((r) => {
        const range = "range" in r ? locRange(r.range) : locRange(r)
        const hover = "range" in r ? hoverText(r.hoverMessage) : undefined
        return hover !== undefined ? { range, hoverMessage: hover } : { range }
      })
      editorHost.setDecorations(uri, type.key, decorationOptions(type.key), items)
    },
    insertSnippet: (snippet: { value?: string } | string, location?: LocLike | readonly LocLike[]) => {
      const value = typeof snippet === "string" ? snippet : (snippet.value ?? "")
      // d.ts allows an array (multi-cursor). We insert at the first location only;
      // taking [0] avoids feeding the array itself to locRange (→ NaN ranges).
      const loc = Array.isArray(location) ? (location as readonly LocLike[])[0] : (location as LocLike | undefined)
      return editorHost.insertSnippet(uri, value, loc ? locRange(loc) : undefined)
    },
    show: () => { /* no-op */ },
    hide: () => { /* no-op */ },
  }
}

export function buildEditors(h: NsHooks, docs: DocumentsApi) {
  const editorHost = h.workspaceHost.editors
  const docMirror = h.workspaceHost.documents
  // One stable TextEditor per uri (VS Code's identity contract). Bounded by the
  // set of opened files; selection stays live via the getter above.
  const cache = new Map<string, ReturnType<typeof makeTextEditor>>()
  const editorFor = (uri: string | null) => {
    // Honest VS Code behaviour: no editor until its document is actually open
    // (avoids caching a TextDocument that reads `undefined` on an open race).
    if (!uri || !docMirror.get(uri)) return undefined
    let editor = cache.get(uri)
    if (!editor) {
      editor = makeTextEditor(uri, docs.docFor(uri), () => editorHost.getSelections(uri).map(toSelection), editorHost)
      cache.set(uri, editor)
    }
    return editor
  }

  // NOTE: these subscriptions live for the extension's lifetime and aren't
  // disposed on unload — the same systemic namespace-disposal gap as B1's
  // onDidChangeFolder / B2's config onChange; bounded by extension count.
  const onActive = new EventEmitter<ReturnType<typeof editorFor>>()
  const onSelection = new EventEmitter<{ textEditor: ReturnType<typeof makeTextEditor>; selections: readonly Selection[]; kind: undefined }>()
  editorHost.onDidChangeActive((uri) => { onActive.fire(editorFor(uri)) })
  editorHost.onDidChangeSelection((uri, sels) => {
    const editor = editorFor(uri)
    if (editor) onSelection.fire({ textEditor: editor, selections: sels.map(toSelection), kind: undefined })
  })

  return {
    activeTextEditor: () => editorFor(editorHost.getActive()),
    visibleTextEditors: () => { const e = editorFor(editorHost.getActive()); return e ? [e] : [] },
    onDidChangeActiveTextEditor: onActive.event,
    onDidChangeTextEditorSelection: onSelection.event,
    onDidChangeVisibleTextEditors: noopEvent,
    // `window.showTextDocument(doc|uri)` — open in the editor, then return the
    // (now-mirrored) TextEditor. Accepts a Uri, a TextDocument, or an fsPath.
    showTextDocument: async (target: unknown) => {
      const uri = fsPathOf(target)
      if (!uri) throw new Error("showTextDocument: no uri")
      await editorHost.showDocument(uri)
      return editorFor(uri) ?? makeTextEditor(uri, docs.docFor(uri), () => editorHost.getSelections(uri).map(toSelection), editorHost)
    },
  }
}

/** Extract an fsPath from a Uri, a TextDocument-like `{uri}`, or a string. */
function fsPathOf(target: unknown): string | undefined {
  if (typeof target === "string") return target
  if (!target || typeof target !== "object") return undefined
  if ("fsPath" in target && typeof target.fsPath === "string") return target.fsPath
  if ("uri" in target) return fsPathOf(target.uri)
  return undefined
}

export type EditorsApi = ReturnType<typeof buildEditors>

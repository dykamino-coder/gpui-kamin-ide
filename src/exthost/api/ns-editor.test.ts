// B5b-2a — window editor surface wired to the REAL editor + document mirrors.
import { afterEach, describe, expect, it } from "vitest"
import * as docMirror from "../../kamin-host/services/documents.js"
import * as edMirror from "../../kamin-host/services/editors.js"
import type { DocumentHost, EditorHost, HostTextEdit, WorkspaceHost } from "../host-services.js"
import { Position, Range, Selection } from "./classes-core.js"
import type { NsHooks } from "./ns-builders.js"
import { buildDocuments } from "./ns-documents.js"
import { buildEditors } from "./ns-editor.js"

// Captured host→renderer write calls (B5b-2b).
const writes: { applyEdits: { uri: string; edits: readonly HostTextEdit[]; eol: number | undefined }[]; reveal: unknown[]; shown: string[] } = { applyEdits: [], reveal: [], shown: [] }

const documentHost: DocumentHost = {
  list: () => docMirror.listDocs(),
  get: (uri) => docMirror.getDoc(uri),
  open: (d) => { docMirror.syncDocOpen(d) },
  onDidOpen: (fn) => docMirror.onDocOpen(fn),
  onDidChange: (fn) => docMirror.onDocChange(fn),
  onDidClose: (fn) => docMirror.onDocClose(fn),
  onDidSave: (fn) => docMirror.onDocSave(fn),
}
const editorHost: EditorHost = {
  getActive: () => edMirror.getActiveEditor(),
  getSelections: (uri) => edMirror.getEditorSelections(uri),
  onDidChangeActive: (fn) => edMirror.onActiveEditorChange(fn),
  onDidChangeSelection: (fn) => edMirror.onEditorSelectionChange(fn),
  applyEdits: (uri, edits, eol) => { writes.applyEdits.push({ uri, edits, eol }); return Promise.resolve(true) },
  revealRange: (uri, range, revealType) => { writes.reveal.push({ uri, range, revealType }); return Promise.resolve() },
  showDocument: (uri) => { writes.shown.push(uri); docMirror.syncDocOpen({ uri, languageId: "ts", version: 1, content: "shown" }); return Promise.resolve() },
  setDecorations: () => { /* B5b-2c — not asserted here */ },
  disposeDecorationType: () => { /* B5b-2c */ },
  insertSnippet: () => Promise.resolve(true),
  setSelections: () => { /* B5b-2c — not asserted here */ },
}
const workspaceHost = { documents: documentHost, editors: editorHost } as unknown as WorkspaceHost
const hooks = { workspaceHost } as unknown as NsHooks
const docs = buildDocuments(hooks)
const editors = buildEditors(hooks, docs)

const PATH = process.platform === "win32" ? "C:\\p\\a.ts" : "/p/a.ts"

afterEach(() => {
  edMirror.setActiveEditor(null)
  for (const d of docMirror.listDocs()) { docMirror.syncDocClose(d.uri); edMirror.dropEditor(d.uri) }
  writes.applyEdits.length = 0; writes.reveal.length = 0; writes.shown.length = 0
})

describe("window editor surface (B5b-2a)", () => {
  it("activeTextEditor reflects the active doc + its document identity matches textDocuments", () => {
    expect(editors.activeTextEditor()).toBeUndefined()
    docMirror.syncDocOpen({ uri: PATH, languageId: "typescript", version: 1, content: "let a = 1" })
    edMirror.setActiveEditor(PATH)
    const ed = editors.activeTextEditor()
    expect(ed?.document.getText()).toBe("let a = 1")
    expect(ed?.document).toBe(docs.list()[0]) // shared identity
    expect(editors.visibleTextEditors()).toHaveLength(1)
  })

  it("selections map to Selection (0-based anchor/active)", () => {
    docMirror.syncDocOpen({ uri: PATH, languageId: "typescript", version: 1, content: "abcdef" })
    edMirror.setActiveEditor(PATH)
    edMirror.setEditorSelections(PATH, [{ anchor: { line: 0, character: 1 }, active: { line: 0, character: 4 } }])
    const ed = editors.activeTextEditor()
    expect(ed?.selection).toBeInstanceOf(Selection)
    expect(ed?.selection.anchor.character).toBe(1)
    expect(ed?.selection.active.character).toBe(4)
  })

  it("fires onDidChangeActiveTextEditor + onDidChangeTextEditorSelection", () => {
    docMirror.syncDocOpen({ uri: PATH, languageId: "typescript", version: 1, content: "x" })
    let activeText: string | undefined
    let selChar = -1
    editors.onDidChangeActiveTextEditor((ed) => { activeText = ed?.document.getText() })
    editors.onDidChangeTextEditorSelection((e) => { selChar = e.selections[0]?.active.character ?? -1 })
    edMirror.setActiveEditor(PATH)
    expect(activeText).toBe("x")
    edMirror.setEditorSelections(PATH, [{ anchor: { line: 0, character: 0 }, active: { line: 0, character: 1 } }])
    expect(selChar).toBe(1)
  })

  it("keeps a stable TextEditor identity across reads + events; selection stays live", () => {
    docMirror.syncDocOpen({ uri: PATH, languageId: "typescript", version: 1, content: "abcdef" })
    edMirror.setActiveEditor(PATH)
    let eventEditor: unknown
    editors.onDidChangeTextEditorSelection((e) => { eventEditor = e.textEditor })
    const a = editors.activeTextEditor()
    expect(editors.activeTextEditor()).toBe(a) // same identity across reads
    edMirror.setEditorSelections(PATH, [{ anchor: { line: 0, character: 2 }, active: { line: 0, character: 3 } }])
    expect(eventEditor).toBe(a) // event carries the SAME editor object
    expect(a?.selection.active.character).toBe(3) // live selection on the cached editor
  })

  it("returns undefined when active is set before the document opens (race)", () => {
    edMirror.setActiveEditor(PATH) // no syncDocOpen yet
    expect(editors.activeTextEditor()).toBeUndefined()
  })

  it("clears active when the editor is dropped", () => {
    docMirror.syncDocOpen({ uri: PATH, languageId: "typescript", version: 1, content: "" })
    edMirror.setActiveEditor(PATH)
    expect(editors.activeTextEditor()).toBeDefined()
    edMirror.dropEditor(PATH)
    expect(editors.activeTextEditor()).toBeUndefined()
  })
})

describe("window editor write surface (B5b-2b)", () => {
  it("edit() builds replace/insert/delete ops and forwards them to the host", async () => {
    docMirror.syncDocOpen({ uri: PATH, languageId: "typescript", version: 1, content: "abcdef" })
    edMirror.setActiveEditor(PATH)
    const ed = editors.activeTextEditor()
    const ok = await ed?.edit((b) => {
      b.replace(new Range(0, 0, 0, 3), "XYZ")
      b.insert(new Position(0, 6), "!")
      b.delete(new Range(1, 0, 1, 2))
    })
    expect(ok).toBe(true)
    expect(writes.applyEdits).toHaveLength(1)
    expect(writes.applyEdits[0]?.edits).toEqual([
      { range: { startLine: 0, startChar: 0, endLine: 0, endChar: 3 }, text: "XYZ" },
      { range: { startLine: 0, startChar: 6, endLine: 0, endChar: 6 }, text: "!" },
      { range: { startLine: 1, startChar: 0, endLine: 1, endChar: 2 }, text: "" },
    ])
  })

  it("edit() forwards setEndOfLine's EOL on the same transaction", async () => {
    docMirror.syncDocOpen({ uri: PATH, languageId: "typescript", version: 1, content: "a\nb" })
    edMirror.setActiveEditor(PATH)
    const ok = await editors.activeTextEditor()?.edit((b) => {
      b.insert(new Position(0, 0), "x")
      b.setEndOfLine(2) // vscode.EndOfLine.CRLF
    })
    expect(ok).toBe(true)
    expect(writes.applyEdits[0]?.eol).toBe(2)
  })

  it("edit() leaves eol undefined when setEndOfLine isn't called", async () => {
    docMirror.syncDocOpen({ uri: PATH, languageId: "typescript", version: 1, content: "a" })
    edMirror.setActiveEditor(PATH)
    await editors.activeTextEditor()?.edit((b) => { b.insert(new Position(0, 0), "x") })
    expect(writes.applyEdits[0]?.eol).toBeUndefined()
  })

  it("revealRange() forwards the range + reveal type", () => {
    docMirror.syncDocOpen({ uri: PATH, languageId: "typescript", version: 1, content: "abc" })
    edMirror.setActiveEditor(PATH)
    editors.activeTextEditor()?.revealRange(new Range(2, 1, 2, 4), 1)
    expect(writes.reveal).toEqual([{ uri: PATH, range: { startLine: 2, startChar: 1, endLine: 2, endChar: 4 }, revealType: 1 }])
  })

  it("showTextDocument(uri) opens the doc and returns its editor", async () => {
    const ed = await editors.showTextDocument(PATH)
    expect(writes.shown).toEqual([PATH])
    expect(ed.document.getText()).toBe("shown")
  })
})

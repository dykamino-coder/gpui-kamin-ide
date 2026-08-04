// B5b — workspace document surface wired to the REAL host mirror: open →
// textDocuments + onDidOpen, delta → onDidChangeTextDocument with real
// contentChanges, close → onDidClose.
import { afterEach, describe, expect, it } from "vitest"
import * as mirror from "../../kamin-host/services/documents.js"
import type { DocumentHost, WorkspaceHost } from "../host-services.js"
import { Range } from "./classes-core.js"
import type { NsHooks } from "./ns-builders.js"
import { buildDocuments } from "./ns-documents.js"

const docHost: DocumentHost = {
  list: () => mirror.listDocs(),
  get: (uri) => mirror.getDoc(uri),
  open: (d) => { mirror.syncDocOpen(d) },
  onDidOpen: (fn) => mirror.onDocOpen(fn),
  onDidChange: (fn) => mirror.onDocChange(fn),
  onDidClose: (fn) => mirror.onDocClose(fn),
  onDidSave: (fn) => mirror.onDocSave(fn),
}
const workspaceHost = { documents: docHost, fs: {} } as unknown as WorkspaceHost
const docs = buildDocuments({ workspaceHost } as unknown as NsHooks)

afterEach(() => { for (const d of mirror.listDocs()) mirror.syncDocClose(d.uri) })

describe("workspace documents (B5b)", () => {
  it("open surfaces a TextDocument in textDocuments + fires onDidOpen", () => {
    let opened = ""
    docs.onDidOpen((d) => { opened = d.uri.fsPath })
    mirror.syncDocOpen({ uri: "/p/x.ts", languageId: "typescript", version: 1, content: "let a = 1" })
    const list = docs.list()
    expect(list).toHaveLength(1)
    expect(list[0]?.getText()).toBe("let a = 1")
    expect(list[0]?.languageId).toBe("typescript")
    expect(opened.endsWith("x.ts")).toBe(true)
  })

  it("delta fires onDidChangeTextDocument with real contentChanges + new text", () => {
    mirror.syncDocOpen({ uri: "/p/y.ts", languageId: "typescript", version: 1, content: "abc" })
    let text = ""
    let version = 0
    let changes: readonly unknown[] = []
    docs.onDidChange((e) => { text = e.document.getText(); version = e.document.version; changes = e.contentChanges })
    mirror.syncDocChange("/p/y.ts", [{ range: { startLine: 0, startChar: 1, endLine: 0, endChar: 2 }, rangeOffset: 1, rangeLength: 1, text: "X" }], 2)
    expect(text).toBe("aXc")
    expect(version).toBe(2)
    expect(changes).toHaveLength(1)
    const ch = changes[0] as { range: Range; text: string }
    expect(ch.text).toBe("X")
    expect(ch.range).toBeInstanceOf(Range)
    expect(ch.range.isEqual(new Range(0, 1, 0, 2))).toBe(true)
  })

  it("close drops it from textDocuments + fires onDidClose", () => {
    mirror.syncDocOpen({ uri: "/p/z.ts", languageId: "typescript", version: 1, content: "" })
    let closed = false
    docs.onDidClose(() => { closed = true })
    expect(docs.list()).toHaveLength(1)
    mirror.syncDocClose("/p/z.ts")
    expect(docs.list()).toHaveLength(0)
    expect(closed).toBe(true)
  })

  it("the same uri yields a stable TextDocument identity", () => {
    mirror.syncDocOpen({ uri: "/p/id.ts", languageId: "typescript", version: 1, content: "" })
    expect(docs.list()[0]).toBe(docs.list()[0])
  })
})

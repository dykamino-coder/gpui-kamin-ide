// B5b — host document mirror applies incremental deltas by offset.
import { afterEach, describe, expect, it, vi } from "vitest"
import {
  type DocChange, getDoc, listDocs, onDocChange, syncDocChange, syncDocClose, syncDocOpen,
} from "./documents.js"

function change(rangeOffset: number, rangeLength: number, text: string): DocChange {
  return { range: { startLine: 0, startChar: 0, endLine: 0, endChar: 0 }, rangeOffset, rangeLength, text }
}

afterEach(() => {
  // The mirror is a module singleton — clear any docs between tests.
  for (const d of listDocs()) syncDocClose(d.uri)
})

describe("document mirror (B5b)", () => {
  it("applies a single insert/replace by offset + bumps version", () => {
    syncDocOpen({ uri: "/a", languageId: "ts", version: 1, content: "abc" })
    syncDocChange("/a", [change(1, 1, "X")], 2) // replace 'b' with 'X'
    expect(getDoc("/a")?.content).toBe("aXc")
    expect(getDoc("/a")?.version).toBe(2)
  })

  it("applies multiple changes in descending-offset order (offsets stay valid)", () => {
    syncDocOpen({ uri: "/b", languageId: "ts", version: 1, content: "0123456789" })
    // Two inserts given in ASCENDING order; applied descending so neither shifts the other.
    syncDocChange("/b", [change(2, 0, "AA"), change(6, 0, "BB")], 2)
    expect(getDoc("/b")?.content).toBe("01AA2345BB6789")
  })

  it("fires onDidChange with the document and the raw change array", () => {
    const seen = vi.fn()
    const off = onDocChange(seen)
    syncDocOpen({ uri: "/c", languageId: "ts", version: 1, content: "x" })
    const chs = [change(1, 0, "y")]
    syncDocChange("/c", chs, 2)
    expect(seen).toHaveBeenCalledOnce()
    const [doc, changes] = seen.mock.calls[0] as [{ content: string }, DocChange[]]
    expect(doc.content).toBe("xy")
    expect(changes).toEqual(chs)
    off()
  })

  it("ignores changes for an unknown document", () => {
    expect(() => { syncDocChange("/missing", [change(0, 0, "z")], 2) }).not.toThrow()
    expect(getDoc("/missing")).toBeUndefined()
  })
})

// B5 — TextDocument line/offset maths over mirrored content.
import { describe, expect, it } from "vitest"
import { URI } from "vscode-uri"
import type { HostDocument } from "../host-services.js"
import { Position, Range } from "./classes-core.js"
import { TextDocument } from "./text-document.js"

function doc(content: string, languageId = "typescript"): TextDocument {
  const state: HostDocument = { uri: "/p/a.ts", languageId, version: 1, content }
  return new TextDocument(URI.file("/p/a.ts"), () => state)
}

describe("TextDocument basics", () => {
  it("exposes uri/fileName/languageId/version/lineCount", () => {
    const d = doc("a\nb\nc")
    expect(d.languageId).toBe("typescript")
    expect(d.version).toBe(1)
    expect(d.lineCount).toBe(3)
    expect(d.fileName.replace(/\\/g, "/")).toBe(URI.file("/p/a.ts").fsPath.replace(/\\/g, "/"))
    expect(d.isClosed).toBe(false)
  })
  it("reports closed when the mirror drops it", () => {
    const d = new TextDocument(URI.file("/p/x.ts"), () => undefined)
    expect(d.isClosed).toBe(true)
    expect(d.getText()).toBe("")
    expect(d.lineCount).toBe(1)
  })
})

describe("TextDocument line + offset maths (LF)", () => {
  const d = doc("const x = 1\nlet y = 2\n")
  it("lineAt returns text, range, first-non-ws", () => {
    const l = d.lineAt(0)
    expect(l.text).toBe("const x = 1")
    expect(l.range.isEqual(new Range(0, 0, 0, 11))).toBe(true)
    expect(l.firstNonWhitespaceCharacterIndex).toBe(0)
    expect(d.lineAt(2).isEmptyOrWhitespace).toBe(true)
  })
  it("offsetAt / positionAt round-trip", () => {
    const pos = new Position(1, 4)
    const off = d.offsetAt(pos)
    expect(off).toBe("const x = 1\n".length + 4)
    expect(d.positionAt(off).isEqual(pos)).toBe(true)
  })
  it("getText(range) slices correctly", () => {
    expect(d.getText(new Range(0, 6, 0, 7))).toBe("x")
    expect(d.getText(new Range(0, 0, 1, 3))).toBe("const x = 1\nlet")
  })
  it("getWordRangeAtPosition finds the word under the cursor", () => {
    const r = d.getWordRangeAtPosition(new Position(0, 7))
    expect(r?.isEqual(new Range(0, 6, 0, 7))).toBe(true)
    expect(d.getWordRangeAtPosition(new Position(0, 8))).toBeUndefined() // the "=" sign
  })
})

describe("TextDocument CRLF handling", () => {
  const d = doc("a\r\nbb\r\nccc")
  it("detects CRLF eol and computes offsets with 2-char breaks", () => {
    expect(d.eol).toBe(2)
    expect(d.lineAt(1).text).toBe("bb")
    expect(d.offsetAt(new Position(2, 0))).toBe("a\r\n".length + "bb\r\n".length)
    expect(d.positionAt("a\r\n".length).isEqual(new Position(1, 0))).toBe(true)
  })
  it("clamps an offset landing inside a CRLF break to end-of-line", () => {
    // "a\r\nbb..." — offset 2 is the `\n`; must clamp to end of line 0, not (1,-1)
    expect(d.positionAt(2).isEqual(new Position(0, 1))).toBe(true)
    expect(d.positionAt(9999).isEqual(new Position(2, 3))).toBe(true) // beyond EOF
  })
})

describe("TextDocument validation", () => {
  const d = doc("a\nbb")
  it("clamps out-of-range positions", () => {
    expect(d.validatePosition(new Position(99, 99)).isEqual(new Position(1, 2))).toBe(true)
    expect(d.validatePosition(new Position(0, 99)).isEqual(new Position(0, 1))).toBe(true)
  })
})

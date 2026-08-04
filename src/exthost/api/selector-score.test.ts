import { describe, expect, it } from "vitest"
import { scoreSelector } from "./selector-score.js"

describe("scoreSelector (vscode.languages.match)", () => {
  // The languageclient gates didOpen/didChange on a positive score; these are
  // the cases that decide whether an LSP server ever sees the document.
  it("scores exact string / wildcard / mismatch", () => {
    expect(scoreSelector("go", "go", "file")).toBe(10)
    expect(scoreSelector("*", "go", "file")).toBe(5)
    expect(scoreSelector("python", "go", "file")).toBe(0)
  })

  it("scores a DocumentFilter on language + scheme (gopls' selector)", () => {
    const sel = { language: "go", scheme: "file" }
    expect(scoreSelector(sel, "go", "file")).toBe(10)
    expect(scoreSelector(sel, "go", "untitled")).toBe(0) // scheme mismatch disqualifies
    expect(scoreSelector(sel, "rust", "file")).toBe(0) // language mismatch disqualifies
  })

  it("takes the best score across a selector array", () => {
    expect(scoreSelector([{ language: "ts" }, { language: "go" }], "go", "file")).toBe(10)
    expect(scoreSelector([{ language: "ts" }, "*"], "go", "file")).toBe(5)
    expect(scoreSelector([{ language: "ts" }, { language: "py" }], "go", "file")).toBe(0)
  })

  it("treats a scheme-`*` filter as a partial (5) match", () => {
    expect(scoreSelector({ language: "go", scheme: "*" }, "go", "file")).toBe(10)
    expect(scoreSelector({ scheme: "*" }, "go", "file")).toBe(5)
  })
})

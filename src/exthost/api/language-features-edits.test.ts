// B6l–B6n: edit/token-producing language features (rename, code actions,
// semantic tokens). Split from language-features.test.ts to keep each test file
// under the 250-LOC ceiling.
import { describe, expect, it } from "vitest"
import { URI } from "vscode-uri"
import { Location, Range, WorkspaceEdit } from "./classes-core.js"
import { CodeAction, CodeActionKind, SemanticTokens, SymbolInformation } from "./classes-lang.js"
import { SymbolKind } from "./enums.js"
import { LanguageFeatures } from "./language-features.js"
import { TextDocument } from "./text-document.js"

function docFor(content: string, languageId: string) {
  return (uri: string) => new TextDocument(URI.file(uri), () => ({ uri, languageId, version: 1, content }))
}

describe("LanguageFeatures — rename / code actions / semantic tokens (B6l–B6n)", () => {
  it("collects a WorkspaceEdit from a rename provider — B6l", async () => {
    const lf = new LanguageFeatures()
    lf.registerRenameProvider("ts", {
      provideRenameEdits: (doc, _pos, newName) => {
        const edit = new WorkspaceEdit()
        edit.replace(doc.uri, new Range(0, 0, 0, 6), newName)
        return edit
      },
    }, docFor("x", "ts"))
    const dto = await lf.provideRename("/a.ts", "ts", 0, 2, "renamed")
    expect(dto?.edits).toHaveLength(1)
    expect(dto?.edits[0]?.textEdit).toEqual({ range: { startLine: 0, startChar: 0, endLine: 0, endChar: 6 }, newText: "renamed" })
    expect(dto?.edits[0]?.resource.replace(/\\/g, "/")).toContain("a.ts")
  })

  it("normalizes code actions (kind + edit) — B6m", async () => {
    const lf = new LanguageFeatures()
    lf.registerCodeActionsProvider("ts", {
      provideCodeActions: (doc) => {
        const action = new CodeAction("Fix it", CodeActionKind.QuickFix)
        ;(action as { isPreferred?: boolean }).isPreferred = true
        const edit = new WorkspaceEdit()
        edit.replace(doc.uri, new Range(0, 0, 0, 6), "FIXED")
        ;(action as { edit?: unknown }).edit = edit
        return [action]
      },
    }, docFor("x", "ts"))
    const actions = await lf.provideCodeActions("/a.ts", "ts", { startLine: 0, startChar: 0, endLine: 0, endChar: 6 })
    expect(actions[0]?.title).toBe("Fix it")
    expect(actions[0]?.kind).toBe("quickfix")
    expect(actions[0]?.isPreferred).toBe(true)
    expect(actions[0]?.edit?.edits[0]?.textEdit.newText).toBe("FIXED")
  })

  it("remaps semantic tokens to the standard legend, dropping unknown types + re-deltaing — B6n", async () => {
    const lf = new LanguageFeatures()
    // provider legend: variable→std 8, unknownType→dropped, keyword→std 19; readonly→std mod 2.
    const legend = { tokenTypes: ["variable", "unknownType", "keyword"], tokenModifiers: ["readonly", "unknownMod"] }
    const data = new Uint32Array([0, 0, 5, 0, 0b01, 0, 6, 3, 1, 0, 1, 2, 4, 2, 0])
    lf.registerDocumentSemanticTokensProvider("ts", { provideDocumentSemanticTokens: () => new SemanticTokens(data) }, legend, docFor("x", "ts"))
    const dto = await lf.provideDocumentSemanticTokens("/a.ts", "ts")
    // token1 variable+readonly kept; token2 unknownType dropped; token3 keyword re-deltaed from token1.
    expect(dto?.data).toEqual([0, 0, 5, 8, 0b100, 1, 2, 4, 19, 0])
  })

  it("merges workspace symbols across providers (Go to Symbol, Ctrl+T)", async () => {
    const lf = new LanguageFeatures()
    lf.registerWorkspaceSymbolProvider({
      provideWorkspaceSymbols: (q) => [new SymbolInformation(`sym:${q}`, SymbolKind.Function, "Mod", new Location(URI.file("/t/a.ts"), new Range(1, 0, 1, 4)))],
    })
    const syms = await lf.provideWorkspaceSymbols("foo")
    expect(syms[0]).toMatchObject({ name: "sym:foo", kind: SymbolKind.Function, containerName: "Mod", range: { startLine: 1, startChar: 0, endLine: 1, endChar: 4 } })
    expect(syms[0]?.uri.replace(/\\/g, "/")).toContain("a.ts")
  })
})

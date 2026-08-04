// B6 — language-feature registry: completion providers across extensions,
// selector matching, result normalization.
import { describe, expect, it } from "vitest"
import { URI } from "vscode-uri"
import { Location, Position, Range } from "./classes-core.js"
import { CodeLens, Color, ColorInformation, ColorPresentation, DocumentLink, DocumentSymbol, InlayHint, ParameterInformation, SelectionRange, SignatureHelp, SignatureInformation } from "./classes-lang.js"
import { CompletionItemKind, InlayHintKind, SymbolKind } from "./enums.js"
import { LanguageFeatures } from "./language-features.js"
import { TextDocument } from "./text-document.js"

function docFor(content: string, languageId: string) {
  return (uri: string) => new TextDocument(URI.file(uri), () => ({ uri, languageId, version: 1, content }))
}

describe("LanguageFeatures — completion", () => {
  it("invokes a matching provider and normalizes items to DTOs", async () => {
    const lf = new LanguageFeatures()
    lf.registerCompletionItemProvider("typescript", {
      provideCompletionItems: () => [
        { label: "foo", kind: CompletionItemKind.Function, detail: "fn", insertText: "foo()" },
        "bar", // bare string → label-only item
      ],
    }, docFor("x", "typescript"))
    const { items } = await lf.provideCompletionItems("/a.ts", "typescript", 0, 0)
    expect(items).toHaveLength(2)
    expect(items[0]).toMatchObject({ label: "foo", detail: "fn", insertText: "foo()" })
    expect(items[1]?.label).toBe("bar")
  })

  it("captures CompletionItemLabel detail/description + snippet insertText (#20)", async () => {
    const lf = new LanguageFeatures()
    lf.registerCompletionItemProvider("typescript", {
      provideCompletionItems: () => [
        { label: { label: "map", detail: "(fn)", description: "Array" }, insertText: { value: "map(${1:fn})" } },
      ],
    }, docFor("x", "typescript"))
    const { items } = await lf.provideCompletionItems("/a.ts", "typescript", 0, 0)
    expect(items[0]).toMatchObject({
      label: "map", labelDetail: "(fn)", labelDescription: "Array",
      insertText: "map(${1:fn})", insertTextIsSnippet: true,
    })
  })

  it("skips providers whose selector does not match the language", async () => {
    const lf = new LanguageFeatures()
    lf.registerCompletionItemProvider("python", { provideCompletionItems: () => [{ label: "py" }] }, docFor("x", "python"))
    lf.registerCompletionItemProvider({ language: "typescript" }, { provideCompletionItems: () => [{ label: "ts" }] }, docFor("x", "typescript"))
    const { items } = await lf.provideCompletionItems("/a.ts", "typescript", 0, 0)
    expect(items.map((i) => i.label)).toEqual(["ts"])
  })

  it("honors DocumentFilter.scheme (file docs ≠ untitled) — #18", async () => {
    const lf = new LanguageFeatures()
    lf.registerCompletionItemProvider({ scheme: "untitled" }, { provideCompletionItems: () => [{ label: "untitled" }] }, docFor("x", "ts"))
    lf.registerCompletionItemProvider({ scheme: "file" }, { provideCompletionItems: () => [{ label: "file" }] }, docFor("x", "ts"))
    lf.registerCompletionItemProvider({ language: "ts", scheme: "file" }, { provideCompletionItems: () => [{ label: "tsfile" }] }, docFor("x", "ts"))
    const { items } = await lf.provideCompletionItems("/a.ts", "ts", 0, 0)
    // editor docs are file-scheme: the untitled-only provider must NOT fire.
    expect(items.map((i) => i.label)).toEqual(["file", "tsfile"])
  })

  it("supports the `*` selector and propagates CompletionList.isIncomplete", async () => {
    const lf = new LanguageFeatures()
    lf.registerCompletionItemProvider("*", { provideCompletionItems: () => ({ items: [{ label: "any" }], isIncomplete: true }) }, docFor("x", "rust"))
    const result = await lf.provideCompletionItems("/a.rs", "rust", 0, 0)
    expect(result.items.map((i) => i.label)).toEqual(["any"])
    expect(result.isIncomplete).toBe(true)
  })

  it("forwards the trigger context to the provider", async () => {
    const lf = new LanguageFeatures()
    let seen: unknown
    lf.registerCompletionItemProvider("ts", { provideCompletionItems: (_d, _p, _t, ctx) => { seen = ctx; return [] } }, docFor("x", "ts"))
    await lf.provideCompletionItems("/a", "ts", 0, 0, 1, ".")
    expect(seen).toEqual({ triggerKind: 1, triggerCharacter: "." })
  })

  it("isolates a throwing provider (others still return)", async () => {
    const lf = new LanguageFeatures()
    lf.registerCompletionItemProvider("ts", { provideCompletionItems: () => { throw new Error("boom") } }, docFor("x", "ts"))
    lf.registerCompletionItemProvider("ts", { provideCompletionItems: () => [{ label: "ok" }] }, docFor("x", "ts"))
    const { items } = await lf.provideCompletionItems("/a", "ts", 0, 0)
    expect(items.map((i) => i.label)).toEqual(["ok"])
  })

  it("dispose() unregisters the provider", async () => {
    const lf = new LanguageFeatures()
    const sub = lf.registerCompletionItemProvider("ts", { provideCompletionItems: () => [{ label: "x" }] }, docFor("c", "ts"))
    sub.dispose()
    expect((await lf.provideCompletionItems("/a", "ts", 0, 0)).items).toHaveLength(0)
  })
})

describe("LanguageFeatures — hover", () => {
  it("normalizes MarkdownString/string contents + range", async () => {
    const lf = new LanguageFeatures()
    lf.registerHoverProvider("ts", {
      provideHover: () => ({ contents: ["plain", { value: "**md**" }], range: new Range(0, 1, 0, 4) }),
    }, docFor("x", "ts"))
    const hovers = await lf.provideHover("/a.ts", "ts", 0, 2)
    expect(hovers).toHaveLength(1)
    expect(hovers[0]?.contents).toEqual(["plain", "**md**"])
    expect(hovers[0]?.range).toEqual({ startLine: 0, startChar: 1, endLine: 0, endChar: 4 })
  })

  it("wraps MarkedString {language,value} as a fenced code block", async () => {
    const lf = new LanguageFeatures()
    lf.registerHoverProvider("ts", {
      provideHover: () => ({ contents: [{ language: "typescript", value: "const x = 1" }] }),
    }, docFor("x", "ts"))
    const hovers = await lf.provideHover("/a.ts", "ts", 0, 0)
    expect(hovers[0]?.contents).toEqual(["```typescript\nconst x = 1\n```"])
  })
})

describe("LanguageFeatures — definition", () => {
  it("normalizes Location and LocationLink shapes", async () => {
    const lf = new LanguageFeatures()
    lf.registerDefinitionProvider("ts", {
      provideDefinition: () => ({ uri: URI.file("/t/def.ts"), range: new Range(2, 0, 2, 5) }),
    }, docFor("x", "ts"))
    lf.registerDefinitionProvider("ts", {
      provideDefinition: () => [{ targetUri: URI.file("/t/link.ts"), targetRange: new Range(9, 0, 9, 1) }],
    }, docFor("x", "ts"))
    const defs = await lf.provideDefinition("/a.ts", "ts", 0, 0)
    expect(defs).toHaveLength(2)
    expect(defs[0]?.uri.replace(/\\/g, "/")).toContain("def.ts")
    expect(defs[0]?.range).toEqual({ startLine: 2, startChar: 0, endLine: 2, endChar: 5 })
    expect(defs[1]?.uri.replace(/\\/g, "/")).toContain("link.ts")
  })

  it("forwards LocationLink.targetSelectionRange", async () => {
    const lf = new LanguageFeatures()
    lf.registerDefinitionProvider("ts", {
      provideDefinition: () => [{ targetUri: URI.file("/t/x.ts"), targetRange: new Range(0, 0, 9, 0), targetSelectionRange: new Range(3, 2, 3, 8) }],
    }, docFor("x", "ts"))
    const defs = await lf.provideDefinition("/a.ts", "ts", 0, 0)
    expect(defs[0]?.targetSelectionRange).toEqual({ startLine: 3, startChar: 2, endLine: 3, endChar: 8 })
  })
})

describe("LanguageFeatures — references / highlight / folding (B6e)", () => {
  it("merges references from all matching providers + forwards includeDeclaration", async () => {
    const lf = new LanguageFeatures()
    let seenCtx: unknown
    lf.registerReferenceProvider("ts", {
      provideReferences: (_d, _p, ctx) => { seenCtx = ctx; return [{ uri: URI.file("/t/r.ts"), range: new Range(1, 0, 1, 3) }] },
    }, docFor("x", "ts"))
    const refs = await lf.provideReferences("/a.ts", "ts", 0, 0, true)
    expect(refs).toHaveLength(1)
    expect(refs[0]?.range).toEqual({ startLine: 1, startChar: 0, endLine: 1, endChar: 3 })
    expect(seenCtx).toEqual({ includeDeclaration: true })
  })

  it("normalizes document highlights with kind", async () => {
    const lf = new LanguageFeatures()
    lf.registerDocumentHighlightProvider("ts", {
      provideDocumentHighlights: () => [{ range: new Range(0, 0, 0, 4), kind: 2 }],
    }, docFor("x", "ts"))
    const hits = await lf.provideDocumentHighlights("/a.ts", "ts", 0, 1)
    expect(hits).toEqual([{ range: { startLine: 0, startChar: 0, endLine: 0, endChar: 4 }, kind: 2 }])
  })

  it("merges folding ranges (0-based lines), carries kind, ignores malformed entries", async () => {
    const lf = new LanguageFeatures()
    lf.registerFoldingRangeProvider("*", {
      provideFoldingRanges: () => [{ start: 0, end: 2, kind: 1 }, { start: "x" }, { start: 4, end: 6 }],
    }, docFor("x", "ts"))
    const ranges = await lf.provideFoldingRanges("/a.ts", "ts")
    expect(ranges).toEqual([{ start: 0, end: 2, kind: 1 }, { start: 4, end: 6 }])
  })

  it("resolves declaration / typeDefinition / implementation as Locations (B6f)", async () => {
    const lf = new LanguageFeatures()
    lf.registerDeclarationProvider("ts", { provideDeclaration: () => new Location(URI.file("/t/d.ts"), new Range(1, 0, 1, 1)) }, docFor("x", "ts"))
    lf.registerTypeDefinitionProvider("ts", { provideTypeDefinition: () => [{ uri: URI.file("/t/td.ts"), range: new Range(2, 0, 2, 1) }] }, docFor("x", "ts"))
    lf.registerImplementationProvider("ts", { provideImplementation: () => new Location(URI.file("/t/i.ts"), new Range(3, 0, 3, 1)) }, docFor("x", "ts"))
    const decl = await lf.provideDeclaration("/a.ts", "ts", 0, 0)
    const typeDef = await lf.provideTypeDefinition("/a.ts", "ts", 0, 0)
    const impl = await lf.provideImplementation("/a.ts", "ts", 0, 0)
    expect(decl[0]?.uri.replace(/\\/g, "/")).toContain("d.ts")
    expect(decl[0]?.range).toEqual({ startLine: 1, startChar: 0, endLine: 1, endChar: 1 })
    expect(typeDef[0]?.uri.replace(/\\/g, "/")).toContain("td.ts")
    expect(impl[0]?.range).toEqual({ startLine: 3, startChar: 0, endLine: 3, endChar: 1 })
  })

  it("normalizes signature help (first provider wins) — B6g", async () => {
    const lf = new LanguageFeatures()
    const sig = new SignatureInformation("greet(name, loud)", "doc")
    sig.parameters = [new ParameterInformation("name"), new ParameterInformation([6, 10])]
    const help = new SignatureHelp()
    help.signatures = [sig]; help.activeParameter = 1
    lf.registerSignatureHelpProvider("ts", { provideSignatureHelp: () => help }, docFor("x", "ts"))
    const dto = await lf.provideSignatureHelp("/a.ts", "ts", 0, 6, "(")
    expect(dto?.activeParameter).toBe(1)
    expect(dto?.signatures[0]?.label).toBe("greet(name, loud)")
    expect(dto?.signatures[0]?.parameters).toEqual([{ label: "name" }, { label: [6, 10] }])
  })

  it("returns null when no signature provider yields signatures", async () => {
    const lf = new LanguageFeatures()
    lf.registerSignatureHelpProvider("ts", { provideSignatureHelp: () => undefined }, docFor("x", "ts"))
    expect(await lf.provideSignatureHelp("/a.ts", "ts", 0, 0)).toBeNull()
  })

  it("normalizes document symbols (nested) — B6h", async () => {
    const lf = new LanguageFeatures()
    const mod = new DocumentSymbol("Mod", "d", SymbolKind.Module, new Range(0, 0, 4, 0), new Range(0, 0, 0, 3))
    mod.children = [new DocumentSymbol("fn", "", SymbolKind.Function, new Range(1, 0, 1, 2), new Range(1, 0, 1, 2))]
    lf.registerDocumentSymbolProvider("ts", { provideDocumentSymbols: () => [mod] }, docFor("x", "ts"))
    const syms = await lf.provideDocumentSymbols("/a.ts", "ts")
    expect(syms[0]).toMatchObject({ name: "Mod", kind: SymbolKind.Module, range: { startLine: 0, endLine: 4 } })
    expect(syms[0]?.children?.[0]).toMatchObject({ name: "fn", kind: SymbolKind.Function })
  })

  it("normalizes document links (Uri target → url) — B6h", async () => {
    const lf = new LanguageFeatures()
    const link = new DocumentLink(new Range(0, 0, 0, 4), URI.parse("https://example.com/x"))
    lf.registerDocumentLinkProvider("ts", { provideDocumentLinks: () => [link] }, docFor("x", "ts"))
    const links = await lf.provideDocumentLinks("/a.ts", "ts")
    expect(links[0]?.range).toEqual({ startLine: 0, startChar: 0, endLine: 0, endChar: 4 })
    expect(links[0]?.url).toBe("https://example.com/x")
  })

  it("normalizes inlay hints (position 0-based, kind, padding) — B6i", async () => {
    const lf = new LanguageFeatures()
    const hint = new InlayHint(new Position(0, 6), ": int", InlayHintKind.Type)
    hint.paddingLeft = true
    lf.registerInlayHintsProvider("ts", { provideInlayHints: () => [hint] }, docFor("x", "ts"))
    const hints = await lf.provideInlayHints("/a.ts", "ts", 0, 0, 5, 0)
    expect(hints[0]).toMatchObject({ position: { line: 0, character: 6 }, label: ": int", kind: InlayHintKind.Type, paddingLeft: true })
  })

  it("flattens selection-range parent chains per position — B6i", async () => {
    const lf = new LanguageFeatures()
    const chain = new SelectionRange(new Range(1, 0, 1, 4), new SelectionRange(new Range(1, 0, 1, 16)))
    lf.registerSelectionRangeProvider("ts", { provideSelectionRanges: () => [chain] }, docFor("x", "ts"))
    const ranges = await lf.provideSelectionRanges("/a.ts", "ts", [{ line: 1, character: 2 }])
    expect(ranges[0]).toEqual([
      { startLine: 1, startChar: 0, endLine: 1, endChar: 4 },
      { startLine: 1, startChar: 0, endLine: 1, endChar: 16 },
    ])
  })

  it("normalizes code lenses (Command.command → id) — B6j", async () => {
    const lf = new LanguageFeatures()
    const lens = new CodeLens(new Range(0, 0, 0, 0), { command: "hello.run", title: "Run", arguments: [1, 2] })
    lf.registerCodeLensProvider("ts", { provideCodeLenses: () => [lens] }, docFor("x", "ts"))
    const lenses = await lf.provideCodeLenses("/a.ts", "ts")
    expect(lenses[0]).toEqual({ range: { startLine: 0, startChar: 0, endLine: 0, endChar: 0 }, command: { id: "hello.run", title: "Run", arguments: [1, 2] } })
  })

  it("normalizes document colors + presentations — B6k", async () => {
    const lf = new LanguageFeatures()
    lf.registerColorProvider("ts", {
      provideDocumentColors: () => [new ColorInformation(new Range(0, 0, 0, 4), new Color(1, 0, 0, 1))],
      provideColorPresentations: () => [new ColorPresentation("Crimson")],
    }, docFor("x", "ts"))
    const colors = await lf.provideDocumentColors("/a.ts", "ts")
    expect(colors[0]).toEqual({ range: { startLine: 0, startChar: 0, endLine: 0, endChar: 4 }, color: { red: 1, green: 0, blue: 0, alpha: 1 } })
    const pres = await lf.provideColorPresentations("/a.ts", "ts", { red: 1, green: 0, blue: 0, alpha: 1 }, { startLine: 0, startChar: 0, endLine: 0, endChar: 4 })
    expect(pres[0]?.label).toBe("Crimson")
  })

})

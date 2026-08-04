// Language-feature provider registry (B6). A single host-wide instance holds
// every extension's `languages.register*Provider` so the renderer's Monaco
// providers can query across all extensions over the existing renderer→host
// RPC. Each entry carries the registering extension's `docFor` so the provider
// callback receives that extension's TextDocument identity. DTO shapes +
// normalizers live in language-feature-dtos.ts.
import type {
  CodeActionDto, CodeLensDto, ColorDto, ColorInformationDto, ColorPresentationDto, CompletionItemDto, CompletionResultDto, DocumentHighlightDto, DocumentLinkDto, DocumentSymbolDto, FoldingRangeDto, HoverDto, InlayHintDto, LocationDto, RangeDto, SemanticTokensDto, SignatureHelpDto, TextEditDto, WorkspaceEditDto,
} from "../../api/language-feature-dtos.js"
import type { Disposable as DisposableType } from "../../api/types.js"
import { Position, Range } from "./classes-core.js"
import { Color } from "./classes-lang.js"
import { flattenSelectionRange, toCodeActionDtos, toCodeLensDtos, toColorInformationDtos, toColorPresentationDtos, toInlayHintDtos, toWorkspaceEditDto } from "./language-feature-dtos-nav.js"
import {
  toCompletionDtos, toDocumentHighlightDtos, toDocumentLinkDtos, toDocumentSymbolDtos, toFoldingRangeDtos,
  toHoverDtos, toLocationDtos, toSignatureHelpDto, toTextEditDtos, isCompletionListIncomplete,
} from "./language-feature-dtos.js"
import type {
  CodeActionProvider, CodeLensProvider, ColorProvider, CompletionProvider, DeclarationProvider, DefinitionProvider,
  DocFor, DocumentHighlightProvider, DocumentLinkProvider, DocumentSelector, DocumentSymbolProvider, Entry,
  FoldingRangeProvider, FormattingProvider, HoverProvider, ImplementationProvider, InlayHintsProvider, ReferenceProvider,
  RenameProvider, SelectionRangeProvider, SemanticTokensLegendLike, SemanticTokensProvider, SignatureHelpProvider,
  TypeDefinitionProvider, WorkspaceSymbolProvider,
} from "./language-feature-types.js"
import { matchesSelector } from "./selector-score.js"
import { toSemanticTokensDto } from "./semantic-tokens-remap.js"
import { Disposable } from "./shared.js"
import type { TextDocument } from "./text-document.js"
import { WorkspaceSymbolRegistry } from "./workspace-symbols.js"

// DTO contract re-exported from the shared single-source module (api/language-feature-dtos.ts) so exthost/index.ts keeps importing DTO types from here.
export type {
  CodeActionDto, CodeLensDto, ColorDto, ColorInformationDto, ColorPresentationDto, CompletionItemDto, CompletionResultDto,
  DocumentHighlightDto, DocumentLinkDto, DocumentSymbolDto, FoldingRangeDto, HoverDto, InlayHintDto, LocationDto,
  SemanticTokensDto, SignatureHelpDto, TextEditDto, WorkspaceEditDto, WorkspaceSymbolDto,
} from "../../api/language-feature-dtos.js"

const NO_TOKEN = { isCancellationRequested: false, onCancellationRequested: () => ({ dispose() { /* */ } }) }
// vscode.CompletionTriggerKind.Invoke — we only synthesize manual invocations.
const COMPLETION_TRIGGER_INVOKE = 0

function register<P>(list: Entry<P>[], entry: Entry<P>): DisposableType {
  list.push(entry)
  return new Disposable(() => { const i = list.indexOf(entry); if (i >= 0) list.splice(i, 1) })
}

export class LanguageFeatures {
  private readonly completion: Entry<CompletionProvider>[] = []
  private readonly hover: Entry<HoverProvider>[] = []
  private readonly definition: Entry<DefinitionProvider>[] = []
  private readonly formatting: Entry<FormattingProvider>[] = []
  private readonly references: Entry<ReferenceProvider>[] = []
  private readonly documentHighlight: Entry<DocumentHighlightProvider>[] = []
  private readonly folding: Entry<FoldingRangeProvider>[] = []
  private readonly declaration: Entry<DeclarationProvider>[] = []
  private readonly typeDefinition: Entry<TypeDefinitionProvider>[] = []
  private readonly implementation: Entry<ImplementationProvider>[] = []
  private readonly signatureHelp: Entry<SignatureHelpProvider>[] = []
  private readonly documentSymbol: Entry<DocumentSymbolProvider>[] = []
  private readonly documentLink: Entry<DocumentLinkProvider>[] = []
  private readonly inlayHints: Entry<InlayHintsProvider>[] = []
  private readonly selectionRange: Entry<SelectionRangeProvider>[] = []
  private readonly codeLens: Entry<CodeLensProvider>[] = []
  private readonly color: Entry<ColorProvider>[] = []
  private readonly rename: Entry<RenameProvider>[] = []
  private readonly codeAction: Entry<CodeActionProvider>[] = []
  private readonly semanticTokens: Entry<SemanticTokensProvider>[] = []
  // Workspace symbols are NOT per-document — own registry (workspace-symbols.ts).
  private readonly workspaceSymbols = new WorkspaceSymbolRegistry()

  // Registration is identical boilerplate for every feature (push onto the typed
  // list, return a Disposable that splices it out); kept one-per-line for the
  // exact `vscode.languages.register*Provider` surface ns-data forwards to.
  registerCompletionItemProvider(s: DocumentSelector, p: CompletionProvider, d: DocFor): DisposableType { return register(this.completion, { selector: s, provider: p, docFor: d }) }
  registerHoverProvider(s: DocumentSelector, p: HoverProvider, d: DocFor): DisposableType { return register(this.hover, { selector: s, provider: p, docFor: d }) }
  registerDefinitionProvider(s: DocumentSelector, p: DefinitionProvider, d: DocFor): DisposableType { return register(this.definition, { selector: s, provider: p, docFor: d }) }
  registerDocumentFormattingEditProvider(s: DocumentSelector, p: FormattingProvider, d: DocFor): DisposableType { return register(this.formatting, { selector: s, provider: p, docFor: d }) }
  registerReferenceProvider(s: DocumentSelector, p: ReferenceProvider, d: DocFor): DisposableType { return register(this.references, { selector: s, provider: p, docFor: d }) }
  registerDocumentHighlightProvider(s: DocumentSelector, p: DocumentHighlightProvider, d: DocFor): DisposableType { return register(this.documentHighlight, { selector: s, provider: p, docFor: d }) }
  registerFoldingRangeProvider(s: DocumentSelector, p: FoldingRangeProvider, d: DocFor): DisposableType { return register(this.folding, { selector: s, provider: p, docFor: d }) }
  registerDeclarationProvider(s: DocumentSelector, p: DeclarationProvider, d: DocFor): DisposableType { return register(this.declaration, { selector: s, provider: p, docFor: d }) }
  registerTypeDefinitionProvider(s: DocumentSelector, p: TypeDefinitionProvider, d: DocFor): DisposableType { return register(this.typeDefinition, { selector: s, provider: p, docFor: d }) }
  registerImplementationProvider(s: DocumentSelector, p: ImplementationProvider, d: DocFor): DisposableType { return register(this.implementation, { selector: s, provider: p, docFor: d }) }
  registerSignatureHelpProvider(s: DocumentSelector, p: SignatureHelpProvider, d: DocFor): DisposableType { return register(this.signatureHelp, { selector: s, provider: p, docFor: d }) }
  registerDocumentSymbolProvider(s: DocumentSelector, p: DocumentSymbolProvider, d: DocFor): DisposableType { return register(this.documentSymbol, { selector: s, provider: p, docFor: d }) }
  registerDocumentLinkProvider(s: DocumentSelector, p: DocumentLinkProvider, d: DocFor): DisposableType { return register(this.documentLink, { selector: s, provider: p, docFor: d }) }
  registerInlayHintsProvider(s: DocumentSelector, p: InlayHintsProvider, d: DocFor): DisposableType { return register(this.inlayHints, { selector: s, provider: p, docFor: d }) }
  registerSelectionRangeProvider(s: DocumentSelector, p: SelectionRangeProvider, d: DocFor): DisposableType { return register(this.selectionRange, { selector: s, provider: p, docFor: d }) }
  registerCodeLensProvider(s: DocumentSelector, p: CodeLensProvider, d: DocFor): DisposableType { return register(this.codeLens, { selector: s, provider: p, docFor: d }) }
  registerColorProvider(s: DocumentSelector, p: ColorProvider, d: DocFor): DisposableType { return register(this.color, { selector: s, provider: p, docFor: d }) }
  registerRenameProvider(s: DocumentSelector, p: RenameProvider, d: DocFor): DisposableType { return register(this.rename, { selector: s, provider: p, docFor: d }) }
  registerCodeActionsProvider(s: DocumentSelector, p: CodeActionProvider, d: DocFor): DisposableType { return register(this.codeAction, { selector: s, provider: p, docFor: d }) }
  registerDocumentSemanticTokensProvider(s: DocumentSelector, p: SemanticTokensProvider, legend: SemanticTokensLegendLike, d: DocFor): DisposableType { return register(this.semanticTokens, { selector: s, provider: p, docFor: d, legend }) }
  registerWorkspaceSymbolProvider(p: WorkspaceSymbolProvider): DisposableType { return this.workspaceSymbols.register(p) }

  /** Run matching providers in `list` and collect each call's result via `map`. */
  private async query<P, R>(list: Entry<P>[], languageId: string, uri: string, call: (p: P, doc: TextDocument) => unknown, map: (r: unknown) => R[]): Promise<R[]> {
    const out: R[] = []
    for (const entry of list) {
      if (!matchesSelector(entry.selector, languageId)) continue
      try {
        out.push(...map(await Promise.resolve(call(entry.provider, entry.docFor(uri)))))
      } catch (err) {
        console.warn("language-features: provider threw:", err)
      }
    }
    return out
  }

  async provideCompletionItems(uri: string, languageId: string, line: number, character: number, triggerKind?: number, triggerCharacter?: string): Promise<CompletionResultDto> {
    const pos = new Position(line, character)
    const context = { triggerKind: triggerKind ?? COMPLETION_TRIGGER_INVOKE, triggerCharacter }
    const items: CompletionItemDto[] = []
    let isIncomplete = false
    for (const entry of this.completion) {
      if (!matchesSelector(entry.selector, languageId)) continue
      try {
        const result = await Promise.resolve(entry.provider.provideCompletionItems(entry.docFor(uri), pos, NO_TOKEN, context))
        items.push(...toCompletionDtos(result))
        if (isCompletionListIncomplete(result)) isIncomplete = true
      } catch (err) {
        console.warn("language-features: completion provider threw:", err)
      }
    }
    return { items, isIncomplete }
  }

  provideHover(uri: string, languageId: string, line: number, character: number): Promise<HoverDto[]> {
    return this.query(this.hover, languageId, uri, (p, doc) => p.provideHover(doc, new Position(line, character), NO_TOKEN), toHoverDtos)
  }

  provideDefinition(uri: string, languageId: string, line: number, character: number): Promise<LocationDto[]> {
    return this.query(this.definition, languageId, uri, (p, doc) => p.provideDefinition(doc, new Position(line, character), NO_TOKEN), toLocationDtos)
  }

  provideReferences(uri: string, languageId: string, line: number, character: number, includeDeclaration: boolean): Promise<LocationDto[]> {
    return this.query(this.references, languageId, uri, (p, doc) => p.provideReferences(doc, new Position(line, character), { includeDeclaration }, NO_TOKEN), toLocationDtos)
  }

  provideDeclaration(uri: string, languageId: string, line: number, character: number): Promise<LocationDto[]> {
    return this.query(this.declaration, languageId, uri, (p, doc) => p.provideDeclaration(doc, new Position(line, character), NO_TOKEN), toLocationDtos)
  }

  provideTypeDefinition(uri: string, languageId: string, line: number, character: number): Promise<LocationDto[]> {
    const pos = new Position(line, character)
    return this.query(this.typeDefinition, languageId, uri, (p, doc) => p.provideTypeDefinition(doc, pos, NO_TOKEN), toLocationDtos)
  }

  provideImplementation(uri: string, languageId: string, line: number, character: number): Promise<LocationDto[]> {
    const pos = new Position(line, character)
    return this.query(this.implementation, languageId, uri, (p, doc) => p.provideImplementation(doc, pos, NO_TOKEN), toLocationDtos)
  }

  /** Signature help — VS Code uses the FIRST provider that yields signatures
   *  (not a merge), so stop at the first non-null result. */
  async provideSignatureHelp(uri: string, languageId: string, line: number, character: number, triggerCharacter?: string): Promise<SignatureHelpDto | null> {
    const pos = new Position(line, character)
    // vscode SignatureHelpTriggerKind: Invoke 1 / TriggerCharacter 2.
    const context = { triggerKind: triggerCharacter ? 2 : 1, triggerCharacter, isRetrigger: false, activeSignatureHelp: undefined }
    for (const entry of this.signatureHelp) {
      if (!matchesSelector(entry.selector, languageId)) continue
      try {
        const dto = toSignatureHelpDto(await Promise.resolve(entry.provider.provideSignatureHelp(entry.docFor(uri), pos, NO_TOKEN, context)))
        if (dto) return dto
      } catch (err) {
        console.warn("language-features: signature-help provider threw:", err)
      }
    }
    return null
  }

  provideDocumentHighlights(uri: string, languageId: string, line: number, character: number): Promise<DocumentHighlightDto[]> {
    const pos = new Position(line, character)
    return this.query(this.documentHighlight, languageId, uri, (p, doc) => p.provideDocumentHighlights(doc, pos, NO_TOKEN), toDocumentHighlightDtos)
  }

  /** Folding — document-scoped (no position), every provider's ranges merged.
   *  Reuses query() (folding has no cursor position; the empty FoldingContext
   *  is the only extra arg). */
  provideFoldingRanges(uri: string, languageId: string): Promise<FoldingRangeDto[]> {
    return this.query(this.folding, languageId, uri, (p, doc) => p.provideFoldingRanges(doc, {}, NO_TOKEN), toFoldingRangeDtos)
  }

  /** Document symbols + links — document-scoped, every provider's results merged. */
  provideDocumentSymbols(uri: string, languageId: string): Promise<DocumentSymbolDto[]> {
    return this.query(this.documentSymbol, languageId, uri, (p, doc) => p.provideDocumentSymbols(doc, NO_TOKEN), toDocumentSymbolDtos)
  }

  provideDocumentLinks(uri: string, languageId: string): Promise<DocumentLinkDto[]> {
    return this.query(this.documentLink, languageId, uri, (p, doc) => p.provideDocumentLinks(doc, NO_TOKEN), toDocumentLinkDtos)
  }

  /** Code lenses — document-scoped, every provider's lenses merged. */
  provideCodeLenses(uri: string, languageId: string): Promise<CodeLensDto[]> {
    return this.query(this.codeLens, languageId, uri, (p, doc) => p.provideCodeLenses(doc, NO_TOKEN), toCodeLensDtos)
  }

  /** Document colors — every provider's colors merged (renderer draws swatches). */
  provideDocumentColors(uri: string, languageId: string): Promise<ColorInformationDto[]> {
    return this.query(this.color, languageId, uri, (p, doc) => p.provideDocumentColors(doc, NO_TOKEN), toColorInformationDtos)
  }

  /** Code actions (lightbulb) — range-scoped, every provider's actions merged. */
  provideCodeActions(uri: string, languageId: string, range: RangeDto): Promise<CodeActionDto[]> {
    const r = new Range(range.startLine, range.startChar, range.endLine, range.endChar)
    const context = { diagnostics: [], only: undefined, triggerKind: 1 }
    return this.query(this.codeAction, languageId, uri, (p, doc) => p.provideCodeActions(doc, r, context, NO_TOKEN), toCodeActionDtos)
  }

  /** Semantic tokens — first matching provider; its tokens are remapped from the
   *  provider's own legend to the shared standard legend Monaco registered with. */
  async provideDocumentSemanticTokens(uri: string, languageId: string): Promise<SemanticTokensDto | null> {
    for (const entry of this.semanticTokens) {
      if (!matchesSelector(entry.selector, languageId)) continue
      try {
        const result = await Promise.resolve(entry.provider.provideDocumentSemanticTokens(entry.docFor(uri), NO_TOKEN))
        const dto = toSemanticTokensDto(result, entry.legend ?? { tokenTypes: [], tokenModifiers: [] })
        if (dto && dto.data.length > 0) return dto
      } catch (err) {
        console.warn("language-features: semantic-tokens provider threw:", err)
      }
    }
    return null
  }

  /** Workspace symbols (Go to Symbol in Workspace, Ctrl+T) — delegated to the
   *  standalone registry (not document-scoped). */
  provideWorkspaceSymbols(query: string) { return this.workspaceSymbols.provide(query) }

  /** Rename — first provider that yields a WorkspaceEdit (renderer applies it). */
  async provideRename(uri: string, languageId: string, line: number, character: number, newName: string): Promise<WorkspaceEditDto | null> {
    const pos = new Position(line, character)
    for (const entry of this.rename) {
      if (!matchesSelector(entry.selector, languageId)) continue
      try {
        const dto = toWorkspaceEditDto(await Promise.resolve(entry.provider.provideRenameEdits(entry.docFor(uri), pos, newName, NO_TOKEN)))
        if (dto && dto.edits.length > 0) return dto
      } catch (err) {
        console.warn("language-features: rename provider threw:", err)
      }
    }
    return null
  }

  /** Color presentations (the picker's format choices) — first matching provider. */
  async provideColorPresentations(uri: string, languageId: string, color: ColorDto, range: RangeDto): Promise<ColorPresentationDto[]> {
    const c = new Color(color.red, color.green, color.blue, color.alpha)
    for (const entry of this.color) {
      if (!matchesSelector(entry.selector, languageId)) continue
      const doc = entry.docFor(uri)
      const context = { document: doc, range: new Range(range.startLine, range.startChar, range.endLine, range.endChar) }
      try {
        const presentations = toColorPresentationDtos(await Promise.resolve(entry.provider.provideColorPresentations(c, context, NO_TOKEN)))
        if (presentations.length > 0) return presentations
      } catch (err) {
        console.warn("language-features: color-presentation provider threw:", err)
      }
    }
    return []
  }

  /** Inlay hints — range-scoped; every provider's hints merged. */
  provideInlayHints(uri: string, languageId: string, sl: number, sc: number, el: number, ec: number): Promise<InlayHintDto[]> {
    const range = new Range(sl, sc, el, ec)
    return this.query(this.inlayHints, languageId, uri, (p, doc) => p.provideInlayHints(doc, range, NO_TOKEN), toInlayHintDtos)
  }

  /** Selection ranges — one expand-chain per cursor position; VS Code uses the
   *  FIRST provider that returns, aligned to the input positions. */
  async provideSelectionRanges(uri: string, languageId: string, positions: { line: number; character: number }[]): Promise<RangeDto[][]> {
    const posObjs = positions.map((p) => new Position(p.line, p.character))
    for (const entry of this.selectionRange) {
      if (!matchesSelector(entry.selector, languageId)) continue
      try {
        const result = await Promise.resolve(entry.provider.provideSelectionRanges(entry.docFor(uri), posObjs, NO_TOKEN))
        if (Array.isArray(result) && result.length > 0) return result.map(flattenSelectionRange)
      } catch (err) {
        console.warn("language-features: selection-range provider threw:", err)
      }
    }
    return []
  }

  /** Document formatting — VS Code runs ONE formatter (the first that yields
   *  edits), not every provider, so we stop at the first non-empty result. */
  async provideDocumentFormattingEdits(uri: string, languageId: string, options: unknown): Promise<TextEditDto[]> {
    for (const entry of this.formatting) {
      if (!matchesSelector(entry.selector, languageId)) continue
      try {
        const edits = toTextEditDtos(await Promise.resolve(entry.provider.provideDocumentFormattingEdits(entry.docFor(uri), options, NO_TOKEN)))
        if (edits.length > 0) return edits
      } catch (err) {
        console.warn("language-features: formatting provider threw:", err)
      }
    }
    return []
  }
}

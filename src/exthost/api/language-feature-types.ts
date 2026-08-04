// Provider-shape interfaces + selector types for the LanguageFeatures registry
// (B6). Split out of language-features.ts to keep that file under the 250-LOC
// ceiling. Every provider is typed loosely at the extension boundary (the real
// callbacks are untyped); the registry guards each call at runtime.
import type { Position } from "./classes-core.js"
import type { TextDocument } from "./text-document.js"

export type DocFor = (uri: string) => TextDocument

/** vscode DocumentSelector — a language id, a {language,scheme,pattern} filter,
 *  or an array of either. `matchesSelector` only discriminates on `language`
 *  today; the filter shape stays loose so an unrecognized member (e.g. a
 *  RelativePattern in `pattern`) widens to a match rather than throwing. */
export interface DocumentFilter { language?: string; scheme?: string; pattern?: unknown }
export type DocumentSelector = string | DocumentFilter | (string | DocumentFilter)[]

/** One registered provider: its selector, the untyped extension callback object,
 *  and the registering extension's docFor (so the callback gets that extension's
 *  TextDocument identity). Semantic-tokens entries also carry a legend. */
export interface Entry<P> { selector: DocumentSelector; provider: P; docFor: DocFor; legend?: SemanticTokensLegendLike }

/** vscode SemanticTokensLegend — the token-type / modifier name tables a
 *  provider's encoded indices reference. */
export interface SemanticTokensLegendLike { tokenTypes: string[]; tokenModifiers: string[] }

export interface CompletionProvider { provideCompletionItems: (doc: TextDocument, pos: Position, token: unknown, context: unknown) => unknown }
export interface HoverProvider { provideHover: (doc: TextDocument, pos: Position, token: unknown) => unknown }
export interface DefinitionProvider { provideDefinition: (doc: TextDocument, pos: Position, token: unknown) => unknown }
export interface DeclarationProvider { provideDeclaration: (doc: TextDocument, pos: Position, token: unknown) => unknown }
export interface TypeDefinitionProvider { provideTypeDefinition: (doc: TextDocument, pos: Position, token: unknown) => unknown }
export interface ImplementationProvider { provideImplementation: (doc: TextDocument, pos: Position, token: unknown) => unknown }
export interface SignatureHelpProvider { provideSignatureHelp: (doc: TextDocument, pos: Position, token: unknown, context: unknown) => unknown }
export interface DocumentSymbolProvider { provideDocumentSymbols: (doc: TextDocument, token: unknown) => unknown }
export interface DocumentLinkProvider { provideDocumentLinks: (doc: TextDocument, token: unknown) => unknown }
export interface InlayHintsProvider { provideInlayHints: (doc: TextDocument, range: unknown, token: unknown) => unknown }
export interface SelectionRangeProvider { provideSelectionRanges: (doc: TextDocument, positions: Position[], token: unknown) => unknown }
export interface CodeLensProvider { provideCodeLenses: (doc: TextDocument, token: unknown) => unknown }
export interface ColorProvider {
  provideDocumentColors: (doc: TextDocument, token: unknown) => unknown
  provideColorPresentations: (color: unknown, context: unknown, token: unknown) => unknown
}
export interface RenameProvider { provideRenameEdits: (doc: TextDocument, pos: Position, newName: string, token: unknown) => unknown }
export interface CodeActionProvider { provideCodeActions: (doc: TextDocument, range: unknown, context: unknown, token: unknown) => unknown }
export interface FormattingProvider { provideDocumentFormattingEdits: (doc: TextDocument, options: unknown, token: unknown) => unknown }
export interface ReferenceProvider { provideReferences: (doc: TextDocument, pos: Position, context: unknown, token: unknown) => unknown }
export interface DocumentHighlightProvider { provideDocumentHighlights: (doc: TextDocument, pos: Position, token: unknown) => unknown }
export interface FoldingRangeProvider { provideFoldingRanges: (doc: TextDocument, context: unknown, token: unknown) => unknown }
export interface SemanticTokensProvider { provideDocumentSemanticTokens: (doc: TextDocument, token: unknown) => unknown }
/** Workspace-wide (not per-document) — queried by Go to Symbol in Workspace. */
export interface WorkspaceSymbolProvider {
  provideWorkspaceSymbols: (query: string, token: unknown) => unknown
  /** Optional — fills in a symbol's location.range on demand (providers may
   *  return range-less symbols from provideWorkspaceSymbols for speed). */
  resolveWorkspaceSymbol?: (symbol: unknown, token: unknown) => unknown
}

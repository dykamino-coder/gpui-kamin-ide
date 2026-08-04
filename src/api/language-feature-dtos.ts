// Single source of truth for the B6 language-feature WS contract. These DTOs
// cross the loopback WS verbatim between the kamin-host ext-host (which maps
// untyped extension results into them) and the renderer's Monaco providers
// (which map them into Monaco shapes). Both sides import these types from here
// — previously they were defined twice (host api/* + renderer host-rpc-lang-dtos)
// and could silently drift. Pure types only: no runtime, so this module is safe
// to import from either bundle.

/** 0-based range crossing the WS (renderer adds 1 for Monaco's 1-based lines). */
export interface RangeDto { startLine: number; startChar: number; endLine: number; endChar: number }

/** One text replacement (0-based range + new text). */
export interface TextEditDto { range: RangeDto; newText: string }

export interface CompletionItemDto {
  label: string
  // CompletionItemLabel.detail/.description (the suffix + right-aligned text in
  // the completion popup) — distinct from the item's `detail` (the doc-side panel).
  labelDetail?: string
  labelDescription?: string
  kind?: number
  detail?: string
  documentation?: string
  insertText?: string
  // true when insertText was a SnippetString (tab-stops) — the renderer must
  // apply it as a snippet, not literal text.
  insertTextIsSnippet?: boolean
  sortText?: string
  filterText?: string
}
/** Items plus VS Code's `isIncomplete` (true → editor re-queries each keystroke). */
export interface CompletionResultDto { items: CompletionItemDto[]; isIncomplete: boolean }

export interface HoverDto { contents: string[]; range?: RangeDto }

/** Location or LocationLink target (definition/references/declaration/…). */
export interface LocationDto { uri: string; range: RangeDto; targetSelectionRange?: RangeDto }

/** A highlighted occurrence. kind: 0 Text, 1 Read, 2 Write (vscode == Monaco). */
export interface DocumentHighlightDto { range: RangeDto; kind?: number }

/** A foldable region — 0-based start/end lines. `kind` is vscode's numeric
 *  FoldingRangeKind (Comment 1 / Imports 2 / Region 3); the renderer maps it to
 *  Monaco's string-valued kind so "Fold All Comments/Regions" work. */
export interface FoldingRangeDto { start: number; end: number; kind?: number }

/** Signature help (parameter hints). A parameter `label` is either the literal
 *  text or a [startOffset, endOffset] span into the signature label (vscode
 *  ParameterInformation.label == Monaco's); both forms cross the WS verbatim. */
export interface ParameterInformationDto { label: string | [number, number]; documentation?: string }
export interface SignatureInformationDto { label: string; documentation?: string; parameters: ParameterInformationDto[]; activeParameter?: number }
export interface SignatureHelpDto { signatures: SignatureInformationDto[]; activeSignature: number; activeParameter: number }

/** A document symbol (outline / breadcrumbs / Go to Symbol). vscode SymbolKind
 *  == Monaco's, no remap. Carries the nested DocumentSymbol shape. */
export interface DocumentSymbolDto { name: string; detail: string; kind: number; range: RangeDto; selectionRange: RangeDto; children?: DocumentSymbolDto[] }

/** A clickable link region in the document. */
export interface DocumentLinkDto { range: RangeDto; url?: string; tooltip?: string }

/** An inline hint (type/parameter annotation). label parts flattened to a
 *  string; position is 0-based (renderer adds 1 for Monaco). */
export interface InlayHintDto { position: { line: number; character: number }; label: string; kind?: number; paddingLeft?: boolean; paddingRight?: boolean; tooltip?: string }

/** A code lens (clickable annotation above a line). vscode Command.command (the
 *  id string) maps to Monaco Command.id. */
export interface CodeLensDto { range: RangeDto; command?: { id: string; title: string; arguments?: unknown[] } }

/** Document colors + the picker's format presentations (0-1 float components). */
export interface ColorDto { red: number; green: number; blue: number; alpha: number }
export interface ColorInformationDto { range: RangeDto; color: ColorDto }
export interface ColorPresentationDto { label: string; textEdit?: TextEditDto; additionalTextEdits?: TextEditDto[] }

/** A flattened workspace edit (per-resource text edits) crossing the WS. File
 *  operations are out of scope — text edits cover rename + the common
 *  code-action case. */
export interface WorkspaceEditDto { edits: { resource: string; textEdit: { range: RangeDto; newText: string } }[] }

/** Workspace symbol (Go to Symbol in Workspace, Ctrl+T). vscode SymbolKind == Monaco's. */
export interface WorkspaceSymbolDto { name: string; kind: number; containerName?: string; uri: string; range: RangeDto }

/** A code action (lightbulb) — optional workspace edit and/or command; kind
 *  drives grouping (quickfix/refactor/source). */
export interface CodeActionDto { title: string; kind?: string; isPreferred?: boolean; edit?: WorkspaceEditDto; command?: { id: string; title: string; arguments?: unknown[] } }

/** Encoded semantic tokens (delta 5-int groups, already remapped to the standard
 *  legend Monaco is registered with). resultId enables future delta requests. */
export interface SemanticTokensDto { data: number[]; resultId?: string }

/** A diagnostic broadcast as `kamin:diag:set` { owner, uri, diagnostics }.
 *  severity is vscode.DiagnosticSeverity (Error=0 … Hint=3). */
export interface DiagnosticDto { range: RangeDto; message: string; severity: number; source?: string; code?: string | number }
export interface DiagSetEvent { owner: string; uri: string; diagnostics: DiagnosticDto[] }

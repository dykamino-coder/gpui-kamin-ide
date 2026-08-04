// Normalizers turning untyped extension results into the B6 DTO WS contract
// (the DTO types themselves live in the shared single-source module
// api/language-feature-dtos.ts). Every mapper defends against the untyped
// extension boundary (providers return strings, objects, arrays, or null
// indiscriminately). Split out of language-features.ts to keep that file under
// the 250-LOC ceiling.
import { URI } from "vscode-uri"
import type {
  CompletionItemDto, DocumentHighlightDto, DocumentLinkDto, DocumentSymbolDto, FoldingRangeDto,
  HoverDto, LocationDto, ParameterInformationDto, SignatureHelpDto, SignatureInformationDto, TextEditDto,
} from "../../api/language-feature-dtos.js"
import { rangeDto } from "./range-dto.js"

export function asString(v: unknown): string | undefined {
  if (typeof v === "string") return v
  if (v && typeof v === "object" && "value" in v) {
    // MarkedString code-block form `{ language, value }` → fenced block so the
    // renderer's markdown keeps syntax highlighting (d.ts MarkedString).
    if ("language" in v && typeof v.language === "string" && v.language) {
      return `\`\`\`${v.language}\n${String(v.value)}\n\`\`\``
    }
    return String(v.value)
  }
  return undefined
}

/** A CompletionList with `isIncomplete: true` (not a bare CompletionItem[]). */
export function isCompletionListIncomplete(result: unknown): boolean {
  return !!result && typeof result === "object" && !Array.isArray(result)
    && (result as { isIncomplete?: unknown }).isIncomplete === true
}

/** Normalize a provider result (CompletionItem[] | CompletionList | label) to DTOs. */
export function toCompletionDtos(result: unknown): CompletionItemDto[] {
  if (!result) return []
  const items = Array.isArray(result) ? result : (result as { items?: unknown[] }).items ?? []
  const out: CompletionItemDto[] = []
  for (const raw of items) {
    if (raw == null) continue
    const it = typeof raw === "string" ? { label: raw } : (raw as Record<string, unknown>)
    // label is `string | CompletionItemLabel{ label, detail?, description? }`.
    const labelObj = typeof it.label === "object" && it.label !== null ? it.label as { label?: unknown; detail?: unknown; description?: unknown } : undefined
    const label = typeof it.label === "string" ? it.label : asString(labelObj?.label) ?? ""
    if (!label) continue
    const dto: CompletionItemDto = { label }
    if (labelObj) {
      if (typeof labelObj.detail === "string") dto.labelDetail = labelObj.detail
      if (typeof labelObj.description === "string") dto.labelDescription = labelObj.description
    }
    if (typeof it.kind === "number") dto.kind = it.kind
    if (typeof it.detail === "string") dto.detail = it.detail
    const doc = asString(it.documentation); if (doc !== undefined) dto.documentation = doc
    const ins = asString(it.insertText); if (ins !== undefined) dto.insertText = ins
    // A non-string insertText with a `.value` is a SnippetString (tab-stops).
    if (it.insertText !== null && typeof it.insertText === "object") dto.insertTextIsSnippet = true
    if (typeof it.sortText === "string") dto.sortText = it.sortText
    if (typeof it.filterText === "string") dto.filterText = it.filterText
    out.push(dto)
  }
  return out
}

export function toTextEditDtos(result: unknown): TextEditDto[] {
  if (!Array.isArray(result)) return []
  const out: TextEditDto[] = []
  for (const raw of result) {
    if (!raw || typeof raw !== "object") continue
    const e = raw as { range?: unknown; newText?: unknown }
    const range = rangeDto(e.range)
    if (range && typeof e.newText === "string") out.push({ range, newText: e.newText })
  }
  return out
}

export function toHoverDtos(result: unknown): HoverDto[] {
  if (!result || typeof result !== "object") return []
  const h = result as { contents?: unknown; range?: unknown }
  const raw = Array.isArray(h.contents) ? h.contents : [h.contents]
  // asString handles string, MarkdownString `{value}` and MarkedString
  // `{language,value}` (code-block) shapes.
  const contents = raw.map((c) => asString(c) ?? "").filter(Boolean)
  if (contents.length === 0) return []
  const dto: HoverDto = { contents }
  const range = rangeDto(h.range); if (range) dto.range = range
  return [dto]
}

export function toLocationDtos(result: unknown): LocationDto[] {
  if (!result) return []
  const list = Array.isArray(result) ? result : [result]
  const out: LocationDto[] = []
  for (const raw of list) {
    if (!raw || typeof raw !== "object") continue
    const loc = raw as { uri?: unknown; range?: unknown; targetUri?: unknown; targetRange?: unknown; targetSelectionRange?: unknown }
    const uriObj = loc.uri ?? loc.targetUri // Location vs LocationLink
    const range = rangeDto(loc.range ?? loc.targetRange)
    const uri = uriObj instanceof URI ? uriObj.fsPath : asString(uriObj) ?? (uriObj && typeof uriObj === "object" && "fsPath" in uriObj ? String(uriObj.fsPath) : undefined)
    if (!uri || !range) continue
    const dto: LocationDto = { uri, range }
    // LocationLink: the identifier span the editor navigates to + underlines.
    const sel = rangeDto(loc.targetSelectionRange); if (sel) dto.targetSelectionRange = sel
    out.push(dto)
  }
  return out
}

export function toDocumentHighlightDtos(result: unknown): DocumentHighlightDto[] {
  if (!Array.isArray(result)) return []
  const out: DocumentHighlightDto[] = []
  for (const raw of result) {
    if (!raw || typeof raw !== "object") continue
    const h = raw as { range?: unknown; kind?: unknown }
    const range = rangeDto(h.range)
    if (!range) continue
    const dto: DocumentHighlightDto = { range }
    if (typeof h.kind === "number") dto.kind = h.kind
    out.push(dto)
  }
  return out
}

export function toFoldingRangeDtos(result: unknown): FoldingRangeDto[] {
  if (!Array.isArray(result)) return []
  const out: FoldingRangeDto[] = []
  for (const raw of result) {
    if (!raw || typeof raw !== "object") continue
    const f = raw as { start?: unknown; end?: unknown; kind?: unknown }
    if (typeof f.start !== "number" || typeof f.end !== "number") continue
    const dto: FoldingRangeDto = { start: f.start, end: f.end }
    if (typeof f.kind === "number") dto.kind = f.kind
    out.push(dto)
  }
  return out
}

function toParamDto(raw: unknown): ParameterInformationDto | null {
  if (!raw || typeof raw !== "object") return null
  const p = raw as { label?: unknown; documentation?: unknown }
  const label = typeof p.label === "string" ? p.label
    : Array.isArray(p.label) && p.label.length === 2 && p.label.every((n) => typeof n === "number")
      ? [p.label[0], p.label[1]] as [number, number] : undefined
  if (label === undefined) return null
  const dto: ParameterInformationDto = { label }
  const doc = asString(p.documentation); if (doc !== undefined) dto.documentation = doc
  return dto
}

function toOneSymbol(raw: unknown): DocumentSymbolDto | null {
  if (!raw || typeof raw !== "object") return null
  const s = raw as { name?: unknown; detail?: unknown; kind?: unknown; range?: unknown; selectionRange?: unknown; children?: unknown; location?: { range?: unknown } }
  if (typeof s.name !== "string") return null
  // DocumentSymbol carries range+selectionRange; SymbolInformation has location.range.
  const range = rangeDto(s.range ?? s.location?.range)
  const selectionRange = rangeDto(s.selectionRange ?? s.range ?? s.location?.range)
  if (!range || !selectionRange) return null
  const dto: DocumentSymbolDto = {
    name: s.name,
    detail: typeof s.detail === "string" ? s.detail : "",
    kind: typeof s.kind === "number" ? s.kind : 0,
    range, selectionRange,
  }
  if (Array.isArray(s.children)) {
    const children = s.children.map(toOneSymbol).filter((c): c is DocumentSymbolDto => c !== null)
    if (children.length > 0) dto.children = children
  }
  return dto
}

export function toDocumentSymbolDtos(result: unknown): DocumentSymbolDto[] {
  if (!Array.isArray(result)) return []
  return result.map(toOneSymbol).filter((s): s is DocumentSymbolDto => s !== null)
}

export function toDocumentLinkDtos(result: unknown): DocumentLinkDto[] {
  if (!Array.isArray(result)) return []
  const out: DocumentLinkDto[] = []
  for (const raw of result) {
    if (!raw || typeof raw !== "object") continue
    const l = raw as { range?: unknown; target?: unknown; url?: unknown; tooltip?: unknown }
    const range = rangeDto(l.range)
    if (!range) continue
    const dto: DocumentLinkDto = { range }
    const target = l.target ?? l.url
    const url = target instanceof URI ? target.toString() : asString(target)
    if (url !== undefined) dto.url = url
    if (typeof l.tooltip === "string") dto.tooltip = l.tooltip
    out.push(dto)
  }
  return out
}

/** Normalize a vscode.SignatureHelp (first provider's result) to a DTO, or null. */
export function toSignatureHelpDto(result: unknown): SignatureHelpDto | null {
  if (!result || typeof result !== "object") return null
  const r = result as { signatures?: unknown; activeSignature?: unknown; activeParameter?: unknown }
  if (!Array.isArray(r.signatures)) return null
  const signatures: SignatureInformationDto[] = []
  for (const raw of r.signatures) {
    if (!raw || typeof raw !== "object") continue
    const s = raw as { label?: unknown; documentation?: unknown; parameters?: unknown; activeParameter?: unknown }
    if (typeof s.label !== "string") continue
    const parameters = (Array.isArray(s.parameters) ? s.parameters : []).map(toParamDto).filter((p): p is ParameterInformationDto => p !== null)
    const sig: SignatureInformationDto = { label: s.label, parameters }
    const doc = asString(s.documentation); if (doc !== undefined) sig.documentation = doc
    if (typeof s.activeParameter === "number") sig.activeParameter = s.activeParameter
    signatures.push(sig)
  }
  if (signatures.length === 0) return null
  return {
    signatures,
    activeSignature: typeof r.activeSignature === "number" ? r.activeSignature : 0,
    activeParameter: typeof r.activeParameter === "number" ? r.activeParameter : 0,
  }
}

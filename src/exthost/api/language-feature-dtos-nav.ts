// Second batch of B6 normalizers — inlay hints, code lenses, colors, workspace
// edits, code actions, workspace symbols, selection ranges. Split from
// language-feature-dtos.ts purely to keep that file under the 250-LOC ceiling;
// the DTO types live in the shared single-source module api/language-feature-dtos.ts.
import { URI } from "vscode-uri"
import type {
  CodeActionDto, CodeLensDto, ColorInformationDto, ColorPresentationDto,
  InlayHintDto, RangeDto, TextEditDto, WorkspaceEditDto, WorkspaceSymbolDto,
} from "../../api/language-feature-dtos.js"
import { asString } from "./language-feature-dtos.js"
import { rangeDto } from "./range-dto.js"

const MAX_SELECTION_DEPTH = 100

export function toCodeLensDtos(result: unknown): CodeLensDto[] {
  if (!Array.isArray(result)) return []
  const out: CodeLensDto[] = []
  for (const raw of result) {
    if (!raw || typeof raw !== "object") continue
    const l = raw as { range?: unknown; command?: unknown }
    const range = rangeDto(l.range)
    if (!range) continue
    const dto: CodeLensDto = { range }
    const cmd = l.command as { command?: unknown; title?: unknown; arguments?: unknown } | undefined
    if (cmd && typeof cmd.command === "string" && typeof cmd.title === "string") {
      dto.command = { id: cmd.command, title: cmd.title }
      if (Array.isArray(cmd.arguments)) dto.command.arguments = cmd.arguments
    }
    out.push(dto)
  }
  return out
}

export function toInlayHintDtos(result: unknown): InlayHintDto[] {
  if (!Array.isArray(result)) return []
  const out: InlayHintDto[] = []
  for (const raw of result) {
    if (!raw || typeof raw !== "object") continue
    const h = raw as { position?: unknown; label?: unknown; kind?: unknown; paddingLeft?: unknown; paddingRight?: unknown; tooltip?: unknown }
    const pos = h.position as { line?: unknown; character?: unknown } | undefined
    if (!pos || typeof pos.line !== "number" || typeof pos.character !== "number") continue
    // label is string | InlayHintLabelPart[] — flatten parts to text (Monaco accepts a plain string).
    const label = typeof h.label === "string" ? h.label
      : Array.isArray(h.label) ? h.label.map((p) => (p && typeof p === "object" && "value" in p ? String((p as { value: unknown }).value) : String(p))).join("")
        : ""
    if (!label) continue
    const dto: InlayHintDto = { position: { line: pos.line, character: pos.character }, label }
    if (typeof h.kind === "number") dto.kind = h.kind
    if (h.paddingLeft === true) dto.paddingLeft = true
    if (h.paddingRight === true) dto.paddingRight = true
    const tip = asString(h.tooltip); if (tip !== undefined) dto.tooltip = tip
    out.push(dto)
  }
  return out
}

function uriToFsPath(uri: unknown): string | undefined {
  if (uri instanceof URI) return uri.fsPath
  if (uri && typeof uri === "object" && "fsPath" in uri) return String(uri.fsPath)
  return asString(uri)
}

/** Read a vscode.WorkspaceEdit (its `.entries()` → [uri, TextEdit[]][]) into a DTO. */
export function toWorkspaceEditDto(result: unknown): WorkspaceEditDto | null {
  if (!result || typeof result !== "object") return null
  const we = result as { entries?: () => [unknown, unknown][] }
  if (typeof we.entries !== "function") return null
  const edits: WorkspaceEditDto["edits"] = []
  for (const [uri, teList] of we.entries()) {
    const resource = uriToFsPath(uri)
    if (!resource || !Array.isArray(teList)) continue
    for (const te of teList) {
      if (!te || typeof te !== "object") continue
      const e = te as { range?: unknown; newText?: unknown }
      const range = rangeDto(e.range)
      if (range && typeof e.newText === "string") edits.push({ resource, textEdit: { range, newText: e.newText } })
    }
  }
  return { edits }
}

export function toWorkspaceSymbolDtos(result: unknown): WorkspaceSymbolDto[] {
  if (!Array.isArray(result)) return []
  const out: WorkspaceSymbolDto[] = []
  for (const raw of result) {
    if (!raw || typeof raw !== "object") continue
    const s = raw as { name?: unknown; kind?: unknown; containerName?: unknown; location?: { uri?: unknown; range?: unknown } }
    if (typeof s.name !== "string") continue
    const uri = uriToFsPath(s.location?.uri)
    const range = rangeDto(s.location?.range)
    if (uri === undefined || !range) continue
    const dto: WorkspaceSymbolDto = { name: s.name, kind: typeof s.kind === "number" ? s.kind : 0, uri, range }
    if (typeof s.containerName === "string") dto.containerName = s.containerName
    out.push(dto)
  }
  return out
}

function commandDto(cmd: unknown): { id: string; title: string; arguments?: unknown[] } | undefined {
  const c = cmd as { command?: unknown; title?: unknown; arguments?: unknown } | undefined
  if (!c || typeof c.command !== "string" || typeof c.title !== "string") return undefined
  return Array.isArray(c.arguments) ? { id: c.command, title: c.title, arguments: c.arguments } : { id: c.command, title: c.title }
}

export function toCodeActionDtos(result: unknown): CodeActionDto[] {
  if (!Array.isArray(result)) return []
  const out: CodeActionDto[] = []
  for (const raw of result) {
    if (!raw || typeof raw !== "object") continue
    const a = raw as { title?: unknown; kind?: unknown; isPreferred?: unknown; edit?: unknown; command?: unknown; arguments?: unknown }
    if (typeof a.title !== "string") continue
    // Bare Command form: { title, command: <id string>, arguments? }.
    if (typeof a.command === "string") {
      out.push({ title: a.title, command: { id: a.command, title: a.title, ...(Array.isArray(a.arguments) ? { arguments: a.arguments } : {}) } })
      continue
    }
    const dto: CodeActionDto = { title: a.title }
    if (typeof a.kind === "string") dto.kind = a.kind
    else if (a.kind && typeof a.kind === "object" && "value" in a.kind && typeof a.kind.value === "string") dto.kind = a.kind.value
    if (a.isPreferred === true) dto.isPreferred = true
    const edit = toWorkspaceEditDto(a.edit); if (edit && edit.edits.length > 0) dto.edit = edit
    const cmd = commandDto(a.command); if (cmd) dto.command = cmd
    out.push(dto)
  }
  return out
}

function oneTextEdit(raw: unknown): TextEditDto | undefined {
  if (!raw || typeof raw !== "object") return undefined
  const e = raw as { range?: unknown; newText?: unknown }
  const range = rangeDto(e.range)
  return range && typeof e.newText === "string" ? { range, newText: e.newText } : undefined
}

function num(v: unknown): number { return typeof v === "number" ? v : 0 }

export function toColorInformationDtos(result: unknown): ColorInformationDto[] {
  if (!Array.isArray(result)) return []
  const out: ColorInformationDto[] = []
  for (const raw of result) {
    if (!raw || typeof raw !== "object") continue
    const c = raw as { range?: unknown; color?: unknown }
    const range = rangeDto(c.range)
    const col = c.color as { red?: unknown; green?: unknown; blue?: unknown; alpha?: unknown } | undefined
    if (!range || !col || typeof col.red !== "number") continue
    out.push({ range, color: { red: col.red, green: num(col.green), blue: num(col.blue), alpha: typeof col.alpha === "number" ? col.alpha : 1 } })
  }
  return out
}

export function toColorPresentationDtos(result: unknown): ColorPresentationDto[] {
  if (!Array.isArray(result)) return []
  const out: ColorPresentationDto[] = []
  for (const raw of result) {
    if (!raw || typeof raw !== "object") continue
    const p = raw as { label?: unknown; textEdit?: unknown; additionalTextEdits?: unknown }
    if (typeof p.label !== "string") continue
    const dto: ColorPresentationDto = { label: p.label }
    const te = oneTextEdit(p.textEdit); if (te) dto.textEdit = te
    if (Array.isArray(p.additionalTextEdits)) {
      const adds = p.additionalTextEdits.map(oneTextEdit).filter((e): e is TextEditDto => e !== undefined)
      if (adds.length > 0) dto.additionalTextEdits = adds
    }
    out.push(dto)
  }
  return out
}

/** Flatten one vscode SelectionRange's `.parent` chain (innermost → outermost)
 *  into the array Monaco expects for that cursor position. */
export function flattenSelectionRange(sr: unknown): RangeDto[] {
  const out: RangeDto[] = []
  let cur = sr as { range?: unknown; parent?: unknown } | undefined
  let guard = 0
  while (cur && typeof cur === "object" && guard++ < MAX_SELECTION_DEPTH) {
    const r = rangeDto(cur.range)
    if (r) out.push(r)
    cur = cur.parent as { range?: unknown; parent?: unknown } | undefined
  }
  return out
}

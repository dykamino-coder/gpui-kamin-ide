// `vscode.TextDocument` (B5) — line/offset maths over the live mirrored
// content. Reads the host document state lazily via `read()` so version/content
// always reflect the latest editor buffer; returns the closed shape once the
// document leaves the mirror. EOL is detected from content (CRLF vs LF).
import type { URI } from "vscode-uri"
import type { HostDocument } from "../host-services.js"
import { Position, Range } from "./classes-core.js"

const WORD_RE = /[A-Za-z0-9_]+/g

export class TextDocument {
  // `saver` persists the document's CURRENT mirror content to disk (backs
  // `save()`). Optional: untitled / content-provider snapshots have no disk
  // target, so their `save()` honestly resolves `false` instead of the old
  // always-`true` no-op that silently dropped the write.
  constructor(
    private readonly _uri: URI,
    private readonly read: () => HostDocument | undefined,
    private readonly saver?: () => Promise<boolean>,
  ) {}

  get uri(): URI { return this._uri }
  get fileName(): string { return this._uri.fsPath }
  get isUntitled(): boolean { return this._uri.scheme === "untitled" }
  get languageId(): string { return this.read()?.languageId ?? "plaintext" }
  get version(): number { return this.read()?.version ?? 0 }
  get isClosed(): boolean { return this.read() === undefined }
  readonly isDirty: boolean = false // dirty tracking arrives with B5b editors
  readonly encoding: string = "utf8"
  get eol(): number { return this.parsed().eolLen === 2 ? 2 : 1 } // CRLF=2, LF=1
  save(): Promise<boolean> { return this.saver ? this.saver() : Promise.resolve(false) }

  // Memoised line split — `getText`/`offsetAt` are called repeatedly by
  // formatters; re-scanning the whole buffer each time is O(n) per call. The
  // cache invalidates whenever the live content string changes (new version).
  private cache: { content: string; lines: string[]; eolLen: number; lineStarts: number[] } | null = null
  private parsed(): { content: string; lines: string[]; eolLen: number; lineStarts: number[] } {
    const content = this.read()?.content ?? ""
    if (this.cache?.content !== content) {
      const eol = content.includes("\r\n") ? "\r\n" : "\n"
      const lines = content.split(eol)
      // Prefix sum of line-start offsets → O(log n) positionAt, O(1) offsetAt.
      const lineStarts = new Array<number>(lines.length)
      let acc = 0
      for (let i = 0; i < lines.length; i++) { lineStarts[i] = acc; acc += (lines[i]?.length ?? 0) + eol.length }
      this.cache = { content, lines, eolLen: eol.length, lineStarts }
    }
    return this.cache
  }
  private text(): string { return this.parsed().content }
  private lines(): string[] { return this.parsed().lines }
  get lineCount(): number { return this.parsed().lines.length }

  getText(range?: Range): string {
    if (!range) return this.text()
    return this.text().slice(this.offsetAt(range.start), this.offsetAt(range.end))
  }

  lineAt(lineOrPosition: number | Position): {
    lineNumber: number; text: string; range: Range; rangeIncludingLineBreak: Range
    firstNonWhitespaceCharacterIndex: number; isEmptyOrWhitespace: boolean
  } {
    const lines = this.lines()
    const raw = typeof lineOrPosition === "number" ? lineOrPosition : lineOrPosition.line
    const line = Math.max(0, Math.min(raw, lines.length - 1))
    const text = lines[line] ?? ""
    const firstNonWs = text.search(/\S/)
    const hasNext = line + 1 < lines.length
    return {
      lineNumber: line,
      text,
      range: new Range(line, 0, line, text.length),
      rangeIncludingLineBreak: new Range(line, 0, hasNext ? line + 1 : line, hasNext ? 0 : text.length),
      firstNonWhitespaceCharacterIndex: firstNonWs < 0 ? text.length : firstNonWs,
      isEmptyOrWhitespace: text.trim().length === 0,
    }
  }

  offsetAt(position: Position): number {
    const { lines, lineStarts } = this.parsed()
    const line = Math.max(0, Math.min(position.line, lines.length - 1))
    return (lineStarts[line] ?? 0) + Math.max(0, Math.min(position.character, lines[line]?.length ?? 0))
  }

  positionAt(offset: number): Position {
    const { lines, lineStarts } = this.parsed()
    const target = Math.max(0, offset)
    // Binary search for the last line whose start offset is <= target.
    let lo = 0
    let hi = lines.length - 1
    while (lo < hi) {
      const mid = (lo + hi + 1) >> 1
      if ((lineStarts[mid] ?? 0) <= target) lo = mid
      else hi = mid - 1
    }
    // Clamp into the line (an offset inside the EOL clamps to end-of-line).
    const character = Math.min(target - (lineStarts[lo] ?? 0), lines[lo]?.length ?? 0)
    return new Position(lo, character)
  }

  getWordRangeAtPosition(position: Position, regex: RegExp = WORD_RE): Range | undefined {
    const text = this.lines()[position.line] ?? ""
    const re = new RegExp(regex.source, regex.flags.includes("g") ? regex.flags : `${regex.flags}g`)
    let m: RegExpExecArray | null
    while ((m = re.exec(text)) !== null) {
      if (m[0].length === 0) { re.lastIndex++; continue } // skip zero-width matches
      const start = m.index
      const end = start + m[0].length
      if (position.character >= start && position.character <= end) {
        return new Range(position.line, start, position.line, end)
      }
    }
    return undefined
  }

  validatePosition(position: Position): Position {
    const lines = this.lines()
    const line = Math.max(0, Math.min(position.line, lines.length - 1))
    const character = Math.max(0, Math.min(position.character, lines[line]?.length ?? 0))
    return new Position(line, character)
  }

  validateRange(range: Range): Range {
    return new Range(this.validatePosition(range.start), this.validatePosition(range.end))
  }
}

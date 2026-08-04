// Core data classes — geometry (Range, Position, Selection, Location),
// theming (Theme*), trees, transfers, edits, errors. These appear in
// nearly every extension's require-time evaluation.

// Real geometry (B5). Position/Range/Selection match the d.ts maths so
// formatters, linters and selection-aware extensions compute correctly.
export class Position {
  /** Type guard for position-LIKE values — providers often return plain
   *  `{ line, character }` literals rather than Position instances. */
  static isPosition(o: unknown): o is Position {
    if (!o || typeof o !== "object") return false
    const p = o as { line?: unknown; character?: unknown }
    return typeof p.line === "number" && typeof p.character === "number"
  }
  /** Normalize a Position or position-like into a real Position. */
  static of(o: Position | { line: number; character: number }): Position {
    return o instanceof Position ? o : new Position(o.line, o.character)
  }
  constructor(public readonly line: number, public readonly character: number) {
    // VS Code contract: coordinates are non-negative; a silently-negative
    // Position corrupts Monaco coords far from the cause.
    if (line < 0) throw new Error("Position.line must be non-negative")
    if (character < 0) throw new Error("Position.character must be non-negative")
  }
  isBefore(o: Position): boolean { return this.line < o.line || (this.line === o.line && this.character < o.character) }
  isBeforeOrEqual(o: Position): boolean { return this.isBefore(o) || this.isEqual(o) }
  isAfter(o: Position): boolean { return !this.isBeforeOrEqual(o) }
  isAfterOrEqual(o: Position): boolean { return !this.isBefore(o) }
  isEqual(o: Position): boolean { return this.line === o.line && this.character === o.character }
  compareTo(o: Position): number { return this.isBefore(o) ? -1 : this.isAfter(o) ? 1 : 0 }
  translate(lineDelta?: number | { lineDelta?: number; characterDelta?: number }, characterDelta?: number): Position {
    if (typeof lineDelta === "object") {
      return new Position(this.line + (lineDelta.lineDelta ?? 0), this.character + (lineDelta.characterDelta ?? 0))
    }
    return new Position(this.line + (lineDelta ?? 0), this.character + (characterDelta ?? 0))
  }
  with(line?: number | { line?: number; character?: number }, character?: number): Position {
    if (typeof line === "object") {
      return new Position(line.line ?? this.line, line.character ?? this.character)
    }
    return new Position(line ?? this.line, character ?? this.character)
  }
}

export class Range {
  static isRange(o: unknown): o is Range {
    if (o instanceof Range) return true
    if (!o || typeof o !== "object") return false
    const r = o as { start?: unknown; end?: unknown }
    return Position.isPosition(r.start) && Position.isPosition(r.end)
  }
  static of(o: Range | { start: Position; end: Position }): Range {
    return o instanceof Range ? o : new Range(Position.of(o.start), Position.of(o.end))
  }
  readonly start: Position
  readonly end: Position
  constructor(start: Position | number, startCharacter: Position | number, endLine?: number, endCharacter?: number) {
    // Two forms: (Position-like, Position-like) or (sl, sc, el, ec). Accept
    // position-LIKE objects (not just instances) so provider literals work.
    const a = typeof start === "number" ? new Position(start, startCharacter as number) : Position.of(start)
    const b = typeof start === "number" ? new Position(endLine ?? 0, endCharacter ?? 0) : Position.of(startCharacter as Position)
    // VS Code normalizes so `start` is the earlier position.
    if (a.isBeforeOrEqual(b)) { this.start = a; this.end = b } else { this.start = b; this.end = a }
  }
  get isEmpty(): boolean { return this.start.isEqual(this.end) }
  get isSingleLine(): boolean { return this.start.line === this.end.line }
  contains(posOrRange: Position | Range): boolean {
    if (posOrRange instanceof Range) return this.contains(posOrRange.start) && this.contains(posOrRange.end)
    return posOrRange.isAfterOrEqual(this.start) && posOrRange.isBeforeOrEqual(this.end)
  }
  isEqual(o: Range): boolean { return this.start.isEqual(o.start) && this.end.isEqual(o.end) }
  intersection(o: Range): Range | undefined {
    const start = this.start.isAfter(o.start) ? this.start : o.start
    const end = this.end.isBefore(o.end) ? this.end : o.end
    return start.isAfter(end) ? undefined : new Range(start, end)
  }
  union(o: Range): Range {
    const start = this.start.isBefore(o.start) ? this.start : o.start
    const end = this.end.isAfter(o.end) ? this.end : o.end
    return new Range(start, end)
  }
  with(start?: Position | { start?: Position; end?: Position }, end?: Position): Range {
    if (start instanceof Position || start === undefined) return new Range(start ?? this.start, end ?? this.end)
    return new Range(start.start ?? this.start, start.end ?? this.end)
  }
}

export class Selection extends Range {
  readonly anchor: Position
  readonly active: Position
  constructor(anchor: Position | number, active: Position | number, activeLine?: number, activeCharacter?: number) {
    const a = anchor instanceof Position ? anchor : new Position(anchor, active as number)
    const b = active instanceof Position ? active : new Position(activeLine ?? 0, activeCharacter ?? 0)
    super(a, b)
    this.anchor = a
    this.active = b
  }
  get isReversed(): boolean { return this.active.isBefore(this.anchor) }
}
export class Location { constructor(public uri: unknown, public range: unknown) {} }
export class LocationLink {}
export class MarkdownString {
  isTrusted = false
  constructor(public value = "", public supportThemeIcons = false) {}
  appendText(v: string): this { this.value += v; return this }
  appendMarkdown(v: string): this { this.value += v; return this }
  appendCodeblock(v: string): this { this.value += v; return this }
}
export class ThemeColor { constructor(public id: string) {} }
export class ThemeIcon {
  static readonly File = { id: "file" }
  static readonly Folder = { id: "folder" }
  constructor(public id: string, public color?: ThemeColor) {}
}
export class TreeItem { constructor(public label: unknown, public collapsibleState?: number) {} }
export class DataTransferItem {
  constructor(public value: unknown) {}
  asString(): Promise<string> { return Promise.resolve(String(this.value)) }
  asFile(): undefined { return undefined }
}
export class DataTransfer {
  // Real backing store: tree DnD runs handleDrag + handleDrop both in the host,
  // so items set during a drag are read back on drop without crossing the WS.
  // Mime types are case-insensitive (VS Code parity).
  private readonly items = new Map<string, DataTransferItem>()
  get(mime: string): DataTransferItem | undefined { return this.items.get(mime.toLowerCase()) }
  set(mime: string, item: DataTransferItem): void { this.items.set(mime.toLowerCase(), item) }
  forEach(cb: (item: DataTransferItem, mime: string, dt: DataTransfer) => void): void {
    for (const [mime, item] of this.items) cb(item, mime, this)
  }
  *[Symbol.iterator](): IterableIterator<[string, DataTransferItem]> { yield* this.items }
}
/** One stored text edit inside a WorkspaceEdit (range + replacement text). */
interface StoredTextEdit { range: unknown; newText: string }
/** Options vscode's createFile/deleteFile/renameFile accept (union of the
 *  three shapes; each op reads only its own fields). */
export interface WorkspaceFileOpOptions {
  overwrite?: boolean
  ignoreIfExists?: boolean
  ignoreIfNotExists?: boolean
  recursive?: boolean
  // DataTransferFile.data() в d.ts обязателен — `?` не нужен.
  contents?: Uint8Array | { data: () => Promise<Uint8Array> }
}
/** A file operation a WorkspaceEdit can carry alongside text edits.
 *  `metadata` (WorkspaceEditEntryMetadata: needsConfirmation/label/…) пока не
 *  рендерится (нет refactor-preview UI), но СОХРАНЯЕТСЯ — иначе данные негде
 *  будет взять, когда UI появится (ревью API-fidelity). */
export interface WorkspaceFileOp { type: "create" | "delete" | "rename"; uri?: unknown; from?: unknown; to?: unknown; options?: WorkspaceFileOpOptions | undefined; metadata?: unknown }

/** Real WorkspaceEdit (B6l) — stores per-resource text edits + file ops so
 *  rename / code-action providers actually carry their changes to the host.
 *  Previously a no-op stub, which silently dropped every edit. */
export class WorkspaceEdit {
  private readonly textEdits = new Map<string, { uri: unknown; edits: StoredTextEdit[] }>()
  readonly fileOps: WorkspaceFileOp[] = []

  private keyOf(uri: unknown): string { return String((uri as { toString?: () => string })?.toString?.() ?? uri) }
  private bucket(uri: unknown): { uri: unknown; edits: StoredTextEdit[] } {
    const k = this.keyOf(uri)
    let b = this.textEdits.get(k)
    if (!b) { b = { uri, edits: [] }; this.textEdits.set(k, b) }
    return b
  }
  get size(): number { return this.textEdits.size + this.fileOps.length }
  replace(uri: unknown, range: unknown, newText: string): void { this.bucket(uri).edits.push({ range, newText }) }
  insert(uri: unknown, position: unknown, newText: string): void { this.bucket(uri).edits.push({ range: { start: position, end: position }, newText }) }
  delete(uri: unknown, range: unknown): void { this.bucket(uri).edits.push({ range, newText: "" }) }
  has(uri: unknown): boolean { return this.textEdits.has(this.keyOf(uri)) }
  set(uri: unknown, edits: StoredTextEdit[]): void { this.bucket(uri).edits = [...(edits ?? [])] }
  get(uri: unknown): StoredTextEdit[] { return this.textEdits.get(this.keyOf(uri))?.edits ?? [] }
  entries(): [unknown, StoredTextEdit[]][] { return [...this.textEdits.values()].map((b) => [b.uri, b.edits]) }
  createFile(uri: unknown, options?: WorkspaceFileOpOptions, metadata?: unknown): void { this.fileOps.push({ type: "create", uri, options, metadata }) }
  deleteFile(uri: unknown, options?: WorkspaceFileOpOptions, metadata?: unknown): void { this.fileOps.push({ type: "delete", uri, options, metadata }) }
  renameFile(from: unknown, to: unknown, options?: WorkspaceFileOpOptions, metadata?: unknown): void { this.fileOps.push({ type: "rename", from, to, options, metadata }) }
}

class TextEditCtor { constructor(public range: unknown, public newText: string) {} }
export const TextEdit = Object.assign(TextEditCtor, {
  replace: (range: unknown, newText: string) => ({ range, newText }),
  insert: (position: unknown, newText: string) => ({ range: { start: position, end: position }, newText }),
  delete: (range: unknown) => ({ range, newText: "" }),
  setEndOfLine: (eol: number) => ({ newEol: eol }),
})
export type TextEdit = TextEditCtor

class SnippetTextEditCtor {}
export const SnippetTextEdit = Object.assign(SnippetTextEditCtor, {
  replace: () => ({}), insert: () => ({}),
})

export class RelativePattern { constructor(public base: unknown, public pattern: string) {} }

export class CancellationTokenSource {
  token = { isCancellationRequested: false, onCancellationRequested: () => ({ dispose() {} }) }
  cancel(): void { this.token.isCancellationRequested = true }
  dispose(): void {}
}
export class CancellationError extends Error { constructor() { super("Canceled"); this.name = "Canceled" } }

class FileSystemErrorCtor extends Error { code = "Unknown" }
const errorOf = (code: string) => (msg?: string) => Object.assign(new Error(msg), { code })
export const FileSystemError = Object.assign(FileSystemErrorCtor, {
  FileNotFound: errorOf("FileNotFound"),
  FileExists: errorOf("FileExists"),
  FileNotADirectory: errorOf("FileNotADirectory"),
  FileIsADirectory: errorOf("FileIsADirectory"),
  NoPermissions: errorOf("NoPermissions"),
  Unavailable: errorOf("Unavailable"),
})

export class FileDecoration {
  propagate = false
  constructor(public badge?: string, public tooltip?: string, public color?: ThemeColor) {}
}

export class QuickPickItem {}
export const QuickInputButtons = { Back: { iconPath: { id: "arrow-left" } } }

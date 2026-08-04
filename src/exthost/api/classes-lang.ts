// Language-feature classes: diagnostics, completion, hover, signature
// help, code actions, semantic tokens, inlay hints, folding, calls,
// type hierarchy, colors. These are what LSP-bridging extensions
// (ms-python, redhat.java, golang.Go, ruby-lsp) extend.

export class DocumentLink { constructor(public range: unknown, public target?: unknown) {} }
export class DocumentSymbol {
  children: DocumentSymbol[] = []
  constructor(public name: string, public detail: string, public kind: number, public range: unknown, public selectionRange: unknown) {}
}
export class SymbolInformation {
  constructor(public name: string, public kind: number, public containerName: string, public location: unknown) {}
}
export class CompletionItem { constructor(public label: unknown, public kind?: number) {} }
export class CompletionList<T = unknown> {
  constructor(public items: T[] = [], public isIncomplete = false) {}
}
/** Escape snippet-significant chars (`$`, `}`, `\`) in literal text. */
function snippetEscape(s: string): string { return s.replace(/\$|}|\\/g, "\\$&") }
type NestedSnippet = (s: SnippetString) => void

export class SnippetString {
  private tabstop = 1
  constructor(public value = "") {}
  appendText(v: string): this { this.value += snippetEscape(v); return this }
  appendTabstop(n: number = this.tabstop++): this { this.value += `$${String(n)}`; return this }
  appendPlaceholder(v: string | NestedSnippet, n: number = this.tabstop++): this {
    const inner = typeof v === "function" ? this.nested(v) : snippetEscape(v)
    this.value += `\${${String(n)}:${inner}}`
    return this
  }
  appendChoice(values: readonly string[], n: number = this.tabstop++): this {
    const choices = values.map((c) => c.replace(/\$|}|\\|,|\|/g, "\\$&")).join(",")
    this.value += `\${${String(n)}|${choices}|}`
    return this
  }
  appendVariable(name: string, defaultValue: string | NestedSnippet): this {
    const inner = typeof defaultValue === "function" ? this.nested(defaultValue) : snippetEscape(defaultValue)
    this.value += inner ? `\${${name}:${inner}}` : `$${name}`
    return this
  }
  private nested(fn: NestedSnippet): string {
    const s = new SnippetString()
    s.tabstop = this.tabstop
    fn(s)
    this.tabstop = s.tabstop
    return s.value
  }
}
export class Hover {
  contents: unknown[]
  constructor(contents: unknown, public range?: unknown) { this.contents = Array.isArray(contents) ? contents : [contents] }
}
export class SignatureHelp { signatures: unknown[] = []; activeSignature = 0; activeParameter = 0 }
export class SignatureInformation {
  parameters: unknown[] = []
  constructor(public label: unknown, public documentation?: unknown) {}
}
export class ParameterInformation { constructor(public label: unknown, public documentation?: unknown) {} }
export class Diagnostic {
  source?: string; code?: unknown
  relatedInformation: unknown[] = []; tags: number[] = []
  constructor(public range: unknown, public message: string, public severity = 0) {}
}
export class DiagnosticRelatedInformation { constructor(public location: unknown, public message: string) {} }
export class CodeAction { constructor(public title: string, public kind?: unknown) {} }

/** d.ts §1241: static members like `QuickFix` are themselves
 *  `CodeActionKind` instances — extensions chain `.append("foo")` off
 *  them. Plain `{ value }` objects break that contract. */
export class CodeActionKind {
  static readonly Empty = new CodeActionKind("")
  static readonly QuickFix = new CodeActionKind("quickfix")
  static readonly Refactor = new CodeActionKind("refactor")
  static readonly RefactorExtract = new CodeActionKind("refactor.extract")
  static readonly RefactorInline = new CodeActionKind("refactor.inline")
  static readonly RefactorMove = new CodeActionKind("refactor.move")
  static readonly RefactorRewrite = new CodeActionKind("refactor.rewrite")
  static readonly Source = new CodeActionKind("source")
  static readonly SourceOrganizeImports = new CodeActionKind("source.organizeImports")
  static readonly SourceFixAll = new CodeActionKind("source.fixAll")
  static readonly Notebook = new CodeActionKind("notebook")
  constructor(public value: string) {}
  append(v: string): CodeActionKind { return new CodeActionKind(`${this.value}.${v}`) }
  intersects(other: CodeActionKind): boolean {
    return this.value === other?.value || (other?.value ?? "").startsWith(`${this.value}.`)
  }
  contains(other: CodeActionKind): boolean { return this.intersects(other) }
}
export class CodeLens {
  isResolved = false
  constructor(public range: unknown, public command?: unknown) {}
}
export class CallHierarchyItem {}
export class CallHierarchyIncomingCall {}
export class CallHierarchyOutgoingCall {}
export class TypeHierarchyItem {}
export class InlineValueText {}
export class InlineValueVariableLookup {}
export class InlineValueEvaluatableExpression {}
export class InlineValueContext {}
export class InlayHint {
  paddingLeft?: boolean; paddingRight?: boolean; tooltip?: unknown; textEdits?: unknown[]
  constructor(public position: unknown, public label: unknown, public kind?: number) {}
}
export class InlayHintLabelPart { constructor(public value: string) {} }
export class SemanticTokens { constructor(public data: Uint32Array, public resultId?: string) {} }
export class SemanticTokensBuilder { push(): void {} build(): { data: Uint32Array } { return { data: new Uint32Array(0) } } }
export class SemanticTokensLegend { constructor(public tokenTypes: string[] = [], public tokenModifiers: string[] = []) {} }
export class SemanticTokensEdits {}
export class SemanticTokensEdit {}
export class DocumentHighlight { constructor(public range: unknown, public kind = 0) {} }
export class ColorInformation { constructor(public range: unknown, public color: unknown) {} }
export class ColorPresentation {
  textEdit?: unknown; additionalTextEdits?: unknown[]
  constructor(public label: string) {}
}
export class Color { constructor(public red: number, public green: number, public blue: number, public alpha: number) {} }
export class FoldingRange { constructor(public start: number, public end: number, public kind?: number) {} }
export class SelectionRange { constructor(public range: unknown, public parent?: unknown) {} }
export class LinkedEditingRanges {}
export class LinkedEditingRange {}
export class LanguageStatusItem {}
export class LogOutputChannel {}
export class DocumentLinkProvider {}

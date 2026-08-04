// `when`-clause expression engine (B4). Evaluates VS Code context
// expressions (menu/keybinding/view `when`) against a context-key map.
// Supports the operators real extensions use: `!`, `&&`, `||`, `==`, `!=`,
// `=~`, `<`, `<=`, `>`, `>=`, `in`/`not in`, parentheses, bare-key
// truthiness, and `true`/`false`/number/`'string'`/`/regex/` literals.
// Unknown keys evaluate to undefined (falsy), as in VS Code.

export type ContextValues = Record<string, unknown>

type Tok =
  | { t: "op"; v: string }
  | { t: "ident"; v: string }
  | { t: "str"; v: string }
  | { t: "num"; v: number }
  | { t: "regex"; v: RegExp }

type Node =
  | { k: "or" | "and"; l: Node; r: Node }
  | { k: "not"; e: Node }
  | { k: "cmp"; op: string; l: Node; r: Node }
  | { k: "match"; l: Node; r: RegExp }
  | { k: "key"; name: string }
  | { k: "lit"; v: string | number | boolean }

const TWO_CHAR = new Set(["&&", "||", "==", "!=", ">=", "<=", "=~"])
const IDENT_START = /[A-Za-z_]/
const IDENT_BODY = /[A-Za-z0-9_.:-]/

function tokenize(src: string): Tok[] {
  const toks: Tok[] = []
  let i = 0
  while (i < src.length) {
    const c = src[i] ?? ""
    if (c === " " || c === "\t") { i++; continue }
    // Two-char operators BEFORE single — so `!=` isn't read as `!` then `=`.
    const two = src.slice(i, i + 2)
    if (TWO_CHAR.has(two)) { toks.push({ t: "op", v: two }); i += 2; continue }
    if (c === "(" || c === ")" || c === "!" || c === "<" || c === ">") { toks.push({ t: "op", v: c }); i++; continue }
    if (c === "'" || c === '"') {
      let j = i + 1
      while (j < src.length && src[j] !== c) j++
      if (j >= src.length) throw new Error("when-clause: unterminated string")
      toks.push({ t: "str", v: src.slice(i + 1, j) }); i = j + 1; continue
    }
    if (c === "/") {
      let j = i + 1
      while (j < src.length && src[j] !== "/") { if (src[j] === "\\") j++; j++ }
      if (j >= src.length) throw new Error("when-clause: unterminated regex")
      const pattern = src.slice(i + 1, j)
      j++
      let flags = ""
      while (j < src.length && IDENT_START.test(src[j] ?? "")) { flags += src[j] ?? ""; j++ }
      toks.push({ t: "regex", v: new RegExp(pattern, flags) }); i = j; continue
    }
    if (c >= "0" && c <= "9") {
      let j = i
      while (j < src.length && ((src[j] ?? "") >= "0" && (src[j] ?? "") <= "9" || src[j] === ".")) j++
      toks.push({ t: "num", v: Number(src.slice(i, j)) }); i = j; continue
    }
    if (IDENT_START.test(c)) {
      let j = i
      while (j < src.length && IDENT_BODY.test(src[j] ?? "")) j++
      toks.push({ t: "ident", v: src.slice(i, j) }); i = j; continue
    }
    throw new Error(`when-clause: unexpected '${c}' at ${i}`)
  }
  return toks
}

const COMPARE_OPS = new Set(["==", "!=", "<", "<=", ">", ">="])

class Parser {
  private i = 0
  constructor(private readonly toks: Tok[]) {}
  private peek(): Tok | undefined { return this.toks[this.i] }
  private isOp(v: string): boolean { const t = this.peek(); return t?.t === "op" && t.v === v }
  private isIdent(v: string): boolean { const t = this.peek(); return t?.t === "ident" && t.v === v }

  parse(): Node {
    const n = this.or()
    if (this.peek()) throw new Error("when-clause: trailing tokens")
    return n
  }
  private or(): Node {
    let l = this.and()
    while (this.isOp("||")) { this.i++; l = { k: "or", l, r: this.and() } }
    return l
  }
  private and(): Node {
    let l = this.comparison()
    while (this.isOp("&&")) { this.i++; l = { k: "and", l, r: this.comparison() } }
    return l
  }
  // `!` binds tighter than comparisons (VS Code BNF: || < && < ! < cmp), so
  // `!key == v` is `(!key) == v`, not `!(key == v)`.
  private unary(): Node {
    if (this.isOp("!")) { this.i++; return { k: "not", e: this.unary() } }
    return this.primary()
  }
  private comparison(): Node {
    const l = this.unary()
    const t = this.peek()
    if (t?.t === "op" && t.v === "=~") {
      this.i++
      const rx = this.peek()
      if (rx?.t !== "regex") throw new Error("when-clause: =~ expects a /regex/")
      this.i++
      return { k: "match", l, r: rx.v }
    }
    if (t?.t === "op" && COMPARE_OPS.has(t.v)) { this.i++; return { k: "cmp", op: t.v, l, r: this.value() } }
    if (this.isIdent("in")) { this.i++; return { k: "cmp", op: "in", l, r: this.primary() } }
    if (this.isIdent("not")) {
      this.i++
      if (!this.isIdent("in")) throw new Error("when-clause: expected 'in' after 'not'")
      this.i++
      return { k: "not", e: { k: "cmp", op: "in", l, r: this.primary() } }
    }
    return l
  }
  private primary(): Node {
    const t = this.toks[this.i++]
    if (!t) throw new Error("when-clause: unexpected end")
    if (t.t === "op" && t.v === "(") {
      const e = this.or()
      if (!this.isOp(")")) throw new Error("when-clause: expected ')'")
      this.i++
      return e
    }
    if (t.t === "ident") {
      if (t.v === "true") return { k: "lit", v: true }
      if (t.v === "false") return { k: "lit", v: false }
      return { k: "key", name: t.v }
    }
    if (t.t === "str") return { k: "lit", v: t.v }
    if (t.t === "num") return { k: "lit", v: t.v }
    throw new Error("when-clause: unexpected token")
  }

  /** Right-hand side of a comparison: a literal VALUE, never a context-key
   *  lookup. `editorLangId == typescript` compares against the string
   *  "typescript", not the context key `typescript` (VS Code semantics). */
  private value(): Node {
    const t = this.toks[this.i++]
    if (!t) throw new Error("when-clause: unexpected end")
    if (t.t === "ident") {
      if (t.v === "true") return { k: "lit", v: true }
      if (t.v === "false") return { k: "lit", v: false }
      return { k: "lit", v: t.v }
    }
    if (t.t === "str") return { k: "lit", v: t.v }
    if (t.t === "num") return { k: "lit", v: t.v }
    throw new Error("when-clause: unexpected token")
  }
}

// Deliberate divergence from VS Code (which uses `.toString()`): objects are
// JSON-stringified for comparison. Context keys are rarely objects, and JSON
// gives a more useful string than "[object Object]" for the array/`in` path.
function coerceStr(v: unknown): string {
  if (typeof v === "string") return v
  if (typeof v === "number" || typeof v === "boolean") return String(v)
  return v == null ? "" : JSON.stringify(v)
}

function looseEq(a: unknown, b: unknown): boolean {
  return a === b || coerceStr(a) === coerceStr(b)
}

function memberOf(a: unknown, b: unknown): boolean {
  if (Array.isArray(b)) return b.some((x) => looseEq(x, a))
  // VS Code also allows a quoted comma-separated string RHS:
  // `resourceExtname in '.js,.ts'`.
  if (typeof b === "string") return b.split(",").some((x) => looseEq(x.trim(), a))
  if (b && typeof b === "object") return coerceStr(a) in (b as Record<string, unknown>)
  return false
}

function evalNode(n: Node, ctx: ContextValues): unknown {
  switch (n.k) {
    case "or": return toBool(evalNode(n.l, ctx)) || toBool(evalNode(n.r, ctx))
    case "and": return toBool(evalNode(n.l, ctx)) && toBool(evalNode(n.r, ctx))
    case "not": return !toBool(evalNode(n.e, ctx))
    case "key": return ctx[n.name]
    case "lit": return n.v
    case "match": return n.r.test(coerceStr(evalNode(n.l, ctx)))
    case "cmp": {
      const a = evalNode(n.l, ctx); const b = evalNode(n.r, ctx)
      switch (n.op) {
        case "==": return looseEq(a, b)
        case "!=": return !looseEq(a, b)
        case "<": return Number(a) < Number(b)
        case "<=": return Number(a) <= Number(b)
        case ">": return Number(a) > Number(b)
        case ">=": return Number(a) >= Number(b)
        case "in": return memberOf(a, b)
        default: return false
      }
    }
    default: return assertNever(n)
  }
}

/** Compile-time exhaustiveness guard — adding a `Node` variant without an
 *  `evalNode` case becomes a type error here. */
function assertNever(n: never): never {
  throw new Error(`when-clause: unhandled node ${JSON.stringify(n)}`)
}

function toBool(v: unknown): boolean {
  return v !== undefined && v !== null && v !== false && v !== "" && v !== "false" && v !== 0 && v !== "0"
}

// Compile cache: a `when` string is tokenized+parsed ONCE, then the AST is
// reused for every evaluate. VS Code does the same (parse at registration).
// Without it we re-lexed+re-parsed (and recompiled `=~` RegExps) on every
// keystroke and every Monaco-focus flip. `null` caches a parse failure so a
// malformed clause doesn't re-throw+re-log each time. Keyed by the finite set
// of manifest `when` strings, so unbounded growth isn't a concern.
const compileCache = new Map<string, Node | null>()

function compile(expr: string): Node | null {
  const cached = compileCache.get(expr)
  if (cached !== undefined) return cached
  let ast: Node | null = null
  try {
    ast = new Parser(tokenize(expr)).parse()
  } catch (err) {
    console.warn(`when-clause: failed to parse "${expr}":`, err)
  }
  compileCache.set(expr, ast)
  return ast
}

/** Evaluate a `when` expression against the context. An empty/absent
 *  expression is always true (no condition). A malformed expression evaluates
 *  to false (fail-closed, as VS Code does for broken clauses). */
export function evaluateWhen(expr: string | undefined, ctx: ContextValues): boolean {
  if (!expr || expr.trim() === "") return true
  const ast = compile(expr)
  return ast === null ? false : toBool(evalNode(ast, ctx))
}

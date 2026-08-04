// B4 — the `when`-clause expression engine.
import { describe, expect, it } from "vitest"
import { evaluateWhen } from "./when-clause.js"

const ctx = {
  editorFocus: true,
  sidebarVisible: false,
  resourceLangId: "typescript",
  view: "explorer",
  gitOpenRepoCount: 2,
  listMultiSelection: false,
  resourceExtname: ".ts",
  tags: ["a", "b"],
}

describe("evaluateWhen — basics", () => {
  it("empty/undefined clause is always true", () => {
    expect(evaluateWhen(undefined, ctx)).toBe(true)
    expect(evaluateWhen("", ctx)).toBe(true)
    expect(evaluateWhen("   ", ctx)).toBe(true)
  })

  it("bare key truthiness; unknown keys are falsy", () => {
    expect(evaluateWhen("editorFocus", ctx)).toBe(true)
    expect(evaluateWhen("sidebarVisible", ctx)).toBe(false)
    expect(evaluateWhen("nope", ctx)).toBe(false)
  })

  it("negation", () => {
    expect(evaluateWhen("!sidebarVisible", ctx)).toBe(true)
    expect(evaluateWhen("!editorFocus", ctx)).toBe(false)
    expect(evaluateWhen("!!editorFocus", ctx)).toBe(true)
  })
})

describe("evaluateWhen — logical precedence", () => {
  it("&& binds tighter than ||", () => {
    // false && X || true  → (false&&X) || true → true
    expect(evaluateWhen("sidebarVisible && editorFocus || editorFocus", ctx)).toBe(true)
    expect(evaluateWhen("editorFocus && sidebarVisible", ctx)).toBe(false)
    expect(evaluateWhen("editorFocus && !sidebarVisible", ctx)).toBe(true)
  })

  it("parentheses override precedence", () => {
    expect(evaluateWhen("editorFocus && (sidebarVisible || editorFocus)", ctx)).toBe(true)
    expect(evaluateWhen("(editorFocus || sidebarVisible) && sidebarVisible", ctx)).toBe(false)
  })
})

describe("evaluateWhen — comparisons", () => {
  it("== and != against string/number/bool literals", () => {
    expect(evaluateWhen("resourceLangId == typescript", ctx)).toBe(true)
    expect(evaluateWhen("resourceLangId == 'typescript'", ctx)).toBe(true)
    expect(evaluateWhen("resourceLangId != python", ctx)).toBe(true)
    expect(evaluateWhen("editorFocus == true", ctx)).toBe(true)
    expect(evaluateWhen("gitOpenRepoCount == 2", ctx)).toBe(true)
  })

  it("numeric ordering", () => {
    expect(evaluateWhen("gitOpenRepoCount > 1", ctx)).toBe(true)
    expect(evaluateWhen("gitOpenRepoCount >= 2", ctx)).toBe(true)
    expect(evaluateWhen("gitOpenRepoCount < 2", ctx)).toBe(false)
    expect(evaluateWhen("gitOpenRepoCount <= 2", ctx)).toBe(true)
  })

  it("regex match with =~", () => {
    expect(evaluateWhen("resourceExtname =~ /\\.(ts|js)$/", ctx)).toBe(true)
    expect(evaluateWhen("resourceLangId =~ /^type/", ctx)).toBe(true)
    expect(evaluateWhen("resourceLangId =~ /^py/", ctx)).toBe(false)
  })

  it("in / not in against an array key", () => {
    expect(evaluateWhen("'a' in tags", ctx)).toBe(true)
    expect(evaluateWhen("'z' in tags", ctx)).toBe(false)
    expect(evaluateWhen("'z' not in tags", ctx)).toBe(true)
  })

  it("in against an unknown key is false; not in is true", () => {
    expect(evaluateWhen("'a' in unknownKey", ctx)).toBe(false)
    expect(evaluateWhen("'a' not in unknownKey", ctx)).toBe(true)
  })

  it("in against a comma-separated string literal (VS Code form)", () => {
    expect(evaluateWhen("resourceExtname in '.ts,.js'", ctx)).toBe(true)
    expect(evaluateWhen("resourceExtname in '.py,.rb'", ctx)).toBe(false)
  })

  it("! binds tighter than comparison: !key == v is (!key) == v", () => {
    // editorFocus is true → (!editorFocus) == true → false == true → false
    expect(evaluateWhen("!editorFocus == true", ctx)).toBe(false)
    // sidebarVisible is false → (!sidebarVisible) == true → true == true → true
    expect(evaluateWhen("!sidebarVisible == true", ctx)).toBe(true)
  })

  it("regex match honours flags", () => {
    expect(evaluateWhen("resourceLangId =~ /^TYPE/i", ctx)).toBe(true)
    expect(evaluateWhen("resourceLangId =~ /^TYPE/", ctx)).toBe(false)
  })

  it("unterminated string/regex fail closed", () => {
    expect(evaluateWhen("resourceLangId == 'typ", ctx)).toBe(false)
    expect(evaluateWhen("resourceLangId =~ /typ", ctx)).toBe(false)
  })
})

describe("evaluateWhen — realistic clauses", () => {
  it("evaluates a typical menu when", () => {
    expect(evaluateWhen("view == explorer && !listMultiSelection", ctx)).toBe(true)
    expect(evaluateWhen("editorFocus && resourceLangId == typescript", ctx)).toBe(true)
  })

  it("fails closed on malformed input", () => {
    expect(evaluateWhen("editorFocus &&", ctx)).toBe(false)
    expect(evaluateWhen("( unbalanced", ctx)).toBe(false)
  })

  it("caches the parsed AST, not the result (re-evaluates against new ctx)", () => {
    const expr = "editorFocus && resourceLangId == typescript"
    expect(evaluateWhen(expr, ctx)).toBe(true)
    expect(evaluateWhen(expr, { ...ctx, editorFocus: false })).toBe(false)
    expect(evaluateWhen(expr, { ...ctx, resourceLangId: "python" })).toBe(false)
    expect(evaluateWhen(expr, ctx)).toBe(true)
  })
})

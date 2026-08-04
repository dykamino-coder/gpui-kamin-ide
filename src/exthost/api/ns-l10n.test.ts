import { describe, expect, it } from "vitest"
import { buildL10n } from "./ns-l10n.js"

describe("vscode.l10n.t — placeholder substitution", () => {
  const l10n = buildL10n()
  const t = (m: Parameters<typeof l10n.t>[0], ...a: unknown[]): string => l10n.t(m, ...a)

  it("substitutes positional {0}/{1} from spread args", () => {
    expect(t("not found: {0}", "C:/x")).toBe("not found: C:/x")
    expect(t("{0} of {1}", 2, 5)).toBe("2 of 5")
  })

  it("substitutes named {key} from a single Record arg", () => {
    expect(t("hello {name}", { name: "Kamin" })).toBe("hello Kamin")
  })

  it("handles the {message,args} object form", () => {
    expect(t({ message: "v{0}", args: ["1.2"] })).toBe("v1.2")
    expect(t({ message: "{a}", args: { a: "x" } })).toBe("x")
  })

  it("leaves unmatched placeholders + plain strings intact", () => {
    expect(t("plain")).toBe("plain")
    expect(t("{0} {1}", "only")).toBe("only {1}") // {1} has no arg → kept
  })
})

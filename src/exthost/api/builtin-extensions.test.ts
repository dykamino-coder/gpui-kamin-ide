// #22 — vscode.extensions exposes built-in facades so third-party extensions
// that feature-detect a builtin (e.g. Vue.volar reading
// getExtension("vscode.typescript-language-features").isActive) don't crash.
import { describe, expect, it } from "vitest"
import type { NsHooks } from "./ns-builders.js"
import { buildExtensions } from "./ns-data.js"
import type { ExtensionFacade } from "./types.js"

const TS_ID = "vscode.typescript-language-features"
const hooks = (list: ExtensionFacade[]) => ({ listExtensions: () => list } as unknown as NsHooks)

describe("extensions namespace — builtin facades", () => {
  it("getExtension returns an active facade for a builtin id", () => {
    const ts = buildExtensions(hooks([])).getExtension(TS_ID)
    expect(ts?.id).toBe(TS_ID)
    expect(ts?.isActive).toBe(true)
    expect(typeof ts?.activate).toBe("function")
  })

  it("a real extension with the same id wins over its builtin facade", () => {
    const real = { id: TS_ID, isActive: false } as unknown as ExtensionFacade
    const ext = buildExtensions(hooks([real]))
    expect(ext.getExtension(TS_ID)).toBe(real)
    // de-duped in `all` — the builtin facade is suppressed when shadowed.
    expect(ext.all.filter((e) => e.id === TS_ID)).toHaveLength(1)
  })

  it("`all` includes builtins not shadowed by an installed extension", () => {
    expect(buildExtensions(hooks([])).all.some((e) => e.id === TS_ID)).toBe(true)
  })

  it("returns undefined for an unknown id (not a builtin)", () => {
    expect(buildExtensions(hooks([])).getExtension("nope.nope")).toBeUndefined()
  })
})

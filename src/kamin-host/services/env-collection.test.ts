// env-collection — the parent applies extensions' environmentVariableCollection
// mutations to terminal spawn env (#11). Persistence is exercised separately;
// here we cover the in-memory merge + replace/append/prepend + drop semantics.
import { describe, expect, it, beforeEach } from "vitest"
import { applyEnvCollections, dropEnvCollection, syncEnvCollection } from "./env-collection.js"

// Replace=1, Append=2, Prepend=3 (vscode.EnvironmentVariableMutatorType).
const REPLACE = 1, APPEND = 2, PREPEND = 3

beforeEach(() => {
  // Session-scoped store (no disk); clear any state from prior tests.
  for (const id of ["a", "b", "c"]) dropEnvCollection(id)
})

describe("applyEnvCollections", () => {
  it("replace overwrites, append/prepend concatenate around the current value", () => {
    syncEnvCollection("a", { persistent: false, vars: { FOO: { type: REPLACE, value: "x" } } })
    syncEnvCollection("b", { persistent: false, vars: { PATH: { type: PREPEND, value: "/venv/bin:" } } })
    syncEnvCollection("c", { persistent: false, vars: { PATH: { type: APPEND, value: ":/extra" } } })
    const env = applyEnvCollections({ FOO: "old", PATH: "/usr/bin" })
    expect(env.FOO).toBe("x")
    // b prepends then c appends, in insertion order.
    expect(env.PATH).toBe("/venv/bin:/usr/bin:/extra")
  })

  it("does not mutate the base env object", () => {
    const base = { PATH: "/usr/bin" }
    syncEnvCollection("a", { persistent: false, vars: { PATH: { type: APPEND, value: ":/x" } } })
    applyEnvCollections(base)
    expect(base.PATH).toBe("/usr/bin")
  })

  it("a re-sync replaces the prior snapshot for that extension", () => {
    syncEnvCollection("a", { persistent: false, vars: { FOO: { type: REPLACE, value: "1" } } })
    syncEnvCollection("a", { persistent: false, vars: { BAR: { type: REPLACE, value: "2" } } })
    const env = applyEnvCollections({})
    expect(env.FOO).toBeUndefined()
    expect(env.BAR).toBe("2")
  })

  it("an empty snapshot (clear) removes the extension's contribution", () => {
    syncEnvCollection("a", { persistent: false, vars: { FOO: { type: REPLACE, value: "1" } } })
    syncEnvCollection("a", { persistent: false, vars: {} })
    expect(applyEnvCollections({}).FOO).toBeUndefined()
  })

  it("drop removes a non-persistent extension's contribution", () => {
    syncEnvCollection("a", { persistent: false, vars: { FOO: { type: REPLACE, value: "1" } } })
    dropEnvCollection("a")
    expect(applyEnvCollections({}).FOO).toBeUndefined()
  })
})

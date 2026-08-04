// #21 — WorkspaceSymbolRegistry resolves range-less symbols (vscode allows a
// provider to omit location.range and fill it in via resolveWorkspaceSymbol)
// rather than dropping them.
import { describe, expect, it } from "vitest"
import { URI } from "vscode-uri"
import { Range } from "./classes-core.js"
import { SymbolKind } from "./enums.js"
import { WorkspaceSymbolRegistry } from "./workspace-symbols.js"

describe("WorkspaceSymbolRegistry", () => {
  it("resolves a range-less symbol via resolveWorkspaceSymbol before mapping", async () => {
    const reg = new WorkspaceSymbolRegistry()
    const rangeless = { name: "Lazy", kind: SymbolKind.Function, containerName: "M", location: { uri: URI.file("/a.ts") } }
    reg.register({
      provideWorkspaceSymbols: () => [rangeless],
      resolveWorkspaceSymbol: (s) => ({ ...(s as object), location: { uri: URI.file("/a.ts"), range: new Range(3, 0, 3, 4) } }),
    })
    const items = await reg.provide("q")
    expect(items).toHaveLength(1)
    expect(items[0]).toMatchObject({ name: "Lazy", containerName: "M", kind: SymbolKind.Function })
    expect(items[0]?.range.startLine).toBe(3)
  })

  it("keeps a symbol that already has a range (no resolve call needed)", async () => {
    const reg = new WorkspaceSymbolRegistry()
    let resolved = false
    reg.register({
      provideWorkspaceSymbols: () => [{ name: "Eager", kind: SymbolKind.Class, location: { uri: URI.file("/b.ts"), range: new Range(1, 0, 1, 3) } }],
      resolveWorkspaceSymbol: (s) => { resolved = true; return s },
    })
    const items = await reg.provide("q")
    expect(items[0]?.range.startLine).toBe(1)
    expect(resolved).toBe(false)
  })

  it("drops a range-less symbol when the provider has no resolver", async () => {
    const reg = new WorkspaceSymbolRegistry()
    reg.register({ provideWorkspaceSymbols: () => [{ name: "X", kind: SymbolKind.Function, location: { uri: URI.file("/a.ts") } }] })
    expect(await reg.provide("q")).toHaveLength(0)
  })

  it("merges results across providers and passes a cancellation token", async () => {
    const reg = new WorkspaceSymbolRegistry()
    let gotToken: unknown
    reg.register({ provideWorkspaceSymbols: (_q, t) => { gotToken = t; return [{ name: "A", kind: SymbolKind.Function, location: { uri: URI.file("/a.ts"), range: new Range(0, 0, 0, 1) } }] } })
    reg.register({ provideWorkspaceSymbols: () => [{ name: "B", kind: SymbolKind.Function, location: { uri: URI.file("/b.ts"), range: new Range(0, 0, 0, 1) } }] })
    const items = await reg.provide("q")
    expect(items.map((i) => i.name)).toEqual(["A", "B"])
    expect(typeof gotToken).toBe("object")
  })
})

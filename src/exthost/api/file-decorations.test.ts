// FileDecorations host registry — first-provider-wins, TreeColor→id + badge
// clamping serialization, and the onDidChangeFileDecorations broadcast.
import { describe, expect, it, vi } from "vitest"
import { FileDecorations } from "./file-decorations.js"
import { EventEmitter } from "./shared.js"

describe("FileDecorations", () => {
  it("returns the first provider's decoration, serialized", async () => {
    const fd = new FileDecorations(vi.fn())
    fd.register({
      provideFileDecoration: (uri) =>
        (uri as { fsPath: string }).fsPath.endsWith(".py")
          ? { badge: "P", tooltip: "Python", color: { id: "gitDecoration.modifiedResourceForeground" }, propagate: true }
          : undefined,
    })
    const deco = await fd.provide("/x/a.py")
    expect(deco).toEqual({ badge: "P", tooltip: "Python", color: "gitDecoration.modifiedResourceForeground", propagate: true })
    expect(await fd.provide("/x/a.txt")).toBeNull()
  })

  it("clamps the badge to 2 chars and falls through to the next provider", async () => {
    const fd = new FileDecorations(vi.fn())
    fd.register({ provideFileDecoration: () => undefined })
    fd.register({ provideFileDecoration: () => ({ badge: "LONG" }) })
    const deco = await fd.provide("/x/a")
    expect(deco?.badge).toBe("LO")
  })

  it("broadcasts kamin:fileDecoration:changed with fs paths (null = all)", () => {
    const bc = vi.fn()
    const onChange = new EventEmitter<unknown>()
    const fd = new FileDecorations(bc)
    fd.register({ provideFileDecoration: () => undefined, onDidChangeFileDecorations: onChange.event })
    onChange.fire({ fsPath: "/x/a.py" })
    expect(bc).toHaveBeenCalledWith("kamin:fileDecoration:changed", { uris: ["/x/a.py"] })
    onChange.fire(undefined)
    expect(bc).toHaveBeenCalledWith("kamin:fileDecoration:changed", { uris: null })
  })

  it("refreshes (broadcasts changed) on both register and dispose", () => {
    const bc = vi.fn()
    const fd = new FileDecorations(bc)
    const reg = fd.register({ provideFileDecoration: () => undefined })
    expect(bc).toHaveBeenCalledWith("kamin:fileDecoration:changed", { uris: null })
    bc.mockClear()
    reg.dispose()
    expect(bc).toHaveBeenCalledWith("kamin:fileDecoration:changed", { uris: null })
  })

  it("dispose unregisters the provider", async () => {
    const fd = new FileDecorations(vi.fn())
    const reg = fd.register({ provideFileDecoration: () => ({ badge: "X" }) })
    expect((await fd.provide("/x/a"))?.badge).toBe("X")
    reg.dispose()
    expect(await fd.provide("/x/a")).toBeNull()
  })
})

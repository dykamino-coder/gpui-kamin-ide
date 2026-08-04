// B3 — the activation engine: who activates at startup vs. on a fired event.
import { describe, expect, it, vi } from "vitest"
import { ActivationManager, type ActivatableExtension } from "./activation-manager.js"

function ext(id: string, events: string[], spy = vi.fn().mockResolvedValue(undefined), deps: string[] = []): ActivatableExtension {
  return { id, events, deps, activate: spy }
}

describe("activateStartup (B3)", () => {
  it("activates `*` but NOT onStartupFinished or event-gated ones", async () => {
    const m = new ActivationManager()
    const star = vi.fn().mockResolvedValue(undefined)
    const finished = vi.fn().mockResolvedValue(undefined)
    const gated = vi.fn().mockResolvedValue(undefined)
    m.register(ext("a", ["*"], star))
    m.register(ext("b", ["onStartupFinished"], finished))
    m.register(ext("c", ["onCommand:foo"], gated))
    await m.activateStartup([])
    expect(star).toHaveBeenCalledOnce()
    expect(finished).not.toHaveBeenCalled() // deferred — loader fires it after
    expect(gated).not.toHaveBeenCalled()
    // onStartupFinished is a normal fired event, after the `*` pass.
    await m.fireEvent("onStartupFinished")
    expect(finished).toHaveBeenCalledOnce()
  })

  it("activates workspaceContains for both glob and bare-path forms", async () => {
    const m = new ActivationManager()
    const glob = vi.fn().mockResolvedValue(undefined)
    const bareDir = vi.fn().mockResolvedValue(undefined)
    const bareFile = vi.fn().mockResolvedValue(undefined)
    const miss = vi.fn().mockResolvedValue(undefined)
    m.register(ext("glob", ["workspaceContains:**/*.foo"], glob))
    m.register(ext("bareDir", ["workspaceContains:.vscode"], bareDir))
    m.register(ext("bareFile", ["workspaceContains:Cargo.toml"], bareFile))
    m.register(ext("miss", ["workspaceContains:**/*.bar"], miss))
    await m.activateStartup(["src/a.foo", ".vscode/settings.json", "Cargo.toml"])
    expect(glob).toHaveBeenCalledOnce()
    expect(bareDir).toHaveBeenCalledOnce() // matches .vscode/ (dir prefix)
    expect(bareFile).toHaveBeenCalledOnce() // exact path
    expect(miss).not.toHaveBeenCalled()
  })
})

describe("fireEvent (B3)", () => {
  it("activates the extension declaring the fired event, once", async () => {
    const m = new ActivationManager()
    const spy = vi.fn().mockResolvedValue(undefined)
    m.register(ext("c", ["onCommand:foo"], spy))
    await m.fireEvent("onCommand:bar") // no match
    expect(spy).not.toHaveBeenCalled()
    await m.fireEvent("onCommand:foo")
    await m.fireEvent("onCommand:foo") // already activated → removed from pending
    expect(spy).toHaveBeenCalledOnce()
  })

  it("does not activate a startup-gated extension on an unrelated event", async () => {
    const m = new ActivationManager()
    const spy = vi.fn().mockResolvedValue(undefined)
    m.register(ext("a", ["*"], spy))
    await m.fireEvent("onLanguage:js")
    expect(spy).not.toHaveBeenCalled()
  })
})

describe("extensionDependencies ordering (B-perf)", () => {
  it("activates dependencies before the dependent", async () => {
    const m = new ActivationManager()
    const order: string[] = []
    const dep = vi.fn().mockImplementation(() => { order.push("dep"); return Promise.resolve() })
    const main = vi.fn().mockImplementation(() => { order.push("main"); return Promise.resolve() })
    m.register(ext("pub.dep", ["onCommand:x"], dep)) // dep is event-gated, not startup
    m.register(ext("pub.main", ["*"], main, ["pub.dep"]))
    await m.activateStartup([])
    expect(order).toEqual(["dep", "main"]) // dep force-activated first
    expect(dep).toHaveBeenCalledOnce()
  })

  it("survives a dependency cycle without infinite recursion", async () => {
    const m = new ActivationManager()
    const a = vi.fn().mockResolvedValue(undefined)
    const b = vi.fn().mockResolvedValue(undefined)
    m.register(ext("p.a", ["*"], a, ["p.b"]))
    m.register(ext("p.b", ["*"], b, ["p.a"]))
    await m.activateStartup([])
    expect(a).toHaveBeenCalledOnce()
    expect(b).toHaveBeenCalledOnce()
  })
})

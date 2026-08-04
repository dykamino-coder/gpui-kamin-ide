import { describe, it, expect, vi } from "vitest"
import { Registry } from "./registry.js"

describe("Registry", () => {
  it("registers a command and dispatches via executeCommand", async () => {
    const r = new Registry()
    const handler = vi.fn().mockReturnValue(42)
    r.registerCommand("a.b", handler)
    expect(r.hasCommand("a.b")).toBe(true)
    const result = await r.executeCommand("a.b", "x", 1)
    expect(handler).toHaveBeenCalledWith("x", 1)
    expect(result).toBe(42)
  })

  it("rejects duplicate command ids", () => {
    const r = new Registry()
    r.registerCommand("dup", () => undefined)
    expect(() => r.registerCommand("dup", () => undefined)).toThrowError(/already registered/)
  })

  it("throws when executing an unknown command", async () => {
    const r = new Registry()
    await expect(r.executeCommand("nope")).rejects.toThrow(/not found/)
  })

  it("missing-command resolver gets a chance to register, then retries (B3)", async () => {
    const r = new Registry()
    const handler = vi.fn().mockReturnValue("ok")
    // Simulate lazy activation: the resolver registers the command on demand.
    r.setMissingCommandResolver((id) => {
      r.registerCommand(id, handler)
      return Promise.resolve()
    })
    const result = await r.executeCommand("lazy.cmd", "arg")
    expect(handler).toHaveBeenCalledWith("arg")
    expect(result).toBe("ok")
  })

  it("still throws if the resolver does not register the command (B3)", async () => {
    const r = new Registry()
    r.setMissingCommandResolver(() => Promise.resolve())
    await expect(r.executeCommand("still.missing")).rejects.toThrow(/not found/)
  })

  it("dispose() removes the command from the registry", () => {
    const r = new Registry()
    const sub = r.registerCommand("x", () => undefined)
    expect(r.hasCommand("x")).toBe(true)
    sub.dispose()
    expect(r.hasCommand("x")).toBe(false)
  })

  it("notify fires after register, after dispose, after setCommandTitle", () => {
    const r = new Registry()
    const fn = vi.fn()
    r.onUpdate(fn)
    const sub = r.registerCommand("x", () => undefined)
    r.setCommandTitle("x", "New title")
    sub.dispose()
    expect(fn).toHaveBeenCalledTimes(3)
  })

  it("snapshot strips the handler closure", () => {
    const r = new Registry()
    r.registerCommand("z", () => undefined, { title: "Zee" })
    const snap = r.snapshot()
    const cmd = snap.commands[0]
    expect(cmd?.id).toBe("z")
    expect(cmd?.title).toBe("Zee")
    expect("handler" in (cmd ?? {})).toBe(false)
  })
})

describe("Registry — B4 contributions", () => {
  it("registers menu items keyed by menu id; snapshot groups them", () => {
    const r = new Registry()
    r.registerMenuItem("editor/context", { command: "a.do", when: "editorFocus" })
    r.registerMenuItem("editor/context", { command: "a.undo" })
    r.registerMenuItem("commandPalette", { command: "a.do", when: "false" })
    const snap = r.snapshot()
    expect(snap.menus["editor/context"]).toHaveLength(2)
    expect(snap.menus.commandPalette).toHaveLength(1)
  })

  it("registers keybindings and submenus into the snapshot", () => {
    const r = new Registry()
    r.registerKeybinding({ key: "ctrl+k", command: "a.do", when: "editorFocus" })
    r.registerSubmenu({ id: "a.sub", label: "More" })
    const snap = r.snapshot()
    expect(snap.keybindings[0]?.command).toBe("a.do")
    expect(snap.submenus[0]?.id).toBe("a.sub")
  })

  it("registers a contributed language and merges same-id contributions", () => {
    const r = new Registry()
    r.registerLanguage({ id: "vue", extensions: [".vue"], aliases: ["Vue"] })
    r.registerLanguage({ id: "vue", extensions: [".vuex"], filenames: ["vue.config.js"] })
    const langs = r.snapshot().languages
    expect(langs).toHaveLength(1)
    expect(langs[0]?.id).toBe("vue")
    expect(langs[0]?.extensions).toEqual([".vue", ".vuex"]) // unioned, deduped
    expect(langs[0]?.filenames).toEqual(["vue.config.js"])
    expect(langs[0]?.aliases).toEqual(["Vue"])
  })

  it("registers a contributed TextMate grammar into the snapshot", () => {
    const r = new Registry()
    const d = r.registerGrammar({ scopeName: "source.vue", language: "vue", path: "C:\\ext\\vue.tmLanguage.json" })
    const g = r.snapshot().grammars
    expect(g).toHaveLength(1)
    expect(g[0]).toMatchObject({ scopeName: "source.vue", language: "vue" })
    d.dispose()
    expect(r.snapshot().grammars).toHaveLength(0)
  })

  it("dispose() removes a contributed menu item", () => {
    const r = new Registry()
    const sub = r.registerMenuItem("view/title", { command: "a.do" })
    expect(r.snapshot().menus["view/title"]).toHaveLength(1)
    sub.dispose()
    expect(r.snapshot().menus["view/title"]).toHaveLength(0)
  })

  it("evaluateWhen resolves against live context keys", () => {
    const r = new Registry()
    expect(r.evaluateWhen("editorFocus")).toBe(false)
    r.setContext("editorFocus", true)
    expect(r.evaluateWhen("editorFocus")).toBe(true)
    expect(r.evaluateWhen(undefined)).toBe(true)
  })
})

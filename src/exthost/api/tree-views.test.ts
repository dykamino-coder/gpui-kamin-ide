// TreeViews host registry — handle-cache lazy children, TreeItem→DTO
// serialization, onDidChangeTreeData broadcast, and createTreeView events.
import { describe, expect, it, vi } from "vitest"
import { EventEmitter } from "./shared.js"
import { TreeViews, type ViewEmitters } from "./tree-views.js"

// A tiny 2-level provider: roots ["a","b"]; each root's children ["<x>.1"].
function provider(onChange: EventEmitter<unknown> = new EventEmitter<unknown>()) {
  return {
    onDidChangeTreeData: onChange.event,
    getChildren: (el?: unknown) => (el === undefined ? ["a", "b"] : [`${el as string}.1`]),
    getTreeItem: (el: unknown) => {
      const isRoot = !(el as string).includes(".")
      return {
        label: el,
        collapsibleState: isRoot ? 1 : 0,
        iconPath: { id: isRoot ? "folder" : "circle-filled" }, // ThemeIcon-like
        command: isRoot ? undefined : { command: "do.it", title: "Do", arguments: [el] },
        tooltip: `tip ${el as string}`,
      }
    },
  }
}

function emitters(): ViewEmitters {
  return { selection: new EventEmitter(), expand: new EventEmitter(), collapse: new EventEmitter(), visibility: new EventEmitter(), checkbox: new EventEmitter() }
}

/** Indexed access that narrows away `undefined` (noUncheckedIndexedAccess). */
function at<T>(arr: readonly T[], i: number): T {
  const v = arr[i]
  if (v === undefined) throw new Error(`index ${String(i)} out of range`)
  return v
}

describe("TreeViews", () => {
  it("serializes root TreeItems with handles + collapsibleState + codicon", async () => {
    const tv = new TreeViews(vi.fn())
    tv.register("v", provider(), null)
    const roots = await tv.getChildren("v")
    expect(roots.map((r) => r.label)).toEqual(["a", "b"])
    expect(at(roots,0).collapsibleState).toBe(1)
    expect(at(roots,0).codicon).toBe("folder")
    expect(typeof at(roots,0).handle).toBe("string")
    expect(at(roots,0).tooltip).toBe("tip a")
  })

  it("lazy-loads a node's children via its handle", async () => {
    const tv = new TreeViews(vi.fn())
    tv.register("v", provider(), null)
    const roots = await tv.getChildren("v")
    const kids = await tv.getChildren("v", at(roots,0).handle)
    expect(kids.map((k) => k.label)).toEqual(["a.1"])
    expect(at(kids,0).collapsibleState).toBe(0)
    expect(at(kids,0).codicon).toBe("circle-filled")
    expect(at(kids,0).command).toEqual({ command: "do.it", title: "Do", arguments: ["a.1"] })
  })

  it("treats a null handle as root (WS serializes the root undefined to null)", async () => {
    const tv = new TreeViews(vi.fn())
    tv.register("v", provider(), null)
    const roots = await tv.getChildren("v", null)
    expect(roots.map((r) => r.label)).toEqual(["a", "b"])
  })

  it("returns [] for an unknown view or stale handle", async () => {
    const tv = new TreeViews(vi.fn())
    tv.register("v", provider(), null)
    expect(await tv.getChildren("nope")).toEqual([])
    expect(await tv.getChildren("v", "bogus-handle")).toEqual([]) // element undefined → getChildren(undefined)=roots? no: handle given but missing
  })

  it("broadcasts kamin:tree:changed when the provider fires onDidChangeTreeData", () => {
    const bc = vi.fn()
    const onChange = new EventEmitter<unknown>()
    const tv = new TreeViews(bc)
    tv.register("v", provider(onChange), null)
    onChange.fire(undefined)
    expect(bc).toHaveBeenCalledWith("kamin:tree:changed", { viewId: "v" })
  })

  it("uses TreeItem.id as the handle when provided", async () => {
    const tv = new TreeViews(vi.fn())
    tv.register("v", {
      getChildren: () => ["x"],
      getTreeItem: () => ({ label: "X", id: "stable-id", collapsibleState: 0 }),
    }, null)
    const node = at(await tv.getChildren("v"), 0)
    expect(node.handle).toBe("stable-id")
  })

  it("createTreeView: reportSelection fires onDidChangeSelection with mapped elements", async () => {
    const tv = new TreeViews(vi.fn())
    const em = emitters()
    tv.register("v", provider(), em)
    const roots = await tv.getChildren("v")
    const seen: unknown[][] = []
    em.selection.event((e) => seen.push([...e.selection]))
    tv.reportSelection("v", [at(roots,1).handle])
    expect(seen).toEqual([["b"]])
  })

  it("setMeta merges + broadcasts; an undefined field clears it; getMeta reads back", () => {
    const bc = vi.fn()
    const tv = new TreeViews(bc)
    tv.register("v", provider(), null)
    tv.setMeta("v", { message: "hi", badge: { value: 3, tooltip: "3 items" } })
    expect(bc).toHaveBeenLastCalledWith("kamin:tree:meta", { viewId: "v", meta: { message: "hi", badge: { value: 3, tooltip: "3 items" } } })
    tv.setMeta("v", { message: undefined })
    expect(tv.getMeta("v")).toEqual({ badge: { value: 3, tooltip: "3 items" } })
  })

  it("reveal broadcasts the handle of a materialized element (identity match)", async () => {
    const bc = vi.fn()
    const tv = new TreeViews(bc)
    // Stable element references so reveal can identity-match a fetched node.
    const a = { id: "A" }, b = { id: "B" }
    tv.register("v", {
      getChildren: (el?: unknown) => (el === undefined ? [a, b] : []),
      getTreeItem: (el: unknown) => ({ label: (el as { id: string }).id, collapsibleState: 0 }),
    }, null)
    await tv.getChildren("v") // materialize roots into byHandle
    bc.mockClear()
    await tv.reveal("v", b, { select: true })
    expect(bc).toHaveBeenCalledTimes(1)
    expect(bc.mock.calls[0]?.[0]).toBe("kamin:tree:reveal")
    expect((bc.mock.calls[0]?.[1] as { select: boolean }).select).toBe(true)
  })

  it("reveal re-materializes roots, so it works before any explicit fetch", async () => {
    const bc = vi.fn()
    const root = { id: "R" }
    const tv = new TreeViews(bc)
    tv.register("v", {
      getChildren: (el?: unknown) => (el === undefined ? [root] : []),
      getTreeItem: () => ({ label: "R", collapsibleState: 0 }),
    }, null)
    await tv.reveal("v", root, { select: true }) // no prior getChildren
    expect(bc).toHaveBeenCalledWith("kamin:tree:reveal", expect.objectContaining({ viewId: "v", select: true }))
  })

  it("serializes TreeItem.checkboxState (enum number and {state} object)", async () => {
    const tv = new TreeViews(vi.fn())
    tv.register("v", {
      getChildren: () => ["a", "b"],
      getTreeItem: (el) => el === "a"
        ? ({ label: "a", collapsibleState: 0, checkboxState: 1 })
        : ({ label: "b", collapsibleState: 0, checkboxState: { state: 0, tooltip: "toggle b" } }),
    }, null)
    const [a, b] = await tv.getChildren("v")
    expect(a?.checkboxState).toBe(1)
    expect(b?.checkboxState).toBe(0)
    expect(b?.checkboxTooltip).toBe("toggle b")
  })

  it("reportCheckbox fires onDidChangeCheckboxState with [element, state]", async () => {
    const tv = new TreeViews(vi.fn())
    const em = emitters()
    tv.register("v", provider(), em)
    const roots = await tv.getChildren("v")
    const seen: [unknown, number][][] = []
    em.checkbox.event((e) => seen.push(e.items))
    tv.reportCheckbox("v", at(roots, 0).handle, 1)
    expect(seen).toEqual([[["a", 1]]])
  })

  it("reveal no-ops for an element that was never materialized", async () => {
    const bc = vi.fn()
    const tv = new TreeViews(bc)
    tv.register("v", provider(), null)
    await tv.reveal("v", { never: "fetched" })
    expect(bc).not.toHaveBeenCalled()
  })

  it("DnD: handleDrag stashes a DataTransfer that handleDrop reads (host-side)", async () => {
    const bc = vi.fn()
    const tv = new TreeViews(bc)
    let dropped: { target: unknown; value: unknown } | null = null
    interface DT { set: (m: string, i: unknown) => void; get: (m: string) => { value: unknown } | undefined }
    const dnd = {
      dropMimeTypes: ["x"], dragMimeTypes: ["x"],
      handleDrag: (source: readonly unknown[], dt: unknown) => { (dt as DT).set("x", { value: source }) },
      handleDrop: (target: unknown, dt: unknown) => { dropped = { target, value: (dt as DT).get("x")?.value } },
    }
    tv.register("v", provider(), emitters(), dnd)
    expect(tv.hasDnd("v")).toBe(true)
    expect(bc).toHaveBeenCalledWith("kamin:tree:dnd", { viewId: "v", enabled: true })
    const roots = await tv.getChildren("v")
    await tv.handleDrag("v", [at(roots, 0).handle]) // drag "a"
    await tv.handleDrop("v", at(roots, 1).handle)   // drop on "b"
    expect(dropped).toEqual({ target: "b", value: ["a"] })
  })

  it("deep reveal via getParent materializes the chain + returns the expandPath", async () => {
    const bc = vi.fn()
    const root = { id: "g" }, leaf = { id: "l", parent: "g" }
    const tv = new TreeViews(bc)
    tv.register("v", {
      getChildren: (el?: unknown) => (el === undefined ? [root] : el === root ? [leaf] : []),
      getTreeItem: (el: unknown) => ({ label: (el as { id: string }).id, collapsibleState: (el as { id: string }).id === "g" ? 1 : 0 }),
      getParent: (el: unknown) => ((el as { parent?: string }).parent ? root : undefined),
    }, null)
    // Reveal the leaf with NOTHING materialized — getParent drives the descent.
    await tv.reveal("v", leaf, { select: true })
    const call = bc.mock.calls.find((c) => c[0] === "kamin:tree:reveal")
    expect(call).toBeDefined()
    const payload = call?.[1] as { handle: string; expandPath: string[] }
    expect(payload.expandPath.length).toBe(1) // the root group's handle
    expect(typeof payload.handle).toBe("string")
  })
})

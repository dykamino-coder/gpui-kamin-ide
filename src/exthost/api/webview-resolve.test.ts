import { describe, expect, it, vi } from "vitest"
import { Webviews } from "./webview.js"

describe("INC-2026-0110 resolve acknowledgement", () => {
  it("rejects before registration, allowing the shell to retry later", () => {
    const views = new Webviews(vi.fn())
    expect(() => { views.resolveView("late") }).toThrow("Webview view provider not registered")
    const resolve = vi.fn()
    views.registerWebviewViewProvider("late", { resolveWebviewView: resolve })
    views.resolveView("late")
    views.resolveView("late")
    expect(resolve).toHaveBeenCalledTimes(1)
  })
})

describe("BR-05 messages before a view is resolved again", () => {
  const subscribe = (seen: unknown[]) => ({
    resolveWebviewView: (view: unknown) => {
      (view as { webview: { onDidReceiveMessage: (f: (m: unknown) => void) => void } }).webview.onDidReceiveMessage((m) => { seen.push(m) })
    },
  })

  it("hands messages that arrived before resolve to the view, in order", () => {
    // An extension-host respawn: the renderer's iframe keeps sending before the
    // new host has resolved the view (or even registered its provider).
    const views = new Webviews(vi.fn())
    views.deliverMessage("chat", { type: "closeTab", id: "a" })
    const seen: unknown[] = []
    views.registerWebviewViewProvider("chat", subscribe(seen))
    views.deliverMessage("chat", { type: "ping" })
    views.resolveView("chat")
    views.deliverMessage("chat", { type: "after" })
    expect(seen).toEqual([{ type: "closeTab", id: "a" }, { type: "ping" }, { type: "after" }])
  })

  it("keeps the backlog bounded and drops it when the renderer closes the view", () => {
    const views = new Webviews(vi.fn())
    views.deliverMessage("gone", "stale")
    views.disposeFromRenderer("gone")
    for (let i = 0; i < 300; i++) views.deliverMessage("chat", i)
    for (let i = 0; i < 40; i++) views.deliverMessage(`view-${String(i)}`, i)
    views.deliverMessage("webview-7", "dead panel")
    const seen: unknown[] = []
    const gone: unknown[] = []
    const late: unknown[] = []
    views.registerWebviewViewProvider("chat", subscribe(seen))
    views.registerWebviewViewProvider("gone", subscribe(gone))
    views.registerWebviewViewProvider("view-39", subscribe(late))
    views.resolveView("chat")
    views.resolveView("gone")
    views.resolveView("view-39")
    expect(seen).toHaveLength(256)
    expect(seen[0]).toBe(44)
    expect(seen[255]).toBe(299)
    expect(gone).toEqual([])
    expect(late).toEqual([])
  })
})
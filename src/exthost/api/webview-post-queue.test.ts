// Post relay: coalescing, frame cap, identity dedup, and ordering against every
// OTHER webview broadcast. Split from webview.test.ts (250-line ceiling).
import { describe, expect, it, vi } from "vitest"
import { Webviews } from "./webview.js"

const FIRST_PANEL_ID = "webview-1"

describe("Webviews — post relay", () => {
// Post ORDERING. Posts are coalesced onto a setImmediate frame while every
// other webview broadcast is synchronous — so without an explicit drain, the
// later call overtakes the earlier post deterministically (a microtask can
// never outrun setImmediate). These pin the extension's own ordering.
describe("post ordering vs other broadcasts", () => {
  /** Channels seen, in order, ignoring the create//html noise of setup. */
  const channelsAfter = (bc: ReturnType<typeof vi.fn>, from: number): string[] =>
    bc.mock.calls.slice(from).map((c) => c[0] as string)

  it("delivers a queued post BEFORE the dispose that follows it", () => {
    const bc = vi.fn()
    const panel = new Webviews(bc).createPanel("v", "t", 1, {})
    const mark = bc.mock.calls.length
    void panel.webview.postMessage({ final: "state" })
    panel.dispose()
    expect(channelsAfter(bc, mark)).toEqual(["kamin:webview:post", "kamin:webview:dispose"])
  })

  it("delivers a queued post BEFORE a following html/title/reveal", () => {
    const bc = vi.fn()
    const panel = new Webviews(bc).createPanel("v", "t", 1, {})
    const mark = bc.mock.calls.length
    void panel.webview.postMessage({ a: 1 })
    panel.webview.html = "<p>next</p>"
    void panel.webview.postMessage({ b: 2 })
    panel.title = "Renamed"
    expect(channelsAfter(bc, mark)).toEqual([
      "kamin:webview:post", "kamin:webview:html",
      "kamin:webview:post", "kamin:webview:title",
    ])
  })

  it("postMessage resolves only once the frame carrying it is broadcast", async () => {
    const bc = vi.fn()
    const panel = new Webviews(bc).createPanel("v", "t", 1, {})
    let settled = false
    const p = panel.webview.postMessage({ x: 1 }).then((ok) => { settled = true; return ok })
    await Promise.resolve() // drain microtasks: a premature resolve lands here
    expect(settled).toBe(false)
    expect(bc).not.toHaveBeenCalledWith("kamin:webview:post", expect.anything())
    await expect(p).resolves.toBe(true) // the setImmediate frame settles it
    expect(bc).toHaveBeenCalledWith("kamin:webview:post", expect.anything())
  })

  it("resolves false and sends nothing for posts still queued when the RENDERER closes the tab", async () => {
    const bc = vi.fn()
    const wv = new Webviews(bc)
    const panel = wv.createPanel("v", "t", 1, {})
    const mark = bc.mock.calls.length
    const p = panel.webview.postMessage({ lost: true })
    wv.disposeFromRenderer(FIRST_PANEL_ID) // iframe already gone
    await expect(p).resolves.toBe(false)
    expect(channelsAfter(bc, mark)).toEqual([]) // never broadcast into a dead id
  })

  it("keeps posts for OTHER panels when one is closed by the renderer", async () => {
    const bc = vi.fn()
    const wv = new Webviews(bc)
    const doomed = wv.createPanel("v", "a", 1, {})
    const alive = wv.createPanel("v", "b", 1, {})
    const dead = doomed.webview.postMessage({ n: 1 })
    const kept = alive.webview.postMessage({ n: 2 })
    wv.disposeFromRenderer(FIRST_PANEL_ID)
    await expect(dead).resolves.toBe(false)
    await expect(kept).resolves.toBe(true)
    expect(bc).toHaveBeenCalledWith("kamin:webview:post", { batch: [{ ids: ["webview-2"], msg: { n: 2 } }] })
  })

  it("carries a fanned-out payload ONCE, with an id list", async () => {
    // BridgeHost.post() hands the SAME object to every attached webview. The
    // host→renderer hop is JSON.stringify, which has no reference dedup — so
    // without grouping, one payload crossed the wire N times.
    const bc = vi.fn()
    const wv = new Webviews(bc)
    const a = wv.createPanel("v", "a", 1, {})
    const b = wv.createPanel("v", "b", 1, {})
    const mark = bc.mock.calls.length
    const shared = { big: "payload" } // one object, two webviews
    const pa = a.webview.postMessage(shared)
    const pb = b.webview.postMessage(shared)
    await Promise.all([pa, pb])
    const frames = bc.mock.calls.slice(mark).filter((c) => c[0] === "kamin:webview:post")
    expect(frames).toHaveLength(1)
    const batch = (frames[0]?.[1] as { batch: { ids: string[]; msg: unknown }[] }).batch
    expect(batch).toHaveLength(1) // ONE entry, not two
    expect(batch.map((g) => g.ids)).toEqual([["webview-1", "webview-2"]])
    expect(batch.map((g) => g.msg)).toEqual([shared]) // same ref — serialized once
  })

  it("keeps DISTINCT payloads separate", async () => {
    const bc = vi.fn()
    const wv = new Webviews(bc)
    const a = wv.createPanel("v", "a", 1, {})
    const b = wv.createPanel("v", "b", 1, {})
    const mark = bc.mock.calls.length
    await Promise.all([a.webview.postMessage({ n: 1 }), b.webview.postMessage({ n: 2 })])
    const frames = bc.mock.calls.slice(mark).filter((c) => c[0] === "kamin:webview:post")
    const batch = (frames[0]?.[1] as { batch: unknown[] }).batch
    expect(batch).toHaveLength(2) // different objects → not grouped
  })

  it("keeps the frame cap when draining synchronously ahead of a dispose", () => {
    const bc = vi.fn()
    const panel = new Webviews(bc).createPanel("v", "t", 1, {})
    const mark = bc.mock.calls.length
    for (let i = 0; i < 20; i++) void panel.webview.postMessage({ i })
    panel.dispose() // forces the drain — must not merge into one huge frame
    const frames = bc.mock.calls.slice(mark).filter((c) => c[0] === "kamin:webview:post")
    expect(frames).toHaveLength(3) // 8 + 8 + 4, cap intact
    expect(frames.map((f) => (f[1] as { batch: unknown[] }).batch.length)).toEqual([8, 8, 4])
    expect(channelsAfter(bc, mark).at(-1)).toBe("kamin:webview:dispose")
  })

  // Size cap: a DLP heap hook only wedges LARGE allocations, so a frame must
  // stay small even when the 8-post COUNT cap is nowhere near reached.
  it("splits a batch by serialised size, not just post count", () => {
    const bc = vi.fn()
    const panel = new Webviews(bc).createPanel("v", "t", 1, {})
    const mark = bc.mock.calls.length
    const big = "x".repeat(5000) // ~5KB each — 3 of them = ~15KB > the 12KB cap
    for (let i = 0; i < 3; i++) void panel.webview.postMessage({ big, i })
    panel.dispose() // drain synchronously
    const frames = bc.mock.calls.slice(mark).filter((c) => c[0] === "kamin:webview:post")
    // Would have been ONE 15KB frame under the count-only cap; size cap splits it.
    expect(frames.length).toBeGreaterThan(1)
    for (const f of frames) {
      expect(JSON.stringify((f[1] as { batch: unknown[] }).batch).length).toBeLessThan(13_000)
    }
  })

  it("still sends a single over-cap post as its own frame (cannot split one msg)", () => {
    const bc = vi.fn()
    const panel = new Webviews(bc).createPanel("v", "t", 1, {})
    const mark = bc.mock.calls.length
    void panel.webview.postMessage({ huge: "y".repeat(20_000) }) // > cap, alone
    panel.dispose()
    const frames = bc.mock.calls.slice(mark).filter((c) => c[0] === "kamin:webview:post")
    expect(frames).toHaveLength(1)
    expect((frames[0]?.[1] as { batch: unknown[] }).batch).toHaveLength(1)
  })
})
})

// B7a — webview registry: panel creation broadcasts, message routing both
// directions, view-state, and dispose (extension- and renderer-initiated).
import { describe, expect, it, vi } from "vitest"
import { Uri } from "./shared.js"
import { Webviews, type PersistedPanel } from "./webview.js"

/** In-memory WebviewStore for the cross-restart tests. */
function fakeStore(initial: PersistedPanel[] = []) {
  let panels = [...initial]
  return { load: () => panels, save: (p: PersistedPanel[]) => { panels = p }, current: () => panels }
}

// Each test builds a fresh Webviews (seq starts at 0), so its first panel id
// is deterministic. The id isn't exposed on the public panel object.
const FIRST_PANEL_ID = "webview-1"

describe("Webviews", () => {
  it("createPanel announces the panel and exposes a live webview", () => {
    const bc = vi.fn()
    const panel = new Webviews(bc).createPanel("md.preview", "Preview", 1, { enableScripts: true })
    expect(panel.viewType).toBe("md.preview")
    expect(panel.title).toBe("Preview")
    expect(bc).toHaveBeenCalledWith("kamin:webview:create", expect.objectContaining({ viewType: "md.preview", title: "Preview" }))
  })

  it("stores the requested view column and reveal() updates it", () => {
    const bc = vi.fn()
    const panel = new Webviews(bc).createPanel("v", "t", { viewColumn: 2 }, {})
    expect(panel.viewColumn).toBe(2)
    panel.reveal(3, true)
    expect(panel.viewColumn).toBe(3)
    expect(bc).toHaveBeenCalledWith("kamin:webview:reveal", expect.objectContaining({ viewColumn: 3, preserveFocus: true }))
  })

  it("splits create options into webview vs panel slices", () => {
    const panel = new Webviews(vi.fn()).createPanel("v", "t", 1, { enableScripts: true, retainContextWhenHidden: true })
    expect(panel.webview.options.enableScripts).toBe(true)
    expect(panel.options.retainContextWhenHidden).toBe(true)
    // slices are distinct: panel-only field absent from the webview slice
    expect((panel.webview.options as Record<string, unknown>).retainContextWhenHidden).toBeUndefined()
  })

  it("postMessage resolves false when the panel is hidden without retainContextWhenHidden", async () => {
    const wv = new Webviews(vi.fn())
    const panel = wv.createPanel("v", "t", 1, {})
    wv.updateViewState(FIRST_PANEL_ID, false, false)
    expect(await panel.webview.postMessage({ x: 1 })).toBe(false)
  })

  it("html setter and postMessage broadcast to the renderer", async () => {
    const bc = vi.fn()
    const panel = new Webviews(bc).createPanel("v", "t", 1, {})
    panel.webview.html = "<h1>hi</h1>"
    expect(panel.webview.html).toBe("<h1>hi</h1>")
    expect(bc).toHaveBeenCalledWith("kamin:webview:html", expect.objectContaining({ html: "<h1>hi</h1>" }))
    void panel.webview.postMessage({ ping: 1 })
    // posts are coalesced and flushed as ONE `{ batch }` frame on setImmediate.
    await new Promise((r) => setImmediate(r))
    expect(bc).toHaveBeenCalledWith("kamin:webview:post", { batch: [expect.objectContaining({ msg: { ping: 1 } })] })
  })

  it("delivers renderer messages to onDidReceiveMessage", () => {
    const wv = new Webviews(vi.fn())
    const panel = wv.createPanel("v", "t", 1, {})
    const id = FIRST_PANEL_ID
    const seen: unknown[] = []
    panel.webview.onDidReceiveMessage((m) => seen.push(m))
    wv.deliverMessage(id, { hello: true })
    expect(seen).toEqual([{ hello: true }])
  })

  it("title setter broadcasts the new title", () => {
    const bc = vi.fn()
    const panel = new Webviews(bc).createPanel("v", "old", 1, {})
    panel.title = "new"
    expect(panel.title).toBe("new")
    expect(bc).toHaveBeenCalledWith("kamin:webview:title", expect.objectContaining({ title: "new" }))
  })

  it("extension dispose() broadcasts dispose and fires onDidDispose once", () => {
    const bc = vi.fn()
    const panel = new Webviews(bc).createPanel("v", "t", 1, {})
    let fired = 0
    panel.onDidDispose(() => { fired++ })
    panel.dispose()
    panel.dispose() // idempotent
    expect(fired).toBe(1)
    expect(bc).toHaveBeenCalledWith("kamin:webview:dispose", expect.anything())
  })

  it("renderer-initiated close fires onDidDispose without re-broadcasting dispose", () => {
    const bc = vi.fn()
    const wv = new Webviews(bc)
    const panel = wv.createPanel("v", "t", 1, {})
    const id = FIRST_PANEL_ID
    let fired = 0
    panel.onDidDispose(() => { fired++ })
    bc.mockClear()
    wv.disposeFromRenderer(id)
    expect(fired).toBe(1)
    expect(bc).not.toHaveBeenCalledWith("kamin:webview:dispose", expect.anything())
  })

  it("updateViewState fires onDidChangeViewState only on change", () => {
    const wv = new Webviews(vi.fn())
    const panel = wv.createPanel("v", "t", 1, {})
    const id = FIRST_PANEL_ID
    let fired = 0
    panel.onDidChangeViewState(() => { fired++ })
    wv.updateViewState(id, true, true) // no change from defaults
    wv.updateViewState(id, false, false) // change
    expect(fired).toBe(1)
    expect(panel.active).toBe(false)
    expect(panel.visible).toBe(false)
  })

  // ── B7c-2: resources (asWebviewUri / cspSource / localResourceRoots) ──
  it("asWebviewUri maps a file uri to the same-origin resource route", () => {
    const local = Uri.file("/ext/media/x.png")
    const panel = new Webviews(vi.fn()).createPanel("v", "t", 1, {})
    const out = panel.webview.asWebviewUri(local)
    // http://kaminwebview.localhost/__resource/<id><local.path>
    expect(out.scheme).toBe("http")
    expect(out.authority).toBe("kaminwebview.localhost")
    expect(out.path).toBe(`/__resource/${FIRST_PANEL_ID}${local.path}`)
  })

  it("cspSource is the webview serving origin", () => {
    const panel = new Webviews(vi.fn()).createPanel("v", "t", 1, {})
    expect(panel.webview.cspSource).toBe("http://kaminwebview.localhost")
  })

  it("defaults localResourceRoots to the extension dir + workspace folder", () => {
    const bc = vi.fn()
    // Compare on fsPath so the assertion is platform-correct (Windows separators).
    const extFs = Uri.file("/ext/root").fsPath
    const wsFs = Uri.file("/work/proj").fsPath
    const wv = new Webviews(bc, { getFolderPath: () => "/work/proj" })
    const panel = wv.createPanel("v", "t", 1, {}, "/ext/root")
    const roots = (panel.webview.options.localResourceRoots ?? []).map((u) => u.fsPath)
    expect(roots).toContain(extFs)
    expect(roots).toContain(wsFs)
    // create broadcast carries the resolved fs paths for the Rust handler
    const calls = bc.mock.calls as [string, { localResourceRoots?: string[] }][]
    const created = calls.find((c) => c[0] === "kamin:webview:create")?.[1]
    expect(created?.localResourceRoots).toContain(extFs)
    expect(created?.localResourceRoots).toContain(wsFs)
  })

  it("honors explicitly provided localResourceRoots over the default", () => {
    const onlyFs = Uri.file("/only/here").fsPath
    const wv = new Webviews(vi.fn(), { getFolderPath: () => "/work/proj" })
    const panel = wv.createPanel("v", "t", 1, { localResourceRoots: [Uri.file("/only/here")] }, "/ext/root")
    const roots = (panel.webview.options.localResourceRoots ?? []).map((u) => u.fsPath)
    expect(roots).toEqual([onlyFs])
  })

  // ── WebviewPanelSerializer: cross-restart persistence ──
  it("persists an open panel's {viewType,title,state} to the store", () => {
    const store = fakeStore()
    const wv = new Webviews(vi.fn(), undefined, store)
    const panel = wv.createPanel("md.preview", "Doc", 1, {})
    expect(store.current()).toEqual([{ viewType: "md.preview", title: "Doc", state: undefined }])
    panel.title = "Doc2"
    expect(store.current()[0]?.title).toBe("Doc2")
    wv.persistPanelState(FIRST_PANEL_ID, { scroll: 5 })
    expect(store.current()[0]?.state).toEqual({ scroll: 5 })
    panel.dispose()
    expect(store.current()).toEqual([])
  })

  it("restore re-creates saved panels that have a serializer + deserializes them", () => {
    const bc = vi.fn()
    const store = fakeStore([{ viewType: "md.preview", title: "Doc", state: { n: 1 } }])
    const wv = new Webviews(bc, undefined, store)
    const seen: unknown[] = []
    wv.registerSerializer("md.preview", { deserializeWebviewPanel: (_p, state) => { seen.push(state) } })
    wv.restore()
    // create broadcast carries the restored state as initialState
    expect(bc).toHaveBeenCalledWith("kamin:webview:create", expect.objectContaining({ viewType: "md.preview", title: "Doc", initialState: { n: 1 } }))
    expect(seen).toEqual([{ n: 1 }])
  })

  it("restore skips saved panels without a registered serializer, and runs once", () => {
    const bc = vi.fn()
    const store = fakeStore([{ viewType: "ghost", title: "G", state: undefined }])
    const wv = new Webviews(bc, undefined, store)
    wv.restore()
    expect(bc).not.toHaveBeenCalledWith("kamin:webview:create", expect.anything())
    wv.registerSerializer("ghost", { deserializeWebviewPanel: () => { /* noop */ } })
    wv.restore() // already ran — no second pass
    expect(bc).not.toHaveBeenCalledWith("kamin:webview:create", expect.anything())
  })
})

// Regression guard for the P0 "silent-empty" API stubs that used to be declared
// but return a hardcoded empty value (the same shape that hid the shipped
// showSaveDialog bug). Each test proves the namespace now does REAL work:
// getLanguages reads the registry, showWorkspaceFolderPick reflects the open
// folder, applyEdit forwards edits to the editor host, and env.openExternal /
// clipboard.readText reach their hooks instead of resolving ""/true.
import { describe, expect, it, vi } from "vitest"
import { Range, WorkspaceEdit } from "./classes-core.js"
import type { NsHooks } from "./ns-builders.js"
import { buildWindow } from "./ns-builders.js"
import { buildEnv, buildLanguages } from "./ns-data.js"
import { buildDocuments } from "./ns-documents.js"
import { makeInputBox, makeQuickPick } from "./ns-quick-input.js"
import { buildWorkspace } from "./ns-workspace.js"

// buildWorkspace + buildDocuments subscribe to several host planes at
// construction (documents, folder, config). A complete no-op workspaceHost
// satisfies them; each test overrides only editors/fs — what applyEdit uses.
/** Explicit no-ops. These stubs exist to satisfy construction — nothing in
 *  these tests calls them, and a bare `() => {}` reads as "forgot to implement". */
const noop = (): void => { /* stub — never invoked by these tests */ }
const noopSub = (): (() => void) => noop // subscribe → dispose
const noopDocs = { list: () => [], get: () => undefined, open: noop, onDidOpen: noopSub, onDidChange: noopSub, onDidClose: noopSub, onDidSave: noopSub }
function wsHost(over: Record<string, unknown>): Record<string, unknown> {
  return {
    getFolderPath: () => null, onDidChangeFolder: noopSub,
    listFiles: () => Promise.resolve([]), watchFiles: noopSub,
    documents: noopDocs, fs: {},
    config: { registerDefaults: noop, getAll: () => ({}), inspect: () => ({}), update: () => Promise.resolve(), onDidChange: noopSub },
    ...over,
  }
}

describe("getLanguages (was []-despite-registry)", () => {
  it("returns the base set unioned with contributed language ids, deduped", async () => {
    const hooks = {
      registry: { snapshot: () => ({ languages: [{ id: "rust" }, { id: "go" }, { id: "json" }] }) },
      // buildLanguages wires diagnostics at construction — a no-op surface is enough.
      diagnostics: { getDiagnostics: () => [], onDidChangeDiagnostics: noopSub },
      languageFeatures: {},
    } as unknown as NsHooks
    const ids = await buildLanguages(hooks, {} as never).getLanguages()
    expect(ids).toContain("rust")
    expect(ids).toContain("go")
    expect(ids).toContain("plaintext") // base set
    expect(ids.filter((x) => x === "json")).toHaveLength(1) // dedup vs base
  })
})

describe("window.showWorkspaceFolderPick (was undefined-when-open)", () => {
  const win = (folderPath: string | null) =>
    buildWindow({ workspaceHost: { getFolderPath: () => folderPath } } as unknown as NsHooks, "ext", {} as never)

  it("resolves the open folder as a WorkspaceFolder", async () => {
    const picked = await win("C:/proj/app").showWorkspaceFolderPick()
    expect(picked).toMatchObject({ name: "app", index: 0 })
    expect((picked as { uri: { fsPath: string } }).uri.fsPath.replace(/\\/g, "/")).toContain("proj/app")
  })
  it("resolves undefined when no folder is open", async () => {
    expect(await win(null).showWorkspaceFolderPick()).toBeUndefined()
  })
})

describe("workspace.applyEdit (was silent no-op)", () => {
  it("forwards each replace to editors.applyEdits with a converted range", async () => {
    const applyEdits = vi.fn().mockResolvedValue(true)
    const showDocument = vi.fn().mockResolvedValue(undefined)
    const hooks = { workspaceHost: wsHost({ editors: { applyEdits, showDocument } }) } as unknown as NsHooks
    const ws = buildWorkspace(hooks, buildDocuments(hooks))

    const edit = new WorkspaceEdit()
    const uri = { fsPath: "C:/proj/a.ts", toString: () => "file:///proj/a.ts" }
    edit.replace(uri, new Range(1, 2, 3, 4), "renamed")

    const ok = await ws.applyEdit(edit)
    expect(ok).toBe(true)
    expect(showDocument).toHaveBeenCalledOnce()
    expect(applyEdits).toHaveBeenCalledWith(
      expect.any(String),
      [{ range: { startLine: 1, startChar: 2, endLine: 3, endChar: 4 }, text: "renamed" }],
    )
  })

  it("resolves false when a sub-edit fails", async () => {
    const hooks = { workspaceHost: wsHost({ editors: { applyEdits: () => Promise.resolve(false), showDocument: () => Promise.resolve() } }) } as unknown as NsHooks
    const ws = buildWorkspace(hooks, buildDocuments(hooks))
    const edit = new WorkspaceEdit()
    edit.replace({ fsPath: "C:/proj/a.ts", toString: () => "file:///proj/a.ts" }, new Range(0, 0, 0, 1), "x")
    expect(await ws.applyEdit(edit)).toBe(false)
  })
})

describe("env silent stubs (was ''/true)", () => {
  it("clipboard.readText reaches the readClipboard hook", async () => {
    const readClipboard = vi.fn().mockResolvedValue("from-os")
    const env = buildEnv({ readClipboard } as unknown as NsHooks)
    expect(await env.clipboard.readText()).toBe("from-os")
    expect(readClipboard).toHaveBeenCalledOnce()
  })

  it("openExternal passes the uri's external string to the hook", async () => {
    const openExternal = vi.fn().mockResolvedValue(true)
    const env = buildEnv({ openExternal } as unknown as NsHooks)
    const ok = await env.openExternal({ toString: () => "https://example.com/auth" })
    expect(ok).toBe(true)
    expect(openExternal).toHaveBeenCalledWith("https://example.com/auth")
  })
})

describe("TextDocument.save (was a no-op that returned true, wrote nothing)", () => {
  it("writes the document's current mirror content to disk", async () => {
    const writeFile = vi.fn().mockResolvedValue(undefined)
    const doc = { uri: "C:/proj/f.ts", content: "export const x = 1\n", version: 3, languageId: "typescript" }
    const hooks = {
      workspaceHost: {
        documents: { ...noopDocs, get: () => doc },
        fs: { writeFile },
      },
    } as unknown as NsHooks
    const docs = buildDocuments(hooks)
    const td = await docs.openTextDocument(doc.uri) as { save: () => Promise<boolean> }

    const ok = await td.save()
    expect(ok).toBe(true)
    const [path, bytes] = (writeFile.mock.calls[0] ?? []) as [string, Uint8Array]
    expect(path.replace(/\\/g, "/")).toContain("proj/f.ts")
    expect(new TextDecoder().decode(bytes)).toBe(doc.content)
  })
})

describe("createInputBox / createQuickPick (were inert — onDidAccept never fired → HANG)", () => {
  it("inputBox.show() resolves the value and fires onDidAccept", async () => {
    const showInputBox = vi.fn().mockResolvedValue("typed-value")
    const box = makeInputBox({ showInputBox } as unknown as NsHooks) as {
      value: string; prompt: string; show: () => void; onDidAccept: (fn: () => void) => void
    }
    box.prompt = "Name?"
    const accepted = new Promise<void>((res) => { box.onDidAccept(() => { res(); }); })
    box.show()
    await accepted
    expect(showInputBox).toHaveBeenCalledWith(expect.objectContaining({ prompt: "Name?" }))
    expect(box.value).toBe("typed-value")
  })

  it("inputBox dismissal fires onDidHide, not onDidAccept", async () => {
    const box = makeInputBox({ showInputBox: () => Promise.resolve(undefined) } as unknown as NsHooks) as {
      show: () => void; onDidHide: (fn: () => void) => void; onDidAccept: (fn: () => void) => void
    }
    let accepted = false
    box.onDidAccept(() => { accepted = true })
    const hidden = new Promise<void>((res) => { box.onDidHide(() => { res(); }); })
    box.show()
    await hidden
    expect(accepted).toBe(false)
  })

  it("quickPick.show() fires onDidChangeSelection + onDidAccept with the chosen item", async () => {
    const showQuickPick = vi.fn().mockResolvedValue([1])
    const qp = makeQuickPick({ showQuickPick } as unknown as NsHooks) as {
      items: { label: string }[]; selectedItems: { label: string }[]
      show: () => void; onDidAccept: (fn: () => void) => void
      onDidChangeSelection: (fn: (items: { label: string }[]) => void) => void
    }
    qp.items = [{ label: "A" }, { label: "B" }]
    let selected: { label: string }[] = []
    qp.onDidChangeSelection((items) => { selected = items })
    const accepted = new Promise<void>((res) => { qp.onDidAccept(() => { res(); }); })
    qp.show()
    await accepted
    expect(selected).toEqual([{ label: "B" }])
    expect(qp.selectedItems).toEqual([{ label: "B" }])
  })
})

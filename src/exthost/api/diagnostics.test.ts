// B6c — diagnostic registry: collection mutations broadcast to the renderer,
// getDiagnostics aggregates across collections, onDidChangeDiagnostics fires.
import { describe, expect, it, vi } from "vitest"
import { URI } from "vscode-uri"
import { Range } from "./classes-core.js"
import { Diagnostics } from "./diagnostics.js"

const uri = (p: string) => URI.file(p)
const diag = (line: number, msg: string, severity = 0) => ({ range: new Range(line, 0, line, 4), message: msg, severity })

describe("Diagnostics", () => {
  it("set() stores diagnostics and broadcasts normalized DTOs", () => {
    const bc = vi.fn()
    const reg = new Diagnostics(bc)
    const col = reg.createCollection("eslint")
    col.set(uri("/a.ts"), [diag(2, "no-unused", 1)])

    expect(col.has(uri("/a.ts"))).toBe(true)
    expect(col.get(uri("/a.ts"))).toHaveLength(1)
    expect(bc).toHaveBeenCalledWith("kamin:diag:set", expect.objectContaining({
      uri: uri("/a.ts").fsPath,
      diagnostics: [expect.objectContaining({ message: "no-unused", severity: 1, range: { startLine: 2, startChar: 0, endLine: 2, endChar: 4 } })],
    }))
  })

  it("fires onDidChangeDiagnostics with the changed uris", () => {
    const reg = new Diagnostics(vi.fn())
    const col = reg.createCollection("c")
    const seen: string[] = []
    reg.onDidChangeDiagnostics((e) => { for (const u of e.uris) seen.push(u.fsPath ?? u.toString()) })
    col.set(uri("/x.ts"), [diag(0, "m")])
    expect(seen).toEqual([uri("/x.ts").fsPath])
  })

  it("getDiagnostics aggregates across collections by uri", () => {
    const reg = new Diagnostics(vi.fn())
    const a = reg.createCollection("a")
    const b = reg.createCollection("b")
    a.set(uri("/f.ts"), [diag(0, "from-a")])
    b.set(uri("/f.ts"), [diag(1, "from-b")])
    expect(reg.getDiagnostics(uri("/f.ts"))).toHaveLength(2)
    const all = reg.getDiagnostics()
    expect(all).toHaveLength(1)
    expect(all[0]?.[1]).toHaveLength(2)
  })

  it("delete() and clear() remove entries and emit empty markers", () => {
    const bc = vi.fn()
    const reg = new Diagnostics(bc)
    const col = reg.createCollection("c")
    col.set(uri("/a.ts"), [diag(0, "m")])
    col.delete(uri("/a.ts"))
    expect(col.has(uri("/a.ts"))).toBe(false)
    expect(bc).toHaveBeenLastCalledWith("kamin:diag:set", expect.objectContaining({ uri: uri("/a.ts").fsPath, diagnostics: [] }))

    col.set(uri("/b.ts"), [diag(0, "m")])
    col.clear()
    expect(reg.getDiagnostics()).toHaveLength(0)
  })

  it("dispose() drops the collection from the aggregate and clears markers", () => {
    const bc = vi.fn()
    const reg = new Diagnostics(bc)
    const col = reg.createCollection("c")
    col.set(uri("/a.ts"), [diag(0, "m")])
    col.dispose()
    expect(reg.getDiagnostics()).toHaveLength(0)
    expect(bc).toHaveBeenLastCalledWith("kamin:diag:set", expect.objectContaining({ diagnostics: [] }))
  })

  it("set([entries]) merges tuples of the same uri within the batch", () => {
    const reg = new Diagnostics(vi.fn())
    const col = reg.createCollection("c")
    col.set([[uri("/f.ts"), [diag(0, "d1")]], [uri("/f.ts"), [diag(1, "d2")]]])
    expect(col.get(uri("/f.ts"))).toHaveLength(2)
  })

  it("set([entries]) with an undefined tuple clears prior-in-batch for that uri", () => {
    const reg = new Diagnostics(vi.fn())
    const col = reg.createCollection("c")
    col.set([[uri("/f.ts"), [diag(0, "d1")]], [uri("/f.ts"), undefined], [uri("/f.ts"), [diag(2, "d3")]]])
    const diags = col.get(uri("/f.ts"))
    expect(diags).toHaveLength(1)
  })

  it("is iterable as [uri, diagnostics] tuples", () => {
    const reg = new Diagnostics(vi.fn())
    const col = reg.createCollection("c")
    col.set(uri("/a.ts"), [diag(0, "m")])
    const entries = [...col]
    expect(entries).toHaveLength(1)
    expect(entries[0]?.[0].fsPath).toBe(uri("/a.ts").fsPath)
    expect(entries[0]?.[1]).toHaveLength(1)
  })

  it("snapshotDtos returns every (owner, uri) pair as plain DTOs", () => {
    const reg = new Diagnostics(vi.fn())
    reg.createCollection("a").set(uri("/f.ts"), [diag(0, "from-a", 1)])
    reg.createCollection("b").set(uri("/f.ts"), [diag(1, "from-b", 0)])
    const snap = reg.snapshotDtos()
    expect(snap).toHaveLength(2) // one entry per owner for the same uri
    expect(snap.every((e) => e.uri === uri("/f.ts").fsPath)).toBe(true)
    expect(new Set(snap.map((e) => e.owner)).size).toBe(2)
    expect(snap.flatMap((e) => e.diagnostics).map((d) => d.message).sort()).toEqual(["from-a", "from-b"])
  })

  it("two collections get distinct owners (no marker clobber)", () => {
    const bc = vi.fn()
    const reg = new Diagnostics(bc)
    reg.createCollection("dup").set(uri("/a.ts"), [diag(0, "x")])
    reg.createCollection("dup").set(uri("/a.ts"), [diag(0, "y")])
    const owners = bc.mock.calls.map((c) => (c[1] as { owner: string }).owner)
    expect(new Set(owners).size).toBe(2)
  })
})

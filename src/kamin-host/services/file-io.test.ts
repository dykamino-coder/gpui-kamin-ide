// #15 — deletePath honors the recursive flag (vscode workspace.fs.delete
// semantics): recursive:false removes files + EMPTY dirs but throws on a
// non-empty directory; recursive (default) removes the whole tree.
import { mkdirSync, mkdtempSync, writeFileSync, existsSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { describe, expect, it } from "vitest"
import { deletePath } from "./file-io.js"

const tmp = (): string => mkdtempSync(join(tmpdir(), "kamin-fileio-"))

describe("deletePath — recursive flag (#15)", () => {
  it("recursive:false deletes a file", async () => {
    const dir = tmp()
    const f = join(dir, "a.txt")
    writeFileSync(f, "x")
    await deletePath(f, false)
    expect(existsSync(f)).toBe(false)
  })

  it("recursive:false deletes an EMPTY directory", async () => {
    const dir = tmp()
    const sub = join(dir, "empty")
    mkdirSync(sub)
    await deletePath(sub, false)
    expect(existsSync(sub)).toBe(false)
  })

  it("recursive:false THROWS on a non-empty directory (vscode parity)", async () => {
    const dir = tmp()
    const sub = join(dir, "full")
    mkdirSync(sub)
    writeFileSync(join(sub, "child.txt"), "x")
    await expect(deletePath(sub, false)).rejects.toThrow()
    expect(existsSync(sub)).toBe(true) // untouched
  })

  it("recursive (default) removes the whole tree", async () => {
    const dir = tmp()
    const sub = join(dir, "full")
    mkdirSync(sub)
    writeFileSync(join(sub, "child.txt"), "x")
    await deletePath(sub)
    expect(existsSync(sub)).toBe(false)
  })
})

import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { describe, it, expect, beforeEach, afterEach } from "vitest"
import {
  discoverExtensions, resolveExtensionMain, coerceManifestString,
  parseActivationEvents, contributedCommandIds,
} from "./loader-discovery.js"

describe("discoverExtensions", () => {
  let dir: string
  beforeEach(() => { dir = mkdtempSync(join(tmpdir(), "kaminide-disc-")) })
  afterEach(() => { rmSync(dir, { recursive: true, force: true }) })

  it("returns nothing for an empty dir", () => {
    expect(discoverExtensions(dir)).toEqual([])
  })

  it("skips children without package.json", () => {
    mkdirSync(join(dir, "alpha"))
    expect(discoverExtensions(dir)).toEqual([])
  })

  it("skips children with malformed package.json", () => {
    mkdirSync(join(dir, "bad"))
    writeFileSync(join(dir, "bad", "package.json"), "{ this is not json")
    expect(discoverExtensions(dir)).toEqual([])
  })

  it("returns parsed manifest + path for a valid extension", () => {
    mkdirSync(join(dir, "ok"))
    writeFileSync(join(dir, "ok", "package.json"), JSON.stringify({ name: "ok", main: "./out.js" }))
    const out = discoverExtensions(dir)
    expect(out).toHaveLength(1)
    expect(out[0]?.manifest.name).toBe("ok")
    expect(out[0]?.path.endsWith("ok")).toBe(true)
  })
})

describe("resolveExtensionMain — path traversal guard", () => {
  it("accepts a relative path inside the extension dir", () => {
    expect(resolveExtensionMain("/ext/foo", "out/extension.js")).toBeTruthy()
  })

  it("accepts the extension dir itself (no relative segment)", () => {
    expect(resolveExtensionMain("/ext/foo", ".")).toBeTruthy()
  })

  it("rejects ../escape paths", () => {
    expect(resolveExtensionMain("/ext/foo", "../sibling/payload")).toBeUndefined()
    expect(resolveExtensionMain("/ext/foo", "../../etc/passwd")).toBeUndefined()
  })

  it("rejects an absolute path that isn't a descendant", () => {
    expect(resolveExtensionMain("/ext/foo", "/tmp/payload")).toBeUndefined()
  })
})

describe("parseActivationEvents (B3)", () => {
  it("returns the string events, ignoring non-strings and absent field", () => {
    expect(parseActivationEvents({ activationEvents: ["*", "onCommand:x", 5] })).toEqual(["*", "onCommand:x"])
    expect(parseActivationEvents({})).toEqual([])
    expect(parseActivationEvents({ activationEvents: "nope" })).toEqual([])
  })
})

describe("contributedCommandIds (B3)", () => {
  it("extracts command ids from contributes.commands", () => {
    const manifest = { contributes: { commands: [{ command: "a.do", title: "Do" }, { command: "a.undo", title: "Undo" }, { title: "noId" }] } }
    expect(contributedCommandIds(manifest)).toEqual(["a.do", "a.undo"])
  })
  it("returns [] when there are no contributed commands", () => {
    expect(contributedCommandIds({})).toEqual([])
    expect(contributedCommandIds({ contributes: {} })).toEqual([])
  })
})

describe("coerceManifestString", () => {
  it("returns the string when non-empty", () => {
    expect(coerceManifestString("hello")).toBe("hello")
  })

  it("returns undefined for empty string, non-strings, or absent", () => {
    expect(coerceManifestString("")).toBeUndefined()
    expect(coerceManifestString(undefined)).toBeUndefined()
    expect(coerceManifestString(123)).toBeUndefined()
    expect(coerceManifestString(null)).toBeUndefined()
  })
})

// B2 — verifies `vscode.workspace.getConfiguration` is really layered
// (default ← global ← workspace) over the host config service, not stubbed.
// Wires buildConfiguration to the REAL config + workspace service modules
// against temp dirs, the same way kamin-host does.
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { afterAll, beforeAll, describe, expect, it } from "vitest"
import {
  getAllConfig, initConfig, inspectConfig, onConfigChange,
  registerConfigDefaults, updateConfig,
} from "../../kamin-host/services/config.js"
import { getWorkspaceFolder, initWorkspace, setWorkspaceFolder } from "../../kamin-host/services/workspace.js"
import type { ConfigHost, WorkspaceHost } from "../host-services.js"
import type { NsHooks } from "./ns-builders.js"
import { buildConfiguration } from "./ns-config.js"

let dataDir: string
let folder: string
let cfg: ReturnType<typeof buildConfiguration>

beforeAll(() => {
  dataDir = mkdtempSync(join(tmpdir(), "kamin-cfg-data-"))
  folder = mkdtempSync(join(tmpdir(), "kamin-cfg-ws-"))
  initWorkspace(dataDir, null)
  initConfig(dataDir)
  setWorkspaceFolder(folder)

  const configHost: ConfigHost = {
    registerDefaults: (v) => { registerConfigDefaults(v) },
    getAll: () => getAllConfig(),
    inspect: (k) => inspectConfig(k),
    update: (k, val, t) => updateConfig(k, val, t),
    onDidChange: (fn) => onConfigChange(fn),
  }
  // Stub only the surface buildConfiguration touches (config + getFolderPath);
  // fs/watch/folder members are exercised by ns-workspace.test.ts.
  const workspaceHost = {
    getFolderPath: () => getWorkspaceFolder().path,
    config: configHost,
  } as unknown as WorkspaceHost
  cfg = buildConfiguration({ workspaceHost } as unknown as NsHooks)
})

afterAll(() => {
  rmSync(dataDir, { recursive: true, force: true })
  rmSync(folder, { recursive: true, force: true })
})

describe("getConfiguration defaults (B2)", () => {
  it("reads a contributed default via section + via full key", () => {
    registerConfigDefaults({ "t1.alpha": 14 })
    expect(cfg.getConfiguration("t1").get("alpha")).toBe(14)
    expect(cfg.getConfiguration().get("t1.alpha")).toBe(14)
  })

  it("has() reflects presence", () => {
    registerConfigDefaults({ "t2.beta": 1 })
    expect(cfg.getConfiguration("t2").has("beta")).toBe(true)
    expect(cfg.getConfiguration("t2").has("nope")).toBe(false)
  })

  it("get() falls back to the passed default for unknown keys", () => {
    expect(cfg.getConfiguration("t2").get("missing", "fallback")).toBe("fallback")
  })
})

describe("getConfiguration layers (B2)", () => {
  it("global update wins over default; inspect shows both layers", async () => {
    registerConfigDefaults({ "t3.gamma": 14 })
    await cfg.getConfiguration("t3").update("gamma", 16, 1) // Global
    expect(cfg.getConfiguration("t3").get("gamma")).toBe(16)
    const ins = cfg.getConfiguration("t3").inspect("gamma")
    expect(ins.defaultValue).toBe(14)
    expect(ins.globalValue).toBe(16)
  })

  it("workspace update wins over global and writes .vscode/settings.json", async () => {
    registerConfigDefaults({ "prec.x": 1 })
    await cfg.getConfiguration("prec").update("x", 2, 1) // Global
    await cfg.getConfiguration("prec").update("x", 3, 2) // Workspace
    expect(cfg.getConfiguration("prec").get("x")).toBe(3)
    const settings = join(folder, ".vscode", "settings.json")
    expect(existsSync(settings)).toBe(true)
    const onDisk = JSON.parse(readFileSync(settings, "utf8")) as Record<string, unknown>
    expect(onDisk["prec.x"]).toBe(3)
    const ins = cfg.getConfiguration("prec").inspect("x")
    expect(ins).toMatchObject({ defaultValue: 1, globalValue: 2, workspaceValue: 3 })
  })

  it("update(undefined) resets a layer", async () => {
    registerConfigDefaults({ "reset.k": "def" })
    await cfg.getConfiguration("reset").update("k", "user", 1)
    expect(cfg.getConfiguration("reset").get("k")).toBe("user")
    await cfg.getConfiguration("reset").update("k", undefined, 1)
    expect(cfg.getConfiguration("reset").get("k")).toBe("def")
  })
})

describe("JSONC settings.json (B2)", () => {
  it("reads commented settings and preserves comments on write", async () => {
    const dir = join(folder, ".vscode")
    mkdirSync(dir, { recursive: true })
    writeFileSync(join(dir, "settings.json"), [
      "{",
      "  // user's hand-written comment",
      '  "jsonc.existing": true,',
      "}",
    ].join("\n"))
    // The workspace layer caches per folder and invalidates on folder change;
    // toggle to force a re-read of the file we just wrote out of band (the
    // same path a reopen takes — live external-edit watching is a later step).
    setWorkspaceFolder(null)
    setWorkspaceFolder(folder)
    expect(cfg.getConfiguration("jsonc").get("existing")).toBe(true)

    await cfg.getConfiguration("jsonc").update("added", 42, 2) // Workspace
    const text = readFileSync(join(dir, "settings.json"), "utf8")
    expect(text).toContain("// user's hand-written comment")
    expect(cfg.getConfiguration("jsonc").get("added")).toBe(42)
    expect(cfg.getConfiguration("jsonc").get("existing")).toBe(true)
  })
})

describe("property-access form (B2)", () => {
  it("exposes config[key] equal to config.get(key)", () => {
    registerConfigDefaults({ "prop.fontSize": 13 })
    const editor = cfg.getConfiguration("prop") as unknown as Record<string, unknown>
    expect(editor.fontSize).toBe(13)
    expect(cfg.getConfiguration("prop").get("fontSize")).toBe(13)
    expect(editor.unknownKey).toBeUndefined()
  })
})

describe("getConfiguration sub-tree (B2)", () => {
  it("returns a nested object for a section group", () => {
    registerConfigDefaults({ "grp.one": 1, "grp.nested.two": 2 })
    const grp = cfg.getConfiguration().get<Record<string, unknown>>("grp")
    expect(grp).toMatchObject({ one: 1, nested: { two: 2 } })
  })
})

describe("onDidChangeConfiguration (B2)", () => {
  it("fires with affectsConfiguration scoped to the changed key", async () => {
    const affected: boolean[] = []
    cfg.onDidChangeConfiguration((e) => {
      affected.push(e.affectsConfiguration("evt"), e.affectsConfiguration("other"))
    })
    await cfg.getConfiguration("evt").update("x", 1, 1)
    expect(affected).toEqual([true, false])
  })
})

// B9 persistence — globalState/workspaceState Memento semantics, per-folder
// workspaceState isolation, secrets + change events, and the storage dirs.
import { mkdtempSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { beforeEach, describe, expect, it } from "vitest"
import { JsonStore } from "../json-store.js"
import * as storage from "./storage.js"
import { initWorkspace, setWorkspaceFolder } from "./workspace.js"

const tmp = (): string => mkdtempSync(join(tmpdir(), "kamin-storage-"))
const EXT = "pub.ext"

describe("storage (B9)", () => {
  beforeEach(() => {
    const dir = tmp()
    initWorkspace(dir, null)
    setWorkspaceFolder(null)
    storage.initStorage(dir)
  })

  it("globalState: update → get → keys, and undefined removes", () => {
    storage.globalUpdate(EXT, "k", 42)
    storage.globalUpdate(EXT, "other", "x")
    expect(storage.globalGet(EXT, "k")).toBe(42)
    expect(storage.globalKeys(EXT)).toEqual(expect.arrayContaining(["k", "other"]))
    storage.globalUpdate(EXT, "k", undefined)
    expect(storage.globalGet(EXT, "k")).toBeUndefined()
    expect(storage.globalKeys(EXT)).toEqual(["other"])
  })

  it("workspaceState is isolated per open folder and persists", () => {
    const a = tmp()
    const b = tmp()
    setWorkspaceFolder(a)
    storage.workspaceUpdate(EXT, "k", "inA")
    expect(storage.workspaceGet(EXT, "k")).toBe("inA")
    setWorkspaceFolder(b)
    expect(storage.workspaceGet(EXT, "k")).toBeUndefined() // b has its own scope
    setWorkspaceFolder(a)
    expect(storage.workspaceGet(EXT, "k")).toBe("inA") // a's value survived
  })

  it("workspaceState works in-memory when no folder is open", () => {
    setWorkspaceFolder(null)
    storage.workspaceUpdate(EXT, "k", 1)
    expect(storage.workspaceGet(EXT, "k")).toBe(1)
  })

  it("secrets: set → get → keys → delete, with change events", () => {
    const seen: string[] = []
    const off = storage.onSecretChange(EXT, (key) => seen.push(key))
    storage.secretSet(EXT, "token", "s3cr3t")
    expect(storage.secretGet(EXT, "token")).toBe("s3cr3t")
    expect(storage.secretKeys(EXT)).toEqual(["token"])
    storage.secretDelete(EXT, "token")
    expect(storage.secretGet(EXT, "token")).toBeUndefined()
    expect(seen).toEqual(["token", "token"]) // store + delete both fire
    off()
    storage.secretSet(EXT, "token", "again")
    expect(seen).toEqual(["token", "token"]) // unsubscribed
  })

  it("storageDir is null without a folder, a path with one; global/log always present", () => {
    setWorkspaceFolder(null)
    expect(storage.storageDir(EXT)).toBeNull()
    expect(storage.globalStorageDir(EXT)).toMatch(/globalStorage/)
    expect(storage.logDir(EXT)).toMatch(/logs/)
    const folder = tmp()
    setWorkspaceFolder(folder)
    expect(storage.storageDir(EXT)).toMatch(/workspaceStorage/)
  })

  it("globalState survives a re-open of the same data dir", () => {
    const dir = tmp()
    storage.initStorage(dir)
    storage.globalUpdate(EXT, "persisted", "yes")
    // A restart means the process EXITED, and exiting flushes (writes are
    // debounced by FLUSH_DEBOUNCE_MS). Without this the test re-opened the dir
    // mid-debounce — a crash, not a restart — and failed on a store that works.
    JsonStore.flushAllSync()
    storage.initStorage(dir) // now: a host restart on the same dataDir
    expect(storage.globalGet(EXT, "persisted")).toBe("yes")
  })

  it("an unflushed globalState write is still readable in-memory (disk lags by design)", () => {
    storage.globalUpdate(EXT, "hot", "value")
    expect(storage.globalGet(EXT, "hot")).toBe("value") // cache is immediate truth
  })
})

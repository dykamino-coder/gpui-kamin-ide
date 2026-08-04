// B1 — verifies `vscode.workspace` (folders + fs) is really bridged to the
// host file-io service, not stubbed. Wires buildWorkspace to the REAL
// file-io module against a temp dir, the same way kamin-host does.
import { mkdtempSync, rmSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { afterAll, beforeAll, describe, expect, it } from "vitest"
import { URI } from "vscode-uri"
import * as fsIo from "../../kamin-host/services/file-io.js"
import type { IndexedFileRef, WatchEvent, WorkspaceHost } from "../host-services.js"
import type { NsHooks } from "./ns-builders.js"
import { buildDocuments } from "./ns-documents.js"
import { buildWorkspace } from "./ns-workspace.js"

let root: string

interface Harness {
  ws: ReturnType<typeof buildWorkspace>
  emitWatch: (events: WatchEvent[]) => void
  emitFolderChange: (path: string | null) => void
}

function makeWorkspace(folderPath: string | null, files: IndexedFileRef[] = []): Harness {
  const watchSinks = new Set<(e: readonly WatchEvent[]) => void>()
  const folderSinks = new Set<(p: string | null) => void>()
  const workspaceHost: WorkspaceHost = {
    getFolderPath: () => folderPath,
    onDidChangeFolder: (fn) => { folderSinks.add(fn); return () => folderSinks.delete(fn) },
    listFiles: () => Promise.resolve(files),
    watchFiles: (fn) => { watchSinks.add(fn); return () => watchSinks.delete(fn) },
    fs: {
      stat: (p) => fsIo.statPath(p),
      readFile: (p) => fsIo.readBytes(p),
      writeFile: (p, d) => fsIo.writeBytes(p, d),
      readDirectory: async (p) => (await fsIo.listDir(p)).map((e) => [e.name, e.type] as [string, typeof e.type]),
      createDirectory: (p) => fsIo.makeDir(p),
      delete: (p) => fsIo.deletePath(p),
      rename: (s, d) => fsIo.movePath(s, d),
      copy: (s, d, o) => fsIo.copyPath(s, d, o),
    },
    // Config plane is exercised by ns-config.test.ts; stub here so
    // buildWorkspace's buildConfiguration wiring has a surface.
    config: {
      registerDefaults: () => {},
      getAll: () => ({}),
      inspect: () => ({}),
      update: () => Promise.resolve(),
      onDidChange: () => () => {},
    },
    // Document plane is exercised by text-document.test.ts; stub here.
    documents: {
      list: () => [],
      get: () => undefined,
      open: () => {},
      onDidOpen: () => () => {},
      onDidChange: () => () => {},
      onDidClose: () => () => {},
      onDidSave: () => () => {},
    },
    // Editor plane is exercised by ns-editor.test.ts; stub here.
    editors: {
      getActive: () => null,
      getSelections: () => [],
      onDidChangeActive: () => () => {},
      onDidChangeSelection: () => () => {},
      applyEdits: () => Promise.resolve(false),
      revealRange: () => Promise.resolve(),
      showDocument: () => Promise.resolve(),
      setDecorations: () => { /* unused */ },
      disposeDecorationType: () => { /* unused */ },
      insertSnippet: () => Promise.resolve(false),
      setSelections: () => { /* unused */ },
    },
  }
  const hooks = { workspaceHost } as unknown as NsHooks
  return {
    ws: buildWorkspace(hooks, buildDocuments(hooks)),
    emitWatch: (events) => { for (const fn of watchSinks) fn(events) },
    emitFolderChange: (path) => { for (const fn of folderSinks) fn(path) },
  }
}

beforeAll(() => { root = mkdtempSync(join(tmpdir(), "kamin-ws-")) })
afterAll(() => { rmSync(root, { recursive: true, force: true }) })

describe("workspace folders (B1)", () => {
  it("reflects the open folder", () => {
    const { ws } = makeWorkspace(root)
    expect(ws.workspaceFolders).toHaveLength(1)
    expect(ws.workspaceFolders?.[0]?.uri.fsPath).toBe(URI.file(root).fsPath)
    expect(ws.name).toBeTypeOf("string")
  })

  it("is undefined with no folder", () => {
    expect(makeWorkspace(null).ws.workspaceFolders).toBeUndefined()
  })

  it("computes asRelativePath + getWorkspaceFolder", () => {
    const { ws } = makeWorkspace(root)
    const inside = URI.file(join(root, "sub", "f.ts"))
    expect(ws.asRelativePath(inside).replace(/\\/g, "/")).toBe("sub/f.ts")
    expect(ws.getWorkspaceFolder(inside)).toBeDefined()
    expect(ws.getWorkspaceFolder(URI.file(join(tmpdir(), "elsewhere.ts")))).toBeUndefined()
  })
})

describe("workspace.fs (B1)", () => {
  it("writes then reads raw bytes", async () => {
    const { ws } = makeWorkspace(root)
    const uri = URI.file(join(root, "hello.txt"))
    const bytes = new TextEncoder().encode("héllo")
    await ws.fs.writeFile(uri, bytes)
    const back = await ws.fs.readFile(uri)
    expect(new TextDecoder().decode(back)).toBe("héllo")
  })

  it("stats a file (type File, real size)", async () => {
    const { ws } = makeWorkspace(root)
    const uri = URI.file(join(root, "s.txt"))
    await ws.fs.writeFile(uri, new Uint8Array([1, 2, 3]))
    const st = await ws.fs.stat(uri)
    expect(st.type).toBe(1) // FileType.File
    expect(st.size).toBe(3)
  })

  it("lists a directory with file types", async () => {
    const { ws } = makeWorkspace(root)
    await ws.fs.createDirectory(URI.file(join(root, "dir")))
    await ws.fs.writeFile(URI.file(join(root, "dir", "a.txt")), new Uint8Array())
    const entries = await ws.fs.readDirectory(URI.file(join(root, "dir")))
    expect(entries).toContainEqual(["a.txt", 1]) // [name, FileType.File]
  })
})

describe("findFiles (B1b)", () => {
  const files = [
    { rel: "src/a.ts", abs: join(root || "/x", "src", "a.ts") },
    { rel: "src/b.js", abs: join(root || "/x", "src", "b.js") },
    { rel: "node_modules/x/c.ts", abs: join(root || "/x", "node_modules", "x", "c.ts") },
  ]

  it("glob-matches the include pattern", async () => {
    const { ws } = makeWorkspace(root, files)
    const hits = await ws.findFiles("**/*.ts")
    expect(hits).toHaveLength(2)
    expect(hits.map((u) => u.fsPath)).toContain(files[0]?.abs)
  })

  it("honours exclude + maxResults", async () => {
    const { ws } = makeWorkspace(root, files)
    expect(await ws.findFiles("**/*.ts", "**/node_modules/**")).toHaveLength(1)
    expect(await ws.findFiles("**/*", undefined, 1)).toHaveLength(1)
  })
})

describe("createFileSystemWatcher (B1b)", () => {
  it("fires onDidCreate/Change/Delete for matching paths only", () => {
    const { ws, emitWatch } = makeWorkspace(root)
    const watcher = ws.createFileSystemWatcher("**/*.ts")
    const created: string[] = []
    const deleted: string[] = []
    watcher.onDidCreate((u: URI) => created.push(u.fsPath))
    watcher.onDidDelete((u: URI) => deleted.push(u.fsPath))
    emitWatch([
      { kind: "add", path: join(root, "x.ts") },
      { kind: "add", path: join(root, "skip.js") },
      { kind: "unlink", path: join(root, "y.ts") },
    ])
    expect(created).toEqual([URI.file(join(root, "x.ts")).fsPath])
    expect(deleted).toEqual([URI.file(join(root, "y.ts")).fsPath])
    watcher.dispose()
  })
})

describe("onDidChangeWorkspaceFolders (B1b)", () => {
  it("fires when the host folder changes", () => {
    const { ws, emitFolderChange } = makeWorkspace(root)
    let fired = 0
    ws.onDidChangeWorkspaceFolders(() => { fired += 1 })
    emitFolderChange("/other")
    expect(fired).toBe(1)
  })

  it("reports a real added/removed diff on a folder switch (not empty arrays)", () => {
    const { added, removed, emitFolderChange } = captureFolderDiff(root)
    emitFolderChange("/other")
    expect(added).toEqual([URI.file("/other").fsPath])
    expect(removed).toEqual([URI.file(root).fsPath])
  })

  it("a close (folder → null) is removed-only", () => {
    const { added, removed, emitFolderChange } = captureFolderDiff(root)
    emitFolderChange(null)
    expect(added).toEqual([])
    expect(removed).toEqual([URI.file(root).fsPath])
  })
})

interface WorkspaceFolderShape { uri: URI; name: string; index: number }
/** Wire a workspace, subscribe, and accumulate the fsPaths of each folder-change
 *  event's added/removed lists (avoids TS not narrowing a closure-assigned var). */
function captureFolderDiff(folderPath: string): {
  added: string[]; removed: string[]; emitFolderChange: (path: string | null) => void
} {
  const { ws, emitFolderChange } = makeWorkspace(folderPath)
  const added: string[] = []
  const removed: string[] = []
  ws.onDidChangeWorkspaceFolders((e) => {
    const ev = e as { added: readonly WorkspaceFolderShape[]; removed: readonly WorkspaceFolderShape[] }
    added.push(...ev.added.map((f) => f.uri.fsPath))
    removed.push(...ev.removed.map((f) => f.uri.fsPath))
  })
  return { added, removed, emitFolderChange }
}

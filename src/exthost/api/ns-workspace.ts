// `vscode.workspace` namespace (B1) — split from ns-data.ts so each stays
// under the 250-LOC ceiling. Folder + fs + findFiles + watchers are bridged
// to the host services via the injected `WorkspaceHost` (host-services.ts).
import { basename, relative } from "node:path"
import { minimatch } from "minimatch"
import { URI } from "vscode-uri"
import type { HostFileType } from "../host-services.js"
import type { WorkspaceFileOp } from "./classes-core.js"
import { FileType } from "./enums.js"
import type { NsHooks } from "./ns-builders.js"
import { buildConfiguration } from "./ns-config.js"
import type { DocumentsApi } from "./ns-documents.js"
import { Disposable, EventEmitter, noopEvent } from "./shared.js"

/** Map the host's string file type to VS Code's `FileType` value (number;
 *  `FileType` is a const-object enum, not a TS type). */
function toFileType(t: HostFileType): number {
  if (t === "dir") return FileType.Directory
  if (t === "file") return FileType.File
  if (t === "symlink") return FileType.SymbolicLink
  return FileType.Unknown
}

/** A vscode GlobPattern is a string or a RelativePattern ({ pattern, … }). */
function patternOf(p: unknown): string {
  if (typeof p === "string") return p
  if (p && typeof p === "object" && "pattern" in p) return String((p).pattern)
  return "**/*"
}

function globMatch(rel: string, pattern: string): boolean {
  return minimatch(rel, pattern, { dot: true, nocase: process.platform === "win32" })
}

/** Workspace-relative POSIX path, or null if `abs` is outside `root`. */
function relPosix(root: string, abs: string): string | null {
  const rel = relative(root, abs).split(/[\\/]/).join("/")
  return rel.startsWith("..") ? null : rel
}

export function buildWorkspace(h: NsHooks, docs: DocumentsApi) {
  // Live folder lookup — read on each access so it tracks Open Folder (B1).
  const folders = () => {
    const path = h.workspaceHost.getFolderPath()
    return path ? [{ uri: URI.file(path), name: basename(path), index: 0 }] : undefined
  }
  const fsHost = h.workspaceHost.fs

  // onDidChangeWorkspaceFolders (B1b): fire a REAL diff when the host's open
  // folder changes (the d.ts contract guarantees at least one of added/removed
  // is non-empty). Single-root today, so a switch is removed:[old] + added:[new],
  // an open is added-only, a close is removed-only. We track the previous path
  // here rather than threading it through the host signature.
  const folderOf = (path: string) => ({ uri: URI.file(path), name: basename(path), index: 0 })
  const folderChange = new EventEmitter<{ added: readonly unknown[]; removed: readonly unknown[] }>()
  let prevFolderPath = h.workspaceHost.getFolderPath()
  h.workspaceHost.onDidChangeFolder((nextPath) => {
    if (nextPath === prevFolderPath) return
    const added = nextPath ? [folderOf(nextPath)] : []
    const removed = prevFolderPath ? [folderOf(prevFolderPath)] : []
    prevFolderPath = nextPath
    folderChange.fire({ added, removed })
  })
  const config = buildConfiguration(h)
  return {
    get workspaceFolders() { return folders() },
    workspaceFile: undefined,
    get name() { return folders()?.[0]?.name },
    get rootPath() { return h.workspaceHost.getFolderPath() ?? undefined },
    get textDocuments() { return docs.list() },
    notebookDocuments: [] as unknown[],
    isTrusted: true,
    trustedDomains: [] as string[],
    // Configuration (B2) — real layered get/has/inspect/update + change event.
    getConfiguration: config.getConfiguration,
    onDidChangeConfiguration: config.onDidChangeConfiguration,
    onDidChangeWorkspaceFolders: folderChange.event,
    onDidOpenTextDocument: docs.onDidOpen, onDidCloseTextDocument: docs.onDidClose,
    onDidChangeTextDocument: docs.onDidChange, onDidSaveTextDocument: docs.onDidSave,
    onWillSaveTextDocument: noopEvent,
    onDidCreateFiles: noopEvent, onDidDeleteFiles: noopEvent, onDidRenameFiles: noopEvent,
    onWillCreateFiles: noopEvent, onWillDeleteFiles: noopEvent, onWillRenameFiles: noopEvent,
    onDidOpenNotebookDocument: noopEvent, onDidCloseNotebookDocument: noopEvent,
    onDidChangeNotebookDocument: noopEvent, onDidSaveNotebookDocument: noopEvent,
    onDidGrantWorkspaceTrust: noopEvent,
    // Real filesystem (B1) — bridged to the host fs service. `uri` is a
    // vscode-uri URI; `.fsPath` gives the OS path the host service wants.
    fs: {
      async stat(uri: URI) {
        const s = await fsHost.stat(uri.fsPath)
        return { type: toFileType(s.type), ctime: s.ctimeMs, mtime: s.mtimeMs, size: s.size }
      },
      readFile: (uri: URI) => fsHost.readFile(uri.fsPath),
      writeFile: (uri: URI, content: Uint8Array) => fsHost.writeFile(uri.fsPath, content),
      async readDirectory(uri: URI): Promise<[string, number][]> {
        const entries = await fsHost.readDirectory(uri.fsPath)
        return entries.map(([name, t]) => [name, toFileType(t)])
      },
      createDirectory: (uri: URI) => fsHost.createDirectory(uri.fsPath),
      delete: (uri: URI, options?: { recursive?: boolean; useTrash?: boolean }) =>
        fsHost.delete(uri.fsPath, options?.recursive ?? false, options?.useTrash ?? false),
      rename: (source: URI, target: URI, options?: { overwrite?: boolean }) =>
        fsHost.rename(source.fsPath, target.fsPath, options?.overwrite ?? false),
      copy: (source: URI, target: URI, options?: { overwrite?: boolean }) => fsHost.copy(source.fsPath, target.fsPath, options?.overwrite ?? false),
      isWritableFileSystem: () => true,
    },
    registerTextDocumentContentProvider: () => new Disposable(() => {}),
    registerFileSystemProvider: () => new Disposable(() => {}),
    registerTaskProvider: () => new Disposable(() => {}),
    registerNotebookSerializer: () => new Disposable(() => {}),
    asRelativePath: (p: unknown, includeWorkspaceFolder?: boolean) => {
      const folder = folders()?.[0]
      const target = p instanceof URI ? p.fsPath : String(p)
      if (!folder) return target
      const rel = relative(folder.uri.fsPath, target)
      if (rel.startsWith("..")) return target
      return includeWorkspaceFolder ? `${folder.name}/${rel}` : rel
    },
    getWorkspaceFolder: (uri: URI) => {
      const folder = folders()?.[0]
      if (!folder) return undefined
      const rel = relative(folder.uri.fsPath, uri.fsPath)
      return rel.startsWith("..") ? undefined : folder
    },
    updateWorkspaceFolders: () => true,
    // findFiles (B1b): glob-match the host file index. include/exclude are
    // string globs or RelativePattern; matched against workspace-rel paths.
    findFiles: async (include: unknown, exclude?: unknown, maxResults?: number) => {
      const inc = patternOf(include)
      const exc = exclude == null ? null : patternOf(exclude)
      const out: URI[] = []
      for (const f of await h.workspaceHost.listFiles()) {
        if (!globMatch(f.rel, inc)) continue
        if (exc && globMatch(f.rel, exc)) continue
        out.push(URI.file(f.abs))
        if (maxResults && out.length >= maxResults) break
      }
      return out
    },
    saveAll: () => Promise.resolve(true),
    // vscode.workspace.applyEdit(WorkspaceEdit) — really apply each bucket's text
    // edits (was a silent `true` no-op that dropped every refactor/quick-fix).
    // File create/delete/rename run first, then per-uri text edits go through the
    // renderer's Monaco (opening the model if needed — applyEdits no-ops on an
    // unopened doc). Resolves false if any sub-edit failed, like vscode.
    applyEdit: async (edit: unknown) => {
      const we = edit as {
        entries?: () => [unknown, { range: unknown; newText: string }[]][]
        // Тип общий с WorkspaceEdit (classes-core) — не дублировать форму.
        fileOps?: WorkspaceFileOp[]
      }
      if (!we || typeof we.entries !== "function") return false
      const ed = h.workspaceHost.editors
      const fsp = (u: unknown): string => (u instanceof URI ? u.fsPath : URI.parse(String(u)).fsPath)
      const toRange = (r: unknown) => {
        const g = r as { start?: { line: number; character: number }; end?: { line: number; character: number }; line?: number; character?: number }
        const s = g?.start ?? g
        const e = g?.end ?? s
        return { startLine: s?.line ?? 0, startChar: s?.character ?? 0, endLine: e?.line ?? 0, endChar: e?.character ?? 0 }
      }
      let ok = true
      // Опции vscode-семантики (раньше терялись: create обнулял существующий
      // файл, delete был всегда recursive, rename никогда не overwrite).
      const o = (op: { options?: NonNullable<typeof we.fileOps>[number]["options"] }) => op.options ?? {}
      const existsAt = (p: string) => h.workspaceHost.fs.stat(p).then(() => true, () => false)
      for (const op of we.fileOps ?? []) {
        const opts = o(op)
        try {
          if (op.type === "create") {
            const p = fsp(op.uri)
            if (!opts.overwrite && await existsAt(p)) {
              if (!opts.ignoreIfExists) ok = false
              continue
            }
            const c = opts.contents
            const data = c instanceof Uint8Array ? c
              : typeof c?.data === "function" ? await c.data()
              : new Uint8Array()
            await h.workspaceHost.fs.writeFile(p, data)
          } else if (op.type === "delete") {
            const p = fsp(op.uri)
            if (!(await existsAt(p))) {
              if (!opts.ignoreIfNotExists) ok = false
              continue
            }
            await h.workspaceHost.fs.delete(p, opts.recursive ?? false, false)
          } else if (op.type === "rename") {
            const to = fsp(op.to)
            if (!opts.overwrite && await existsAt(to)) {
              if (!opts.ignoreIfExists) ok = false
              continue
            }
            await h.workspaceHost.fs.rename(fsp(op.from), to, opts.overwrite ?? false)
          }
        } catch { ok = false }
      }
      for (const [uri, edits] of we.entries()) {
        const p = fsp(uri)
        await ed.showDocument(p).catch(() => { /* best-effort open */ })
        const ops = (edits ?? []).map((e) => ({ range: toRange(e.range), text: e.newText }))
        if (ops.length > 0 && !(await ed.applyEdits(p, ops))) ok = false
      }
      return ok
    },
    openTextDocument: docs.openTextDocument,
    openNotebookDocument: () => Promise.resolve({ uri: undefined }),
    // createFileSystemWatcher (B1b): glob-filter the host watcher's events.
    createFileSystemWatcher: (
      globPattern: unknown,
      ignoreCreate?: boolean,
      ignoreChange?: boolean,
      ignoreDelete?: boolean,
    ) => {
      const pattern = patternOf(globPattern)
      const onDidCreate = new EventEmitter<URI>()
      const onDidChange = new EventEmitter<URI>()
      const onDidDelete = new EventEmitter<URI>()
      const unsub = h.workspaceHost.watchFiles((events) => {
        const root = h.workspaceHost.getFolderPath()
        if (!root) return
        for (const ev of events) {
          const rel = relPosix(root, ev.path)
          if (rel === null || !globMatch(rel, pattern)) continue
          const uri = URI.file(ev.path)
          if (ev.kind === "add" || ev.kind === "addDir") { if (!ignoreCreate) onDidCreate.fire(uri) }
          else if (ev.kind === "change") { if (!ignoreChange) onDidChange.fire(uri) }
          else if (!ignoreDelete) onDidDelete.fire(uri)
        }
      })
      return {
        ignoreCreateEvents: Boolean(ignoreCreate),
        ignoreChangeEvents: Boolean(ignoreChange),
        ignoreDeleteEvents: Boolean(ignoreDelete),
        onDidCreate: onDidCreate.event, onDidChange: onDidChange.event, onDidDelete: onDidDelete.event,
        dispose() { unsub(); onDidCreate.dispose(); onDidChange.dispose(); onDidDelete.dispose() },
      }
    },
    decode: () => Promise.resolve(""),
    encode: () => Promise.resolve(new Uint8Array()),
  }
}

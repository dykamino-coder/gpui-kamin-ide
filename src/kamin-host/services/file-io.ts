// Host-side file IO primitives. The renderer cannot touch the disk
// directly (sandbox + contextIsolation + RunAsNode=false fuses);
// every path goes through these helpers and a `kamin:fs:*` IPC.
//
// Why we don't expose `fs` raw to the renderer: a malicious extension
// renderer-script could exfil any file the user could read. By
// keeping IO in the host we get a single chokepoint to add audit
// logging / path allowlists when extensions land in Phase D.

import { execFile } from "node:child_process"
import { readFile, writeFile, readdir, stat, lstat, mkdir, rm, rmdir, rename, cp, open, unlink } from "node:fs/promises"
import { dirname, extname, join, normalize, sep } from "node:path"
import { fileURLToPath } from "node:url"
import { promisify } from "node:util"

const execFileAsync = promisify(execFile)

// Extensions that `env.openExternal` must NOT silently launch — it's meant for
// http/https/mailto + opening documents in their default app, not running code.
const OPEN_EXTERNAL_BLOCKED_EXT = new Set([
  ".exe", ".bat", ".cmd", ".com", ".scr", ".msi", ".ps1", ".psm1", ".vbs", ".vbe",
  ".js", ".jse", ".wsf", ".wsh", ".pif", ".reg", ".hta", ".cpl", ".msc", ".jar", ".lnk",
])

export interface DirEntry {
  name: string
  /** Absolute path, normalised by `path.normalize`. */
  path: string
  type: "file" | "dir" | "symlink" | "other"
  size: number
  /** UNIX epoch ms — used for cache invalidation on next launch. */
  mtimeMs: number
}

/** Read-text refuses past this. Beyond ~5 MB Monaco's tokeniser
 *  starts to choke and the panel feels unresponsive — same threshold
 *  VS Code uses before falling back to its large-file mode. */
const BYTES_PER_KB = 1024
const KB_PER_MB = 1024
const FILE_SIZE_GUARD_MB = 5
const FILE_SIZE_GUARD_BYTES = FILE_SIZE_GUARD_MB * KB_PER_MB * BYTES_PER_KB

/** UTF-8 BOM bytes — `EF BB BF`. Named so the BOM-strip below doesn't
 *  trip the no-magic-numbers rule. */
const BOM_BYTE_0 = 0xEF
const BOM_BYTE_1 = 0xBB
const BOM_BYTE_2 = 0xBF
const BOM_LENGTH = 3

/** List one directory's children — non-recursive. The recursive walk
 *  used by the indexer lives in `walker.ts` and reuses this. */
export async function listDir(absPath: string): Promise<DirEntry[]> {
  const entries = await readdir(absPath, { withFileTypes: true })
  const out: DirEntry[] = []
  for (const e of entries) {
    const childPath = join(absPath, e.name)
    let s
    try { s = await stat(childPath) } catch { continue } // unstattable entry (perms/race) → skip
    out.push({
      name: e.name,
      path: normalize(childPath),
      type: e.isDirectory() ? "dir" : e.isFile() ? "file" : e.isSymbolicLink() ? "symlink" : "other",
      size: s.size,
      mtimeMs: s.mtimeMs,
    })
  }
  // Tree convention: dirs first, then files, both alphabetical
  // case-insensitive. Same order as VS Code Explorer / Rider.
  out.sort((a, b) => {
    if (a.type !== b.type) return a.type === "dir" ? -1 : 1
    return a.name.localeCompare(b.name, undefined, { sensitivity: "base" })
  })
  return out
}

/** Read text file — UTF-8, BOM stripped. Throws on size > 5 MB so a
 *  surprise binary doesn't blow the renderer; viewer falls back to a
 *  preview / "open externally" hint. */
export async function readText(absPath: string): Promise<string> {
  const s = await stat(absPath)
  if (s.size > FILE_SIZE_GUARD_BYTES) {
    throw new Error(`file-io: ${absPath} is ${s.size} bytes, refuses to load (>${FILE_SIZE_GUARD_BYTES})`)
  }
  const buf = await readFile(absPath)
  // Strip UTF-8 BOM if present so Monaco doesn't show an invisible
  // leading codepoint that breaks first-column tooling.
  if (buf.length >= BOM_LENGTH && buf[0] === BOM_BYTE_0 && buf[1] === BOM_BYTE_1 && buf[2] === BOM_BYTE_2) {
    return buf.subarray(BOM_LENGTH).toString("utf8")
  }
  return buf.toString("utf8")
}

let tmpWriteCounter = 0

/** Write text. Atomic + durable: write to a UNIQUE temp file, fsync it, then
 *  rename over the target. A per-call unique name (was a fixed `<path>.tmp`)
 *  stops two concurrent writers from interleaving into one temp file, and the
 *  fsync makes the contract's crash-safety real (rename metadata could otherwise
 *  land before the data, leaving a zero-length/stale file after a power loss). */
export async function writeText(absPath: string, content: string): Promise<void> {
  await mkdir(dirname(absPath), { recursive: true })
  const tmp = `${absPath}.${String(process.pid)}.${String(++tmpWriteCounter)}.tmp`
  const fh = await open(tmp, "w")
  try {
    await fh.writeFile(content, { encoding: "utf8" })
    await fh.sync()
  } finally {
    await fh.close()
  }
  try {
    await rename(tmp, absPath)
  } catch (err) {
    await unlink(tmp).catch(() => { /* best-effort cleanup */ })
    throw err
  }
}

/** Windows extended-length prefix `\\?\` — bypasses Win32 name parsing so
 *  reserved device names (nul, con, aux, com1…) and >260-char paths can be
 *  addressed. No-op off Windows / if already prefixed. Requires an absolute
 *  backslash path (host paths are `path.normalize`d, so they qualify). */
function winExtended(p: string): string {
  if (process.platform !== "win32" || p.startsWith("\\\\?\\")) return p
  if (p.startsWith("\\\\")) return `\\\\?\\UNC\\${p.slice(2)}`
  return `\\\\?\\${p}`
}

/** Delete a path PERMANENTLY. `recursive` defaults to true (the renderer's tree
 *  delete, behind a Confirm modal). With `recursive: false` — vscode's
 *  `workspace.fs.delete` default — a file or EMPTY directory is removed but a
 *  non-empty directory throws (ENOTEMPTY), matching VS Code. Uses the `\\?\`
 *  prefix so it can also remove reserved-name files the Recycle Bin can't take. */
export async function deletePath(absPath: string, recursive = true): Promise<void> {
  const p = winExtended(absPath)
  if (recursive) { await rm(p, { recursive: true, force: true }); return }
  const st = await lstat(p).catch(() => null)
  if (st?.isDirectory()) await rmdir(p)
  else await rm(p, { force: true })
}

/** Move a path to the OS Recycle Bin (Windows) so the user can restore it —
 *  the default for tree deletes, unlike the permanent `deletePath`. Uses the
 *  VB FileSystem helper; the whole subtree recycles for a directory.
 *  security: the path is passed via an ENV var (read as `$env:KAMIN_TRASH_PATH`)
 *  rather than interpolated into the script, so no string-escaping is needed and
 *  the path can never break out of the literal into executable PowerShell. */
export async function trashPath(absPath: string): Promise<void> {
  const s = await stat(absPath)
  const method = s.isDirectory() ? "DeleteDirectory" : "DeleteFile"
  const script = `Add-Type -AssemblyName Microsoft.VisualBasic; [Microsoft.VisualBasic.FileIO.FileSystem]::${method}($env:KAMIN_TRASH_PATH,'OnlyErrorDialogs','SendToRecycleBin')`
  await execFileAsync("powershell.exe", ["-NoProfile", "-NonInteractive", "-Command", script], {
    windowsHide: true,
    env: { ...process.env, KAMIN_TRASH_PATH: absPath },
  })
}

/** Restore a path previously sent to the Recycle Bin back to its original
 *  location — the undo for `trashPath`. Matches the `$I` metadata records under
 *  `<drive>\$Recycle.Bin\<SID>\` by original path (locale-independent, unlike the
 *  shell "Restore" verb), then moves the paired `$R` data back. Returns true if a
 *  matching item was found. Windows-only. */
export async function restoreFromTrash(originalPath: string): Promise<boolean> {
  const binRoot = `${originalPath.slice(0, 2)}\\$Recycle.Bin`
  let sidDirs: string[]
  try { sidDirs = await readdir(binRoot) } catch { return false } // no Recycle Bin on this drive → not found
  const wanted = originalPath.replace(/\\/g, "/").toLowerCase()
  for (const sid of sidDirs) {
    const dir = join(binRoot, sid)
    let names: string[]
    try { names = await readdir(dir) } catch { continue } // other users' SIDs: access denied
    for (const name of names) {
      if (!name.startsWith("$I")) continue
      let buf: Buffer
      try { buf = await readFile(join(dir, name)) } catch { continue }
      const orig = parseRecycleOriginalPath(buf)
      if (orig?.replace(/\\/g, "/").toLowerCase() !== wanted) continue
      await mkdir(dirname(originalPath), { recursive: true })
      await rename(join(dir, `$R${name.slice(2)}`), originalPath)
      await rm(join(dir, name), { force: true })
      return true
    }
  }
  return false
}

// Recycle Bin $I record layout (bytes): version(8) + size(8) + deletionTime(8),
// then v2 adds a 4-byte char count before the UTF-16LE path; v1 has a fixed
// 260-wchar (520-byte) path field. https://en.wikipedia.org/wiki/Trash_(computing)
const RECYCLE_HEADER_LEN = 24
const RECYCLE_V2_PATH_OFFSET = 28 // header + 4-byte char count
const RECYCLE_V1_PATH_BYTES = 520 // 260 UTF-16 chars, null-padded
const UTF16_BYTES = 2
const RECYCLE_V2 = 2

/** Original path stored in a Recycle Bin `$I` record (Win10 v2 + legacy v1). */
export function parseRecycleOriginalPath(buf: Buffer): string | null {
  if (buf.length < RECYCLE_HEADER_LEN) return null
  const version = buf.readUInt32LE(0) // low half of the 8-byte header (1 or 2)
  if (version === RECYCLE_V2) {
    if (buf.length < RECYCLE_V2_PATH_OFFSET) return null
    const end = RECYCLE_V2_PATH_OFFSET + buf.readUInt32LE(RECYCLE_HEADER_LEN) * UTF16_BYTES
    if (buf.length < end) return null
    return buf.toString("utf16le", RECYCLE_V2_PATH_OFFSET, end - UTF16_BYTES) // drop trailing null
  }
  const raw = buf.toString("utf16le", RECYCLE_HEADER_LEN, RECYCLE_HEADER_LEN + RECYCLE_V1_PATH_BYTES)
  const nul = raw.indexOf(String.fromCharCode(0))
  return nul >= 0 ? raw.slice(0, nul) : raw
}

// ── Native OS file clipboard (CF_HDROP) — interop with Explorer ───────────
// Copy/cut in the tree puts the files on the Windows clipboard (with the
// "Preferred DropEffect": 2 = move/cut, 5 = copy), so a paste in Explorer works;
// paste in the tree reads whatever Explorer (or we) put there. Paths go via an
// env var (never interpolated into the script — see trashPath). Clipboard needs
// an STA thread, hence `-Sta`.


/** Rename / move. Single atomic call when src and dst share the same
 *  device; node falls back to copy+delete otherwise.
 *
 *  `overwrite` (default true = прежнее поведение node-rename для файлов):
 *  false — семантика vscode `workspace.fs.rename`: существующая цель = EEXIST
 *  (раньше опция extension-моста тихо ТЕРЯЛАСЬ — ревью API-фиделити). true —
 *  цель сносится заранее: node-rename поверх существующей ПАПКИ падает сам. */
export async function movePath(srcAbs: string, dstAbs: string, overwrite = true): Promise<void> {
  await mkdir(dirname(dstAbs), { recursive: true })
  const existing = await lstat(dstAbs).catch(() => null)
  if (existing) {
    if (!overwrite) {
      throw Object.assign(new Error(`EEXIST: target exists, rename '${srcAbs}' -> '${dstAbs}'`), {
        code: "EEXIST",
      })
    }
    await deletePath(dstAbs, true)
  }
  await rename(srcAbs, dstAbs)
}

/** Reveal a path in the OS file manager, selected (Windows Explorer). */
export async function revealInOS(absPath: string): Promise<void> {
  // explorer.exe exits non-zero even on success, so ignore the exit code.
  await execFileAsync("explorer.exe", [`/select,${absPath}`]).catch(() => { /* expected */ })
}

/** Open a URL / path in its associated default application (Windows). Path goes
 *  via an env var so it never lands in the script as code (see trashPath).
 *  Guarded: web/mail schemes and ordinary documents are fine, but an extension
 *  must not use env.openExternal to silently launch an executable. */
export async function openExternal(target: string): Promise<void> {
  const scheme = /^([a-z][a-z0-9+.-]*):/i.exec(target)?.[1]?.toLowerCase()
  if (scheme && !["http", "https", "mailto", "file"].includes(scheme)) {
    throw new Error(`openExternal: refused scheme "${scheme}"`)
  }
  if (!scheme || scheme === "file") {
    let p = target
    try { if (scheme === "file") p = fileURLToPath(target) } catch { throw new Error("openExternal: invalid file URL") }
    if (p.startsWith("\\\\")) throw new Error("openExternal: refused UNC path")
    const ext = extname(p).toLowerCase()
    if (OPEN_EXTERNAL_BLOCKED_EXT.has(ext)) throw new Error(`openExternal: refused executable "${ext}"`)
  }
  await execFileAsync("powershell.exe",
    ["-NoProfile", "-NonInteractive", "-Command", "Start-Process -FilePath $env:KAMIN_OPEN_PATH"],
    { windowsHide: true, env: { ...process.env, KAMIN_OPEN_PATH: target } })
}

/** Open the OS terminal AT a folder (Windows) — the fallback the renderer uses
 *  when the integrated Terminal tool isn't placed in any panel. Prefers Windows
 *  Terminal (`wt -d <dir>`), else a PowerShell window at that working dir. The
 *  dir is read from an env var and passed as a discrete ArgumentList element —
 *  never interpolated into the script — so it can't break out (see trashPath). */
export async function openTerminalAt(absPath: string): Promise<void> {
  const script =
    "$cwd = $env:KAMIN_TERM_CWD; " +
    "if (Get-Command wt.exe -ErrorAction SilentlyContinue) { Start-Process wt.exe -ArgumentList '-d', $cwd } " +
    "else { Start-Process powershell.exe -WorkingDirectory $cwd }"
  await execFileAsync("powershell.exe",
    ["-NoProfile", "-NonInteractive", "-Command", script],
    { windowsHide: true, env: { ...process.env, KAMIN_TERM_CWD: absPath } })
}

// ── Byte-level primitives for the VS Code `workspace.fs` API (Phase B).
// Distinct from readText/writeText (UTF-8 + BOM-strip + 5 MB guard for the
// editor): extensions expect raw bytes and no size guard.

interface FileStatInfo {
  type: "file" | "dir" | "symlink" | "other"
  size: number
  ctimeMs: number
  mtimeMs: number
}

/** Stat one path — shape feeds VS Code's `FileStat` (numeric fields). */
export async function statPath(absPath: string): Promise<FileStatInfo> {
  const s = await stat(absPath)
  return {
    type: s.isDirectory() ? "dir" : s.isFile() ? "file" : s.isSymbolicLink() ? "symlink" : "other",
    size: s.size,
    ctimeMs: s.ctimeMs,
    mtimeMs: s.mtimeMs,
  }
}

/** Raw bytes — for `workspace.fs.readFile` (returns Uint8Array). */
export async function readBytes(absPath: string): Promise<Uint8Array> {
  return new Uint8Array(await readFile(absPath))
}

/** Raw bytes write — atomic (tmp + rename), like writeText. */
export async function writeBytes(absPath: string, data: Uint8Array): Promise<void> {
  await mkdir(dirname(absPath), { recursive: true })
  const tmp = `${absPath}.tmp`
  await writeFile(tmp, data)
  await rename(tmp, absPath)
}

/** Create a directory (recursive) — `workspace.fs.createDirectory`. */
export async function makeDir(absPath: string): Promise<void> {
  await mkdir(absPath, { recursive: true })
}

/** True if a path exists — used by the renderer to pick a non-colliding
 *  paste target (e.g. "file copy.ts"). */
export async function pathExists(absPath: string): Promise<boolean> {
  try { await stat(absPath); return true } catch { return false }
}

/** Copy a file/dir — `workspace.fs.copy`. */
export async function copyPath(srcAbs: string, dstAbs: string, overwrite: boolean): Promise<void> {
  await mkdir(dirname(dstAbs), { recursive: true })
  await cp(srcAbs, dstAbs, { recursive: true, force: overwrite, errorOnExist: !overwrite })
}

export function joinPath(...segs: string[]): string {
  return normalize(segs.join(sep))
}

export const FS_LIMITS = { fileSizeMaxBytes: FILE_SIZE_GUARD_BYTES } as const

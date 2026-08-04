// Sideload support: install a .vsix archive (or an already-extracted extension
// folder) into the user extensions directory, where the loader discovers it
// alongside the built-ins. A .vsix is a zip whose payload lives under
// `extension/`; bsdtar (Windows System32 tar.exe, macOS tar) groks zip, so we
// shell out to it rather than pulling a JS zip dependency — same mechanism the
// compat-corpus harness uses.
import { execFile } from "node:child_process"
import { existsSync, mkdirSync, mkdtempSync, readFileSync, renameSync } from "node:fs"
import { readFile, rm, stat } from "node:fs/promises"
import { extname, join, resolve, sep } from "node:path"
import { promisify } from "node:util"

// ASYNC extraction. Extracting a .vsix (tar over a zip) + deleting a prior
// install were done with `spawnSync`/`rmSync` — blocking the WHOLE host event
// loop for the full extract/delete (arbitrarily long for a big extension), so
// the entire UI went "Not Responding" during a sideload install. execFile + the
// promise fs keep the loop responsive; the cheap one-shot fs ops (mkdir/mkdtemp/
// rename/stat/read) stay sync.
const execFileAsync = promisify(execFile)

const SYS_TAR = process.platform === "win32"
  ? `${process.env.SystemRoot ?? "C:\\Windows"}\\System32\\tar.exe`
  : "tar"

export interface InstalledExtension { dir: string; id: string }

/** `publisher.name` id (+ a filesystem-safe folder name) from a manifest dir. */
function readManifestId(extDir: string): string {
  const manifest = JSON.parse(readFileSync(join(extDir, "package.json"), "utf8")) as Record<string, unknown>
  const pub = typeof manifest.publisher === "string" ? manifest.publisher : "unknown"
  const name = typeof manifest.name === "string" ? manifest.name : "unnamed"
  return `${pub}.${name}`
}

function safeFolder(id: string): string {
  return id.replace(/[^a-zA-Z0-9._-]/g, "_")
}

/** Refuse to write/remove outside the user extensions dir. */
function assertInside(target: string, userExtDir: string): void {
  const abs = resolve(target)
  const root = resolve(userExtDir)
  if (abs !== root && !abs.startsWith(root + sep)) {
    throw new Error(`refusing to touch ${target} — outside the user extensions dir`)
  }
}

/** Extract a .vsix into `<userExtDir>/<id>`, overwriting any existing install of
 *  the same id. Returns the install dir + id. */
export async function installVsixArchive(vsixPath: string, userExtDir: string): Promise<InstalledExtension> {
  if (!existsSync(vsixPath)) throw new Error(`VSIX not found: ${vsixPath}`)
  mkdirSync(userExtDir, { recursive: true })
  const staging = mkdtempSync(join(userExtDir, ".staging-"))
  try {
    // Pull only the `extension/` subtree → <staging>/extension/.
    try {
      await execFileAsync(SYS_TAR, ["-xf", vsixPath, "-C", staging, "extension"])
    } catch (err) {
      const stderr = (err as { stderr?: string }).stderr
      throw new Error(`extract failed: ${(typeof stderr === "string" && stderr.trim()) || String(err)}`, { cause: err })
    }
    const extracted = join(staging, "extension")
    if (!existsSync(join(extracted, "package.json"))) throw new Error("VSIX has no extension/package.json")
    const id = readManifestId(extracted)
    const dir = join(userExtDir, safeFolder(id))
    assertInside(dir, userExtDir)
    await rm(dir, { recursive: true, force: true })
    renameSync(extracted, dir)
    return { dir, id }
  } finally {
    await rm(staging, { recursive: true, force: true })
  }
}

/** Remove an installed extension's folder (must live inside the user dir). */
export async function removeExtensionDir(dir: string, userExtDir: string): Promise<void> {
  assertInside(dir, userExtDir)
  await rm(resolve(dir), { recursive: true, force: true })
}

const ICON_MAX_BYTES = 524_288 // 512 KiB
const ICON_MIME: Record<string, string> = {
  ".png": "image/png", ".svg": "image/svg+xml", ".jpg": "image/jpeg",
  ".jpeg": "image/jpeg", ".gif": "image/gif", ".webp": "image/webp",
}

/** Read an extension's `contributes`-less `icon` (manifest.icon, relative to the
 *  extension dir) as a data URL for the Extensions list. Confined to the
 *  extension dir; null on miss / unsupported type / oversize. */
export async function readExtensionIcon(extDir: string, iconRel: string): Promise<string | null> {
  try {
    const p = resolve(extDir, iconRel)
    if (p !== resolve(extDir) && !p.startsWith(resolve(extDir) + sep)) return null
    const st = await stat(p)
    const mime = ICON_MIME[extname(p).toLowerCase()]
    if (!st.isFile() || st.size > ICON_MAX_BYTES || !mime) return null
    return `data:${mime};base64,${(await readFile(p)).toString("base64")}`
  } catch { return null }
}

// Sideload install/uninstall for the ExtensionLoader — kept out of loader.ts so
// that file stays under the per-file ceiling. Free functions over a small `ops`
// surface the loader provides (it owns the maps + prepare/unload/activation).
import { readFileSync } from "node:fs"
import { join } from "node:path"
import type { ExtensionDescriptor } from "../api/types.js"
import { manifestId, parseConfigDefaults } from "./loader-discovery.js"

export interface KnownEntry { manifest: Record<string, unknown>; path: string; builtin: boolean }

/** The loader internals install/uninstall need, exposed as a narrow capability. */
export interface SideloadOps {
  readonly loaded: { has(id: string): boolean; get(id: string): { descriptor: ExtensionDescriptor } | undefined }
  readonly known: Map<string, KnownEntry>
  registerDefaults(values: Record<string, unknown>): void
  prepare(manifest: Record<string, unknown>, path: string, builtin: boolean): void
  unload(id: string): void
  clearDisabled(id: string): void
  activate(): Promise<void>
}

/** Install (or reinstall) a sideloaded extension already extracted at `extDir`:
 *  register + prepare + activate it live, clearing any prior "disabled" mark. */
export async function installFromDir(ops: SideloadOps, extDir: string): Promise<ExtensionDescriptor> {
  const manifest = readManifest(extDir)
  if (!manifest) throw new Error(`no valid package.json in ${extDir}`)
  const id = manifestId(manifest)
  if (ops.loaded.has(id)) ops.unload(id) // reinstall over a loaded copy
  ops.clearDisabled(id)
  ops.known.set(id, { manifest, path: extDir, builtin: false })
  const defaults = parseConfigDefaults(manifest)
  if (Object.keys(defaults).length > 0) ops.registerDefaults(defaults)
  ops.prepare(manifest, extDir, false)
  await ops.activate()
  const descriptor = ops.loaded.get(id)?.descriptor
  if (!descriptor) throw new Error(`failed to prepare ${id}`)
  return descriptor
}

/** Unload a sideloaded extension + forget it; returns its dir to delete. Refuses
 *  built-ins. */
export function uninstallSideloaded(ops: SideloadOps, id: string): string {
  const entry = ops.known.get(id)
  if (!entry) throw new Error(`unknown extension ${id}`)
  if (entry.builtin) throw new Error(`cannot uninstall built-in extension ${id}`)
  ops.unload(id)
  ops.known.delete(id)
  ops.clearDisabled(id)
  return entry.path
}

function readManifest(extDir: string): Record<string, unknown> | undefined {
  try { return JSON.parse(readFileSync(join(extDir, "package.json"), "utf8")) as Record<string, unknown> }
  catch { return undefined } // missing/invalid package.json → not a loadable extension
}

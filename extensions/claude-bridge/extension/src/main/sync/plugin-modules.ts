// Hooks modules ("mods") of a plugin run inside the Claude CLI process, so the
// Bridge server must hold their files: unlike command hooks, MCP servers and
// monitors, the host cannot run them on the CLI's behalf.

import fs from 'fs'
import path from 'path'
import type { SyncPluginFile } from '../../shared/sync-types'

const MAX_FILE_BYTES = 512 * 1024
const MAX_TREE_BYTES = 4 * 1024 * 1024
const MAX_TREE_FILES = 2000

/** Paths the server builds itself or that would start a host-side component
 *  (MCP, LSP, monitors, event hooks) a second time inside the container. */
const SKIPPED_FILES = new Set([
  '.claude-plugin/plugin.json',
  '.mcp.json',
  '.lsp.json',
  'hooks/hooks.json',
  'monitors/monitors.json',
  'settings.json',
])
const SKIPPED_ROOT_DIRS = new Set(['.git', 'skills', 'agents', 'commands', 'workflows', 'output-styles', 'themes'])

export interface PluginModuleTree {
  /** Module paths relative to the proxy plugin's `hooks/` directory. */
  modules: string[]
  files: Record<string, SyncPluginFile>
}

function toPosix(value: string): string {
  return value.split(path.sep).join('/')
}

function relativeInside(root: string, candidate: string): string | null {
  const rel = path.relative(path.resolve(root), path.resolve(candidate))
  if (!rel || rel === '..' || rel.startsWith('..' + path.sep) || path.isAbsolute(rel)) return null
  return toPosix(rel)
}

async function declaredModules(pluginRoot: string, manifest: Record<string, unknown>): Promise<string[]> {
  const hookFiles = ['hooks/hooks.json']
  const spec = manifest.hooks
  for (const ref of typeof spec === 'string' ? [spec] : Array.isArray(spec) ? spec : []) {
    if (typeof ref === 'string') hookFiles.push(ref)
  }

  const modules: string[] = []
  for (const ref of hookFiles) {
    const hookFile = path.resolve(pluginRoot, ref)
    if (!relativeInside(pluginRoot, hookFile)) continue
    let parsed: unknown
    try { parsed = JSON.parse(await fs.promises.readFile(hookFile, 'utf-8')) } catch { continue }
    const declared = (parsed as { modules?: unknown } | null)?.modules
    if (!Array.isArray(declared)) continue
    for (const item of declared) {
      if (typeof item !== 'string') continue
      const rel = relativeInside(pluginRoot, path.resolve(path.dirname(hookFile), item))
      if (rel && !modules.includes(rel)) modules.push(rel)
    }
  }
  return modules
}

function encode(buffer: Buffer): SyncPluginFile {
  try {
    return { encoding: 'utf8', content: new TextDecoder('utf-8', { fatal: true }).decode(buffer) }
  } catch {
    return { encoding: 'base64', content: buffer.toString('base64') }
  }
}

/**
 * The plugin's file tree for a plugin that declares hooks modules, or
 * `undefined` when it declares none or the tree does not fit the limits. A
 * partial tree would load a module whose imports are missing, so an oversized
 * plugin is reported and left out as a whole.
 */
export async function collectPluginModuleTree(
  pluginRoot: string,
  manifest: Record<string, unknown>,
  report: (reason: string) => void = () => {},
): Promise<PluginModuleTree | undefined> {
  const declared = await declaredModules(pluginRoot, manifest)
  if (declared.length === 0) return undefined

  const files: Record<string, SyncPluginFile> = {}
  let total = 0
  let count = 0
  let refusal = ''

  async function walk(dir: string): Promise<void> {
    let entries: fs.Dirent[]
    try { entries = await fs.promises.readdir(dir, { withFileTypes: true }) } catch { return }
    entries.sort((a, b) => a.name.localeCompare(b.name))
    for (const entry of entries) {
      if (refusal) return
      const full = path.join(dir, entry.name)
      const rel = relativeInside(pluginRoot, full)
      if (!rel) continue
      if (entry.isDirectory()) {
        if (!SKIPPED_ROOT_DIRS.has(rel)) await walk(full)
        continue
      }
      if (!entry.isFile() || SKIPPED_FILES.has(rel)) continue
      const stat = await fs.promises.stat(full)
      if (stat.size > MAX_FILE_BYTES) { refusal = `${rel} is larger than ${MAX_FILE_BYTES} bytes`; return }
      total += stat.size
      if (++count > MAX_TREE_FILES || total > MAX_TREE_BYTES) {
        refusal = `the plugin tree exceeds ${MAX_TREE_FILES} files or ${MAX_TREE_BYTES} bytes`
        return
      }
      files[rel] = encode(await fs.promises.readFile(full))
    }
  }
  await walk(pluginRoot)

  if (refusal) {
    report(refusal)
    return undefined
  }
  const modules = declared.filter((rel) => files[rel] !== undefined)
  if (modules.length === 0) return undefined
  return {
    modules: modules.map((rel) => {
      const fromHooks = path.posix.relative('hooks', rel)
      return fromHooks.startsWith('.') ? fromHooks : `./${fromHooks}`
    }),
    files,
  }
}

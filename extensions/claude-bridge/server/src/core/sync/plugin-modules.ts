// Hooks modules ("mods") of a synced plugin. They run inside the Claude CLI
// process, so their files have to exist in the proxy plugin root. Everything
// else executable in a plugin stays on the client host.

import fsp from 'fs/promises'
import path from 'path'
import type { SyncPluginData } from '../types/pty'

/** Paths the server writes itself, or that would make the CLI start a
 *  host-side component (MCP, LSP, monitors, event hooks) inside the container. */
const RESERVED_FILES = new Set([
  '.claude-plugin/plugin.json',
  '.mcp.json',
  '.lsp.json',
  'hooks/hooks.json',
  'monitors/monitors.json',
  'settings.json',
])

function insideRoot(root: string, target: string): boolean {
  const relative = path.relative(root, target)
  return !!relative && relative !== '..' && !relative.startsWith('..' + path.sep) && !path.isAbsolute(relative)
}

/** `BRIDGE_PLUGIN_MODS=0` keeps client hooks modules out of the server's CLI. */
export function pluginModsEnabled(): boolean {
  return process.env.BRIDGE_PLUGIN_MODS !== '0'
}

/**
 * Write the plugin's file tree and a `hooks/hooks.json` that names only its
 * hooks modules. Returns the module paths that were written. Call it before
 * the component directories are written: those replace their own subtrees.
 */
export async function writePluginModuleTree(root: string, plugin: SyncPluginData): Promise<string[]> {
  if (!pluginModsEnabled() || !Array.isArray(plugin.modules) || !plugin.files) return []
  const base = path.resolve(root)

  const written = new Set<string>()
  for (const [relPath, file] of Object.entries(plugin.files)) {
    if (!relPath || relPath.includes('\\') || RESERVED_FILES.has(path.posix.normalize(relPath))) continue
    if (!file || typeof file.content !== 'string') continue
    if (file.encoding !== 'utf8' && file.encoding !== 'base64') continue
    const target = path.resolve(base, relPath)
    if (!insideRoot(base, target)) continue
    await fsp.mkdir(path.dirname(target), { recursive: true })
    await fsp.writeFile(target, Buffer.from(file.content, file.encoding))
    written.add(target)
  }

  const hooksDir = path.join(base, 'hooks')
  const modules = plugin.modules.filter(
    (ref): ref is string =>
      typeof ref === 'string' && !ref.includes('\\') && written.has(path.resolve(hooksDir, ref)),
  )
  if (modules.length === 0) return []
  await fsp.mkdir(hooksDir, { recursive: true })
  await fsp.writeFile(path.join(hooksDir, 'hooks.json'), JSON.stringify({ modules }, null, 2), 'utf-8')
  return modules
}

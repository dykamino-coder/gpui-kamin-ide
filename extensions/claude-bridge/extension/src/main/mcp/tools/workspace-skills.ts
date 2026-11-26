// Codex discovers text skills on the same host and in the same project as its Read tool.
// Only catalog metadata crosses this call. Opening the instructions is a normal Read.
import fs from 'node:fs/promises'
import path from 'node:path'
import os from 'node:os'
import YAML from 'yaml'
import type { ToolCallContext } from '../executor'
import { extractFrontmatter, listEnabledInstalledPlugins, parseYamlBool, pluginSkillRefs, readEffectivePluginManifest } from '../../plugin-helpers'

interface Entry {
  name: string
  source: string
  description: string
  file_path: string
}

function contained(root: string, target: string): boolean {
  const rel = path.relative(root, target)
  return !path.isAbsolute(rel) && rel !== '..' && !rel.startsWith('..' + path.sep)
}

/** A plugin may declare a single SKILL.md or nested skill directories. Bound
 * the traversal to its installation; do not follow junctions/symlink entries. */
async function skillFiles(root: string, refs: string[], recursive: boolean): Promise<string[]> {
  const realRoot = await fs.realpath(root).catch(() => null)
  if (!realRoot) return []
  const files = new Set<string>()
  const visited = new Set<string>()
  async function visit(location: string, depth: number): Promise<void> {
    if (visited.has(location)) return
    visited.add(location)
    const stat = await fs.lstat(location).catch(() => null)
    if (!stat || stat.isSymbolicLink()) return
    if (stat.isFile() && path.basename(location).toLowerCase() === 'skill.md') files.add(location)
    if (!stat.isDirectory() || (!recursive && depth > 1)) return
    const entries = await fs.readdir(location, { withFileTypes: true }).catch(() => [])
    for (const entry of entries) {
      if (entry.isDirectory() || (entry.isFile() && entry.name.toLowerCase() === 'skill.md')) {
        await visit(path.join(location, entry.name), depth + 1)
      }
    }
  }
  for (const ref of refs) {
    const location = path.resolve(root, ref)
    if (!contained(root, location)) continue
    const real = await fs.realpath(location).catch(() => null)
    if (real && contained(realRoot, real)) await visit(location, 0)
  }
  return [...files].sort()
}

async function describeSkill(file: string, source: string): Promise<Entry | null> {
  const stat = await fs.stat(file)
  if (stat.size > 100_000) return null
  const text = (await fs.readFile(file, 'utf8')).replace(/^\uFEFF/, '')
  const front = extractFrontmatter(text)
  if (front === null && /^---/.test(text)) return null
  const meta = (front ? YAML.parse(front) : {}) ?? {}
  if (typeof meta !== 'object' || Array.isArray(meta)) return null
  // These entries depend on the native engine for scoping, permission
  // restrictions, interpolation or execution. Keep that contract with Claude.
  const requiresNativeRuntime = ['hooks', 'allowed-tools', 'disallowed-tools', 'context', 'agent', 'paths', 'shell', 'model', 'effort']
  if (requiresNativeRuntime.some(k => k in meta) || /!`|^\s*```!/m.test(text)) return null
  const disable = meta['disable-model-invocation'] ?? meta.disableModelInvocation
  if (disable !== undefined && parseYamlBool(String(disable)) !== false) return null
  const name = typeof meta.name === 'string' && meta.name.trim() ? meta.name : path.basename(path.dirname(file))
  const description = typeof meta.description === 'string' ? meta.description : name
  return { name, source, description: description.slice(0, 600), file_path: file }
}

export async function listWorkspaceSkills(input: Record<string, unknown>, context?: ToolCallContext) {
  if (Object.keys(input).length) throw new Error('ListWorkspaceSkills takes no arguments')
  const cwd = context?.cwd
  const skills = new Map<string, Entry>()
  async function add(root: string, refs: string[], source: string, recursive: boolean): Promise<void> {
    for (const file of await skillFiles(root, refs, recursive)) {
      // A project entry shadows its user counterpart even when invocation is disabled.
      const key = (source.startsWith('plugin:') ? source + '/' : '') + path.basename(path.dirname(file))
      const row = await describeSkill(file, source).catch(() => null)
      if (row) skills.set(key, row)
      else skills.delete(key)
    }
  }
  await add(os.homedir(), ['.claude/skills'], 'user', false)
  if (cwd) await add(cwd, ['.claude/skills'], 'project', false)
  for (const plugin of await listEnabledInstalledPlugins({ cwd })) {
    const manifest = await readEffectivePluginManifest(plugin.installPath, plugin.name, plugin.marketplace)
    await add(plugin.installPath, pluginSkillRefs(plugin.installPath, manifest), 'plugin:' + plugin.key, true)
  }
  return { content: [{ type: 'text', text: JSON.stringify([...skills.values()]) }] }
}

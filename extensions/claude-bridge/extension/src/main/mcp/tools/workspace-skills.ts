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

// Bound time spent walking a marketplace package that declares "." as its
// skills directory. Explicitly named roots still work inside a dependency.
const MAX_DEPTH = 16
const MAX_ENTRIES = 16_384
interface ScanBudget { remaining: number; incomplete: boolean }

async function skillFiles(root: string, refs: string[], recursive: boolean, budget: ScanBudget): Promise<string[]> {
  const realRoot = await fs.realpath(root).catch(() => null)
  if (!realRoot) return []
  const files = new Set<string>()
  const visited = new Set<string>()
  async function visit(location: string, depth: number): Promise<void> {
    if (visited.has(location) || budget.remaining <= 0) return
    if (depth > MAX_DEPTH) { budget.incomplete = true; return }
    visited.add(location)
    const dir = await fs.opendir(location).catch(() => null)
    if (!dir) return
    for await (const entry of dir) {
      if (--budget.remaining <= 0) { budget.incomplete = true; break }
      const child = path.join(location, entry.name)
      if (entry.isFile() && entry.name.toLowerCase() === 'skill.md') files.add(child)
      if (entry.isDirectory() && (recursive || depth === 0) && entry.name !== '.git' && entry.name !== 'node_modules') {
        await visit(child, depth + 1)
      }
    }
  }
  for (const ref of refs) {
    if (--budget.remaining <= 0) { budget.incomplete = true; break }
    const location = path.resolve(root, ref)
    if (!contained(root, location)) continue
    const real = await fs.realpath(location).catch(() => null)
    if (!real || !contained(realRoot, real)) continue
    const stat = await fs.lstat(location).catch(() => null)
    if (stat?.isFile() && path.basename(location).toLowerCase() === 'skill.md') files.add(location)
    if (stat?.isDirectory()) await visit(location, 0)
  }
  return [...files].sort()
}

async function describeSkill(file: string, source: string): Promise<{ name: string; row?: Entry } | null> {
  const stat = await fs.stat(file)
  if (stat.size > 100_000) return null
  const text = (await fs.readFile(file, 'utf8')).replace(/^\uFEFF/, '')
  const front = extractFrontmatter(text)
  if (front === null && /^---/.test(text)) return null
  const meta = (front ? YAML.parse(front) : {}) ?? {}
  if (typeof meta !== 'object' || Array.isArray(meta)) return null
  // These entries depend on the native engine for scoping, permission
  // restrictions, interpolation or execution. Keep that contract with Claude.
  const name = typeof meta.name === 'string' && meta.name.trim() ? meta.name : path.basename(path.dirname(file))
  const requiresNativeRuntime = ['hooks', 'allowed-tools', 'disallowed-tools', 'context', 'agent', 'paths', 'shell', 'model', 'effort']
  if (requiresNativeRuntime.some(k => k in meta) || /!`|^\s*```!/m.test(text)) return { name }
  const disable = meta['disable-model-invocation'] ?? meta.disableModelInvocation
  if (disable !== undefined && parseYamlBool(String(disable)) !== false) return { name }
  const description = typeof meta.description === 'string' ? meta.description : name
  return { name, row: { name, source, description: description.slice(0, 600), file_path: file } }
}

export async function listWorkspaceSkills(input: Record<string, unknown>, context?: ToolCallContext) {
  if (Object.keys(input).length) throw new Error('ListWorkspaceSkills takes no arguments')
  const cwd = context?.cwd
  const skills = new Map<string, Entry>()
  const claimedDirectories = new Set<string>()
  const claimedNames = new Set<string>()
  const budget: ScanBudget = { remaining: MAX_ENTRIES, incomplete: false }
  async function add(root: string, refs: string[], source: string, recursive: boolean): Promise<void> {
    const namespace = source.startsWith('plugin:') ? source + '/' : ''
    for (const file of await skillFiles(root, refs, recursive, budget)) {
      const directory = namespace + path.basename(path.dirname(file))
      if (claimedDirectories.has(directory)) continue
      claimedDirectories.add(directory)
      const entry = await describeSkill(file, source).catch(() => null)
      if (!entry) continue
      const key = namespace + entry.name
      if (claimedNames.has(key)) continue
      claimedNames.add(key)
      if (entry.row) skills.set(key, entry.row)
    }
  }
  // Inspect project overrides first. If a project scan was truncated it may
  // contain an opt-out, so omit the lower-priority user catalog for this call.
  if (cwd) await add(cwd, ['.claude/skills'], 'project', false)
  if (!budget.incomplete) await add(os.homedir(), ['.claude/skills'], 'user', false)
  for (const plugin of await listEnabledInstalledPlugins({ cwd })) {
    if (budget.remaining <= 0) break
    const manifest = await readEffectivePluginManifest(plugin.installPath, plugin.name, plugin.marketplace)
    await add(plugin.installPath, pluginSkillRefs(plugin.installPath, manifest), 'plugin:' + plugin.key, true)
  }
  return { content: [{ type: 'text', text: JSON.stringify({ skills: [...skills.values()], incomplete: budget.incomplete }) }] }
}

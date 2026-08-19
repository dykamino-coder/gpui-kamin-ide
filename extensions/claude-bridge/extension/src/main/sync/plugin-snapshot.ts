import fs from 'fs'
import crypto from 'crypto'
import os from 'os'
import path from 'path'
import type { SyncPluginData } from '../../shared/sync-types'
import { listEnabledInstalledPlugins, readEffectivePluginManifest, substituteUserConfig } from '../plugin-helpers'
import { buildMonitorTriggerHooks } from '../plugin-monitors'

const MAX_TEXT_FILE_BYTES = 100_000

function isInside(root: string, candidate: string): boolean {
  const rel = path.relative(path.resolve(root), path.resolve(candidate))
  return rel === '' || (!rel.startsWith('..' + path.sep) && rel !== '..' && !path.isAbsolute(rel))
}

function safeResolve(root: string, ref: string): string | null {
  const resolved = path.isAbsolute(ref) ? path.resolve(ref) : path.resolve(root, ref)
  return isInside(root, resolved) ? resolved : null
}

function manifestPathRefs(value: unknown): string[] {
  if (typeof value === 'string') return [value]
  if (Array.isArray(value)) return value.filter((v): v is string => typeof v === 'string')
  return []
}

async function readText(filePath: string): Promise<string | undefined> {
  try {
    const stat = await fs.promises.stat(filePath)
    if (!stat.isFile() || stat.size > MAX_TEXT_FILE_BYTES) return undefined
    return await fs.promises.readFile(filePath, 'utf-8')
  } catch {
    return undefined
  }
}

async function walkFiles(root: string, accept: (filePath: string) => boolean): Promise<string[]> {
  const out: string[] = []
  async function walk(dir: string): Promise<void> {
    let entries: fs.Dirent[]
    try { entries = await fs.promises.readdir(dir, { withFileTypes: true }) } catch { return }
    await Promise.all(entries.map(async (entry) => {
      const full = path.join(dir, entry.name)
      if (entry.isDirectory()) await walk(full)
      else if (entry.isFile() && accept(full)) out.push(full)
    }))
  }
  await walk(root)
  return out.sort()
}

function annotateWithHostPath(content: string, absHostPath: string): string {
  const marker = '<!-- bridge-sync -->'
  if (content.includes(marker)) return content
  return `${content}\n\n---\n${marker}\n**Bridge sync note:** This plugin file is loaded by the remote Claude CLI, but its original files live on the user's machine at:\n\n\`\`\`\n${absHostPath}\n\`\`\`\n\nResolve sibling scripts, references, and assets from that original host path. File tools and Bash execute on the user's machine.\n`
}

export function mergeNonSensitivePluginOptions(
  manifest: Record<string, any>,
  savedOptions: unknown,
): Record<string, unknown> {
  const out: Record<string, unknown> = {}
  for (const [key, spec] of Object.entries(manifest?.userConfig ?? {})) {
    if (spec && typeof spec === 'object' && (spec as any).sensitive !== true && 'default' in spec) {
      out[key] = (spec as { default: unknown }).default
    }
  }
  if (savedOptions && typeof savedOptions === 'object' && !Array.isArray(savedOptions)) {
    for (const [key, value] of Object.entries(savedOptions)) {
      const spec = manifest?.userConfig?.[key]
      // Old Claude builds could leave a plaintext shadow of a sensitive
      // option in settings.json. Never materialize that value into remote
      // skill/agent/command content; only declared non-sensitive options are
      // eligible for content substitution.
      if (spec && typeof spec === 'object' && spec.sensitive !== true) out[key] = value
    }
  }
  return out
}

function loadNonSensitiveOptions(pluginId: string, manifest: Record<string, any>): Record<string, unknown> {
  let savedOptions: unknown = null
  try {
    const settings = JSON.parse(fs.readFileSync(path.join(os.homedir(), '.claude', 'settings.json'), 'utf-8'))
    savedOptions = settings?.pluginConfigs?.[pluginId]?.options
  } catch { /* no saved values */ }
  return mergeNonSensitivePluginOptions(manifest, savedOptions)
}

function substituteContent(
  content: string,
  sourcePath: string,
  pluginRoot: string,
  options: Record<string, unknown>,
): string {
  let out = substituteUserConfig(content, options)
  out = out.replace(/\$\{CLAUDE_PLUGIN_ROOT\}/g, pluginRoot)
  return annotateWithHostPath(out, sourcePath)
}

type ComponentKind = 'skills' | 'agents' | 'commands' | 'workflows' | 'outputStyles' | 'themes'

async function collectComponentFiles(
  pluginRoot: string,
  refs: string[],
  kind: ComponentKind,
  options: Record<string, unknown>,
): Promise<Record<string, string>> {
  const out: Record<string, string> = {}
  const sourceSeen = new Set<string>()

  const accept = (filePath: string): boolean => {
    const lower = path.basename(filePath).toLowerCase()
    if (kind === 'skills') return lower === 'skill.md'
    if (kind === 'themes') return lower.endsWith('.json')
    if (kind === 'workflows') return /\.(?:c|m)?js$/.test(lower)
    return lower.endsWith('.md')
  }

  for (const ref of refs) {
    const source = safeResolve(pluginRoot, ref)
    if (!source) continue
    let stat: fs.Stats
    try { stat = await fs.promises.stat(source) } catch { continue }
    const files = stat.isDirectory()
      ? await walkFiles(source, accept)
      : (stat.isFile() && accept(source) ? [source] : [])

    for (const filePath of files) {
      const canonical = path.resolve(filePath)
      if (sourceSeen.has(canonical)) continue
      sourceSeen.add(canonical)

      let rel: string
      if (stat.isDirectory()) {
        rel = path.relative(source, filePath)
        // A custom skill path may point directly at a directory containing a
        // root SKILL.md. Give it a stable skill directory in the proxy plugin.
        if (kind === 'skills' && !rel.includes(path.sep)) {
          rel = path.join(path.basename(source), rel)
        }
      } else if (kind === 'skills') {
        rel = path.join(path.basename(path.dirname(filePath)), path.basename(filePath))
      } else {
        rel = path.basename(filePath)
      }
      rel = rel.split(path.sep).join('/')
      if (!rel || rel.startsWith('../')) continue
      if (out[rel] !== undefined) {
        rel = `${path.basename(path.dirname(filePath))}--${rel}`
      }

      const raw = await readText(filePath)
      if (raw === undefined) continue
      out[rel] = kind === 'themes' || kind === 'workflows'
        ? raw
        : substituteContent(raw, filePath, pluginRoot, options)
    }
  }
  return out
}

type HookSettings = Record<string, unknown[]>

function mergeHooks(target: HookSettings, source: unknown): void {
  if (!source || typeof source !== 'object' || Array.isArray(source)) return
  const obj = source as Record<string, unknown>
  const hooks = obj.hooks && typeof obj.hooks === 'object' && !Array.isArray(obj.hooks)
    ? obj.hooks as Record<string, unknown>
    : obj
  for (const [event, matchers] of Object.entries(hooks)) {
    if (!Array.isArray(matchers)) continue
    target[event] = [...(target[event] ?? []), ...matchers]
  }
}

async function readHookFile(filePath: string): Promise<unknown> {
  const raw = await readText(filePath)
  if (raw === undefined) return null
  try { return JSON.parse(raw) } catch { return null }
}

export async function collectPluginHooks(pluginRoot: string, manifest: Record<string, unknown>): Promise<HookSettings> {
  const out: HookSettings = {}
  const seen = new Set<string>()
  const addPath = async (ref: string): Promise<void> => {
    const filePath = safeResolve(pluginRoot, ref)
    if (!filePath || seen.has(filePath)) return
    seen.add(filePath)
    mergeHooks(out, await readHookFile(filePath))
  }

  // Canonical current path plus legacy paths supported by older bridge builds.
  await addPath('hooks/hooks.json')
  await addPath('hooks.json')
  await addPath('.claude-plugin/hooks.json')

  const spec = manifest.hooks
  if (typeof spec === 'string') await addPath(spec)
  else if (Array.isArray(spec)) {
    for (const item of spec) {
      if (typeof item === 'string') await addPath(item)
      else mergeHooks(out, item)
    }
  } else {
    mergeHooks(out, spec)
  }
  return out
}

export function hashPluginHook(event: string, matcher: string | undefined, handler: unknown): string {
  return crypto.createHash('sha256')
    .update(JSON.stringify({ event, matcher, handler }))
    .digest('hex')
    .slice(0, 16)
}

function filterApprovedHooks(pluginId: string, hooks: HookSettings): HookSettings {
  let approved: Set<string> | null = null
  try {
    const file = path.join(os.homedir(), '.claude', 'open-claude-bridge', 'plugin-hook-approvals.json')
    const store = JSON.parse(fs.readFileSync(file, 'utf-8')) as Record<string, string[]>
    if (Object.prototype.hasOwnProperty.call(store, pluginId)) approved = new Set(store[pluginId] ?? [])
  } catch { /* no approval store */ }
  if (!approved) return {}

  const out: HookSettings = {}
  for (const [event, rawMatchers] of Object.entries(hooks)) {
    if (!Array.isArray(rawMatchers)) continue
    const kept: unknown[] = []
    for (const rawMatcher of rawMatchers) {
      if (!rawMatcher || typeof rawMatcher !== 'object') continue
      const matcher = rawMatcher as { matcher?: string; hooks?: unknown[] }
      const handlers = (matcher.hooks ?? []).filter(handler => approved!.has(hashPluginHook(event, matcher.matcher, handler)))
      if (handlers.length > 0) kept.push({ ...matcher, hooks: handlers })
    }
    if (kept.length > 0) out[event] = kept
  }
  return out
}

export function rewritePluginMcpMatchers(pluginName: string, hooks: HookSettings): HookSettings {
  const pluginPart = pluginName.replace(/[^a-zA-Z0-9_-]/g, '_')
  const officialPrefix = `mcp__plugin_${pluginPart}_`
  const bridgePrefix = `mcp__user-tools__plugin_${pluginPart}_`
  // Claude's built-in file/shell tools are disabled in bridge sessions and
  // exposed through the user-tools MCP server. Plugin hooks are authored
  // against the native names, so translate standalone matcher tokens too.
  const bridgeNativeTools = [
    'Read', 'Write', 'Edit', 'Glob', 'Grep', 'Bash', 'PowerShell',
    'BashOutput', 'KillShell', 'NotebookEdit', 'WebFetch',
    'EnterWorktree', 'ExitWorktree', 'AskUserQuestion',
    'PushNotification', 'ShowWidget', 'TodoList', 'TodoWrite',
    'LspDiagnostics', 'LspHover', 'LspDefinition', 'LspReferences',
  ]
  const bridgeNativeMatcher = new RegExp(`\\b(${bridgeNativeTools.join('|')})\\b`, 'g')
  const out: HookSettings = {}
  for (const [event, rawMatchers] of Object.entries(hooks)) {
    if (!Array.isArray(rawMatchers)) continue
    out[event] = rawMatchers.map((raw) => {
      if (!raw || typeof raw !== 'object') return raw
      const matcher = raw as { matcher?: unknown; hooks?: unknown[] }
      const rewrittenHandlers = Array.isArray(matcher.hooks)
        ? matcher.hooks.map((handler) => {
          if (!handler || typeof handler !== 'object') return handler
          const h = handler as Record<string, unknown>
          if (h.type !== 'mcp_tool' || typeof h.server !== 'string' || typeof h.tool !== 'string') return h
          const rawServer = h.server.trim()
          const officialServerPrefix = `plugin:${pluginName}:`
          const serverKey = rawServer.startsWith(officialServerPrefix)
            ? rawServer.slice(officialServerPrefix.length)
            : rawServer
          const serverPart = serverKey.replace(/[^a-zA-Z0-9_-]/g, '_') || 'server'
          return { ...h, server: 'user-tools', tool: `plugin_${pluginPart}_${serverPart}__${h.tool}` }
        })
        : matcher.hooks
      return {
        ...matcher,
        ...(typeof matcher.matcher === 'string' ? {
          matcher: matcher.matcher
            .split(officialPrefix).join(bridgePrefix)
            .replace(bridgeNativeMatcher, 'mcp__user-tools__$1'),
        } : {}),
        ...(rewrittenHandlers ? { hooks: rewrittenHandlers } : {}),
      }
    })
  }
  return out
}

function proxyManifest(manifest: Record<string, unknown>, fallbackName: string): Record<string, unknown> {
  const allowed = ['name', 'displayName', 'version', 'description', 'author', 'homepage', 'repository', 'license', 'keywords']
  const out: Record<string, unknown> = { name: typeof manifest.name === 'string' ? manifest.name : fallbackName }
  for (const key of allowed) {
    if (key in manifest && manifest[key] !== undefined) out[key] = manifest[key]
  }
  // Executable declarations are intentionally omitted. The bridge runs them
  // on the user's host; loading them natively would execute inside the Linux
  // container against paths that do not exist there.
  return out
}

export async function buildPluginSnapshots(): Promise<Record<string, SyncPluginData>> {
  const plugins = await listEnabledInstalledPlugins()
  const result: Record<string, SyncPluginData> = {}

  await Promise.all(plugins.map(async (plugin) => {
    const manifest = await readEffectivePluginManifest(plugin.installPath, plugin.name, plugin.marketplace)
    const options = loadNonSensitiveOptions(plugin.key, manifest)

    const defaultSkills = path.join(plugin.installPath, 'skills')
    const marketplaceRootSource = fs.existsSync(path.join(plugin.installPath, '.claude-plugin', 'marketplace.json'))
    const skillsRefs = [
      ...(fs.existsSync(defaultSkills) && !(marketplaceRootSource && manifest.skills !== undefined) ? ['skills'] : []),
      ...manifestPathRefs(manifest.skills),
    ]
    if (skillsRefs.length === 0 && fs.existsSync(path.join(plugin.installPath, 'SKILL.md'))) {
      skillsRefs.push('SKILL.md')
    }

    // Per Claude's merge contract commands/agents/outputStyles/themes replace
    // their default directory when explicitly declared; skills add to default.
    const commandsRefs = manifest.commands !== undefined ? manifestPathRefs(manifest.commands) : ['commands']
    const agentsRefs = manifest.agents !== undefined ? manifestPathRefs(manifest.agents) : ['agents']
    const outputStyleRefs = manifest.outputStyles !== undefined ? manifestPathRefs(manifest.outputStyles) : ['output-styles']
    const experimental = manifest.experimental && typeof manifest.experimental === 'object' && !Array.isArray(manifest.experimental)
      ? manifest.experimental as Record<string, unknown>
      : {}
    const themeSpec = experimental.themes ?? manifest.themes
    const themeRefs = themeSpec !== undefined ? manifestPathRefs(themeSpec) : ['themes']

    const workflowRefs = manifest.workflows !== undefined ? manifestPathRefs(manifest.workflows) : ['workflows']

    const [skills, agents, commands, workflows, outputStyles, themes, hooks, monitorHooks, settings] = await Promise.all([
      collectComponentFiles(plugin.installPath, skillsRefs, 'skills', options),
      collectComponentFiles(plugin.installPath, agentsRefs, 'agents', options),
      collectComponentFiles(plugin.installPath, commandsRefs, 'commands', options),
      collectComponentFiles(plugin.installPath, workflowRefs, 'workflows', options),
      collectComponentFiles(plugin.installPath, outputStyleRefs, 'outputStyles', options),
      collectComponentFiles(plugin.installPath, themeRefs, 'themes', options),
      collectPluginHooks(plugin.installPath, manifest),
      buildMonitorTriggerHooks(plugin.installPath, manifest, plugin.key),
      readText(path.join(plugin.installPath, 'settings.json')),
    ])

    const pluginName = plugin.name
    const approvedHooks = rewritePluginMcpMatchers(pluginName, filterApprovedHooks(plugin.key, hooks))
    for (const [event, matchers] of Object.entries(monitorHooks)) {
      approvedHooks[event] = [...(approvedHooks[event] ?? []), ...matchers]
    }
    result[plugin.key] = {
      id: plugin.key,
      name: pluginName,
      marketplace: plugin.marketplace,
      sourceRoot: plugin.installPath,
      manifest: proxyManifest(manifest, plugin.name),
      skills,
      agents,
      commands,
      workflows,
      outputStyles,
      themes,
      hooks: approvedHooks,
      settings,
    }
  }))

  return result
}

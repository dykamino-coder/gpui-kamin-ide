// Merge hooks from all 3 layers (per-CLI conventions):
//   1. user-global   — ~/.claude/settings.json:hooks
//   2. project       — <userCwd>/.claude/settings.json:hooks
//   3. plugins       — ~/.claude/plugins/installed_plugins.json → walk each
//                      enabled plugin's .claude-plugin/plugin.json:hooks +
//                      <plugin>/hooks.json
//
// Bridge-side merge order matches CLI's: user → project → plugins. Later
// matchers append after earlier ones (same as CLI). Result is a combined
// HookSettings ready to pass through proxy-rewriter.

import fs from 'fs'
import os from 'os'
import path from 'path'
import type { HookSettings, HookEvent, HookMatcher, HookSource } from './types'

interface MergedHooks {
  hooks: HookSettings
  /** Per-handler source map — used by registry to label each entry. */
  sourceByMatcher: Map<HookMatcher, HookSource>
}

/** Append matchers from `src` into `acc` keeping order. */
function pushAll(acc: HookSettings, src: HookSettings | undefined): void {
  if (!src) return
  for (const [event, matchers] of Object.entries(src)) {
    if (!Array.isArray(matchers)) continue
    const e = event as HookEvent
    if (!acc[e]) acc[e] = []
    for (const m of matchers) {
      if (m && Array.isArray(m.hooks)) acc[e]!.push(m)
    }
  }
}

/** A malformed tool policy must not turn into an empty list of guards. */
function validateToolHooks(hooks: HookSettings | undefined): void {
  if (hooks === undefined) return
  if (!hooks || typeof hooks !== 'object' || Array.isArray(hooks)) throw new Error('Invalid hook settings')
  for (const event of ['PreToolUse', 'PostToolUse', 'PostToolUseFailure'] as const) {
    const groups = hooks[event]
    if (groups === undefined) continue
    if (!Array.isArray(groups)) throw new Error('Invalid hook groups')
    for (const group of groups) {
      if (!group || !Array.isArray(group.hooks)) throw new Error('Invalid hook group')
      if (group.matcher !== undefined) {
        if (typeof group.matcher !== 'string') throw new Error('Invalid hook matcher')
        if (group.matcher !== '*') new RegExp(group.matcher)
      }
      for (const handler of group.hooks) {
        if (
          !handler ||
          typeof handler.type !== 'string' ||
          (handler.if !== undefined && typeof handler.if !== 'string')
        )
          throw new Error('Invalid hook handler')
        if (handler.type === 'command' && (typeof handler.command !== 'string' || !handler.command.trim()))
          throw new Error('Invalid hook command')
      }
    }
  }
}

function readJsonHooks(filePath: string, strict = false): HookSettings | undefined {
  try {
    if (!fs.existsSync(filePath)) return undefined
    const raw = fs.readFileSync(filePath, 'utf-8')
    const parsed = JSON.parse(raw) as { hooks?: HookSettings }
    if (strict && (!parsed || typeof parsed !== 'object' || Array.isArray(parsed))) throw new Error()
    if (strict) validateToolHooks(parsed.hooks)
    return parsed.hooks
  } catch {
    if (strict) throw new Error('Cannot load the workspace hook settings')
    return undefined
  }
}

function hooksDisabled(filePath: string): boolean {
  try {
    return fs.existsSync(filePath) && JSON.parse(fs.readFileSync(filePath, 'utf-8'))?.disableAllHooks === true
  } catch {
    return false
  }
}

interface InstalledPlugin {
  name: string
  marketplace: string
  installPath: string
  enabled: boolean
}

function loadEnabledPlugins(): InstalledPlugin[] {
  try {
    const installedFile = path.join(os.homedir(), '.claude', 'plugins', 'installed_plugins.json')
    if (!fs.existsSync(installedFile)) return []
    const raw = fs.readFileSync(installedFile, 'utf-8')
    const parsed = JSON.parse(raw) as {
      plugins?: Record<string, Array<{ installPath?: string }>>
    }
    const settingsFile = path.join(os.homedir(), '.claude', 'settings.json')
    let enabledMap: Record<string, boolean> = {}
    try {
      if (fs.existsSync(settingsFile)) {
        const s = JSON.parse(fs.readFileSync(settingsFile, 'utf-8')) as { enabledPlugins?: Record<string, boolean> }
        enabledMap = s.enabledPlugins ?? {}
      }
    } catch {
      /* ignore */
    }

    const out: InstalledPlugin[] = []
    for (const [key, entries] of Object.entries(parsed.plugins ?? {})) {
      if (enabledMap[key] === false) continue
      const entry = Array.isArray(entries) ? entries[0] : null
      if (!entry?.installPath) continue
      const at = key.lastIndexOf('@')
      const name = at > 0 ? key.slice(0, at) : key
      const marketplace = at > 0 ? key.slice(at + 1) : 'unknown'
      out.push({
        name: name ?? key,
        marketplace: marketplace ?? 'unknown',
        installPath: entry.installPath,
        enabled: true,
      })
    }
    return out
  } catch {
    return []
  }
}

/** Read hooks declared in a plugin's manifest. Convention: either
 *  `<plugin>/.claude-plugin/plugin.json:hooks` or `<plugin>/hooks.json:hooks`. */
function readPluginHooks(plugin: InstalledPlugin): HookSettings | undefined {
  const candidates = [
    path.join(plugin.installPath, '.claude-plugin', 'plugin.json'),
    path.join(plugin.installPath, 'hooks.json'),
    path.join(plugin.installPath, '.claude-plugin', 'hooks.json'),
    path.join(plugin.installPath, 'hooks', 'hooks.json'),
  ]
  for (const file of candidates) {
    const hooks = readJsonHooks(file)
    if (hooks && Object.keys(hooks).length > 0) return hooks
  }
  return undefined
}

interface SyncedPluginHooks {
  id: string
  sourceRoot: string
  hooks: HookSettings
}

function loadSyncedPluginHooks(filePath: string | undefined, strict = false): SyncedPluginHooks[] {
  if (!filePath || !fs.existsSync(filePath)) return []
  try {
    const parsed = JSON.parse(fs.readFileSync(filePath, 'utf-8')) as Array<{
      id?: string
      sourceRoot?: string
      hooks?: HookSettings
    }>
    if (!Array.isArray(parsed)) throw new Error()
    return parsed.flatMap((entry) => {
      if (!entry?.id || !entry.sourceRoot || !entry.hooks || typeof entry.hooks !== 'object') {
        if (strict) throw new Error()
        return []
      }
      if (strict) validateToolHooks(entry.hooks)
      return [{ id: entry.id, sourceRoot: entry.sourceRoot, hooks: entry.hooks }]
    })
  } catch {
    if (strict) throw new Error('Cannot load the workspace plugin hook snapshot')
    return []
  }
}

/** Collect hooks from all 3 layers + return source-map so registry can
 *  label each matcher with its origin (user / project / plugin).
 *
 *  `syncedSettingsFiles` — absolute paths to per-token SYNCED settings.json
 *  copies (user + project, uploaded by the client's sync). The server runs in
 *  a container: `os.homedir()` here is the container home and `userCwd` is a
 *  path on the USER'S machine that doesn't exist on this filesystem, so
 *  without these the merge input was always empty and the generated
 *  session settings never contained a `hooks` key at all. */
export function mergeAllHooks(
  userCwd?: string,
  syncedSettingsFiles?: string[],
  syncedPluginsFile?: string,
): MergedHooks {
  const merged: HookSettings = {}
  const sourceByMatcher = new Map<HookMatcher, HookSource>()

  const effectiveSettingsFiles = [path.join(os.homedir(), '.claude', 'settings.json'), ...(syncedSettingsFiles ?? [])]
  if (effectiveSettingsFiles.some(hooksDisabled)) return { hooks: merged, sourceByMatcher }

  // ── 1. user-global ─────────────────────────────────────────
  const userHooks = readJsonHooks(path.join(os.homedir(), '.claude', 'settings.json'))
  if (userHooks) {
    pushAll(merged, userHooks)
    for (const matchers of Object.values(userHooks)) {
      if (Array.isArray(matchers)) for (const m of matchers) sourceByMatcher.set(m, { kind: 'user' })
    }
  }

  appendClientSettings(merged, sourceByMatcher, userCwd, syncedSettingsFiles)

  // ── 2. project (<cwd>/.claude/settings.json) ───────────────
  if (userCwd) {
    const projectFile = path.join(userCwd, '.claude', 'settings.json')
    const projectHooks = readJsonHooks(projectFile)
    if (projectHooks) {
      pushAll(merged, projectHooks)
      for (const matchers of Object.values(projectHooks)) {
        if (Array.isArray(matchers))
          for (const m of matchers) {
            sourceByMatcher.set(m, { kind: 'project', projectPath: userCwd })
          }
      }
    }
  }

  // ── 3. plugins ────────────────────────────────────────────
  for (const plugin of loadEnabledPlugins()) {
    const pluginHooks = readPluginHooks(plugin)
    if (!pluginHooks) continue
    pushAll(merged, pluginHooks)
    const manifestPath = path.join(plugin.installPath, '.claude-plugin', 'plugin.json')
    const pluginId = `${plugin.name}@${plugin.marketplace}`
    for (const matchers of Object.values(pluginHooks)) {
      if (Array.isArray(matchers))
        for (const m of matchers) {
          sourceByMatcher.set(m, { kind: 'plugin', pluginId, manifestPath })
        }
    }
  }

  appendClientPlugins(merged, sourceByMatcher, syncedPluginsFile)

  return { hooks: merged, sourceByMatcher }
}

function appendClientSettings(
  merged: HookSettings,
  sourceByMatcher: Map<HookMatcher, HookSource>,
  userCwd?: string,
  syncedSettingsFiles?: string[],
  strict = false,
): void {
  // ── 1b. synced per-token settings (the user's REAL hooks) ──
  for (const [index, file] of (syncedSettingsFiles ?? []).entries()) {
    const hooks = readJsonHooks(file, strict)
    if (!hooks) continue
    pushAll(merged, hooks)
    const source: HookSource =
      index === 0 || !userCwd ? { kind: 'user', client: true } : { kind: 'project', projectPath: userCwd, client: true }
    for (const matchers of Object.values(hooks)) {
      if (Array.isArray(matchers)) for (const m of matchers) sourceByMatcher.set(m, source)
    }
  }
}

function appendClientPlugins(
  merged: HookSettings,
  sourceByMatcher: Map<HookMatcher, HookSource>,
  syncedPluginsFile?: string,
  strict = false,
): void {
  // The bridge container cannot see the client host's plugin cache. The VSIX
  // host therefore uploads approved hook metadata alongside each proxy-plugin
  // snapshot; commands still execute on the client host via the HTTP/WS proxy.
  for (const plugin of loadSyncedPluginHooks(syncedPluginsFile, strict)) {
    pushAll(merged, plugin.hooks)
    for (const matchers of Object.values(plugin.hooks)) {
      if (Array.isArray(matchers))
        for (const m of matchers) {
          sourceByMatcher.set(m, {
            kind: 'plugin',
            pluginId: plugin.id,
            manifestPath: path.join(plugin.sourceRoot, '.claude-plugin', 'plugin.json'),
          })
        }
    }
  }
}

/** Only inputs published by this client identity. A path supplied by a remote
 * project never selects a settings file on the Bridge container. */
export function mergeClientHooks(userCwd: string | undefined, settings: string[], plugins: string): MergedHooks {
  const hooks: HookSettings = {}
  const sourceByMatcher = new Map<HookMatcher, HookSource>()
  if (!settings[0] || !hooksDisabled(settings[0])) {
    // Project preferences may disable their own declarations only.
    const activeSettings = settings.filter((file, index) => index === 0 || !hooksDisabled(file))
    appendClientSettings(hooks, sourceByMatcher, userCwd, activeSettings, true)
    appendClientPlugins(hooks, sourceByMatcher, plugins, true)
  }
  return { hooks, sourceByMatcher }
}

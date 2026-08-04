// ============================================================================
// Sync Client — uploads user-level and project-level files to bridge server
// ============================================================================

import fs from 'fs'
import path from 'path'
import os from 'os'
import crypto from 'crypto'
import type { SyncUserData, SyncProjectData } from '../../shared/sync-types'
import { loadEnabledPluginsMap } from '../plugin-helpers'

// ---------------------------------------------------------------------------
// Debounce state
// ---------------------------------------------------------------------------

const SYNC_DEBOUNCE_MS = 60_000  // Don't re-sync if synced < 60s ago
let lastUserSync = 0
const lastProjectSync = new Map<string, number>()

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/** First 16 chars of sha256(token) — must match server's tokenHash() */
function tokenHash(token: string): string {
  return crypto.createHash('sha256').update(token).digest('hex').slice(0, 16)
}

/** Ensure URL uses http(s) protocol (strip ws:// prefix if present) */
function toHttpUrl(serverUrl: string): string {
  return serverUrl.replace(/^ws(s?):\/\//, 'http$1://')
}

/**
 * Append a "lives on the client" note at the end of a synced entrypoint file.
 * The server-side Claude CLI sees this hint and (via Read tool) can ask the
 * client for sibling files at the original absolute path — which the client
 * reads from its own filesystem. Sibling assets are deliberately NOT uploaded.
 */
function annotateWithClientPath(content: string, absClientPath: string): string {
  const marker = '<!-- bridge-sync -->'
  if (content.includes(marker)) return content  // already annotated
  const footer = [
    '',
    '',
    '---',
    marker,
    `**Bridge sync note:** This file was synced from the user's machine. The original path is:`,
    '',
    '```',
    absClientPath,
    '```',
    '',
    `Any sibling files (scripts, references, assets) referenced from here live next to that file. To read them, call the Read tool with the absolute client-side path — e.g. \`${path.dirname(absClientPath)}/<filename>\`. Do NOT try to resolve relative paths against the session working directory; they will not exist on the server.`,
    '',
  ].join('\n')
  return content + footer
}

/**
 * Read the "entrypoint" markdown files of a skills/agents/commands directory —
 * NOT everything. Sibling assets stay on the client and are read lazily via
 * the Read tool, using the absolute path embedded in the entrypoint (see
 * `annotateWithClientPath`). Rationale: Read runs on the client, so if we
 * uploaded those siblings to the server, the CLI would ask the client for a
 * server-side path that doesn't exist on the client's filesystem.
 *
 * Layout handled:
 *   skills/<name>/SKILL.md           (per-skill subdirs)
 *   agents/<name>.md                  (flat .md) + agents/<name>/agent.md
 *   commands/<name>.md                (flat .md) + commands/<subdir>/<name>.md
 */
async function readEntrypointMd(
  baseDir: string,
  kind: 'skills' | 'agents' | 'commands',
): Promise<Record<string, string>> {
  const result: Record<string, string> = {}
  let entries: fs.Dirent[]
  try { entries = await fs.promises.readdir(baseDir, { withFileTypes: true }) }
  catch { return result }

  await Promise.all(entries.map(async (entry) => {
    const full = path.join(baseDir, entry.name)
    if (entry.isDirectory()) {
      // Per-skill dir: SKILL.md is canonical entrypoint. Also accept lower
      // case skill.md / agent.md / command.md to be forgiving.
      const candidates = kind === 'skills'
        ? ['SKILL.md', 'skill.md']
        : kind === 'agents'
          ? ['AGENT.md', 'agent.md', `${entry.name}.md`]
          : ['COMMAND.md', 'command.md', `${entry.name}.md`]
      for (const cand of candidates) {
        const mdPath = path.join(full, cand)
        try {
          const stat = await fs.promises.stat(mdPath)
          if (stat.size > 100_000) break
          const raw = await fs.promises.readFile(mdPath, 'utf-8')
          result[`${entry.name}/${cand}`] = annotateWithClientPath(raw, mdPath)
          break
        } catch { /* try next candidate */ }
      }
      return
    }
    if (entry.isFile() && entry.name.toLowerCase().endsWith('.md')) {
      try {
        const stat = await fs.promises.stat(full)
        if (stat.size > 100_000) return
        const raw = await fs.promises.readFile(full, 'utf-8')
        result[entry.name] = annotateWithClientPath(raw, full)
      } catch { /* skip */ }
    }
  }))
  return result
}

/** Safely read a text file, return undefined if missing/error */
async function safeReadFile(filePath: string): Promise<string | undefined> {
  try { return await fs.promises.readFile(filePath, 'utf-8') }
  catch { return undefined }
}

/**
 * Enumerate user-installed plugins and collect their `skills/agents/commands`
 * entrypoint .md files, keyed as `plugin:<name@marketplace>/<filename>` to
 * avoid collisions with user-level items. Honors `~/.claude/settings.json:
 * enabledPlugins` — disabled plugins are skipped so their slash commands and
 * skills don't surface on the server either.
 */
async function readPluginEntrypoints(
  kind: 'skills' | 'agents' | 'commands',
): Promise<Record<string, string>> {
  const result: Record<string, string> = {}
  const pluginsFile = path.join(os.homedir(), '.claude', 'plugins', 'installed_plugins.json')
  let raw: string
  try { raw = await fs.promises.readFile(pluginsFile, 'utf-8') } catch { return result }
  let parsed: any
  try { parsed = JSON.parse(raw) } catch { return result }
  const plugins = parsed?.plugins
  if (!plugins || typeof plugins !== 'object') return result

  const enabled = await loadEnabledPluginsMap()

  await Promise.all(Object.entries(plugins).map(async ([key, entries]: [string, any]) => {
    if (enabled.get(key) === false) return  // user disabled
    if (!Array.isArray(entries) || entries.length === 0) return
    // First entry per plugin key is authoritative (matches installer behavior).
    const entry = entries[0]
    const installPath = entry?.installPath
    if (!installPath || typeof installPath !== 'string') return
    const subdir = path.join(installPath, kind)
    const items = await readEntrypointMd(subdir, kind)
    for (const [name, content] of Object.entries(items)) {
      // Server-side CLI only looks at `~/.claude/skills/<slug>/SKILL.md` (and
      // equivalent for agents/commands). If we keyed this as
      // `plugin:<key>/<name>/SKILL.md` the extra folder level hides it from
      // CLI discovery even though the Synced Data panel shows the tree. Flat
      // keys land the file right where CLI expects.
      //   - No prefix if no collision with user skill (common case)
      //   - Prefix with plugin key when the user already has a skill of the
      //     same slug, to preserve both.
      const keySlug = key.replace(/@/g, '__at__').replace(/[^a-zA-Z0-9_.-]/g, '-')
      const primaryKey = name
      if (result[primaryKey] !== undefined) {
        // Already set by another plugin with same slug — keep both via prefix.
        result[`${keySlug}--${name}`] = content
      } else {
        result[primaryKey] = content
      }
    }
  }))
  return result
}

function log(msg: string, ...args: unknown[]) {
  console.log(`[sync] ${msg}`, ...args)
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/**
 * Upload user-level files (~/.claude/skills, agents, settings.json, CLAUDE.md) to bridge.
 * Debounced: skips if synced less than 60s ago.
 */
export async function syncUserData(serverUrl: string, token: string): Promise<void> {
  const now = Date.now()
  if (now - lastUserSync < SYNC_DEBOUNCE_MS) {
    return // debounced
  }

  const hash = tokenHash(token)
  const claudeDir = path.join(os.homedir(), '.claude')

  const [userSkills, userAgents, userCommands, pluginSkills, pluginAgents, pluginCommands, settings, claudeMd] = await Promise.all([
    readEntrypointMd(path.join(claudeDir, 'skills'), 'skills'),
    readEntrypointMd(path.join(claudeDir, 'agents'), 'agents'),
    readEntrypointMd(path.join(claudeDir, 'commands'), 'commands'),
    readPluginEntrypoints('skills'),
    readPluginEntrypoints('agents'),
    readPluginEntrypoints('commands'),
    safeReadFile(path.join(claudeDir, 'settings.json')),
    safeReadFile(path.join(claudeDir, 'CLAUDE.md')),
  ])
  // Merge user-level and plugin-level as flat skill/agent/command trees —
  // CLI only scans the top level of `~/.claude/{skills,agents,commands}`,
  // so the old `plugin:<k>/<name>` nested path was hidden from slash-command
  // discovery. Spreading plugin first + user last means user-level entries
  // win on name collisions.
  const skills = { ...pluginSkills, ...userSkills }
  const agents = { ...pluginAgents, ...userAgents }
  const commands = { ...pluginCommands, ...userCommands }
  const data: SyncUserData = { skills, agents, commands, settings, claudeMd }

  // Skip if nothing to sync
  const hasContent =
    Object.keys(data.skills).length > 0 ||
    Object.keys(data.agents).length > 0 ||
    Object.keys(data.commands).length > 0 ||
    data.settings ||
    data.claudeMd
  if (!hasContent) {
    lastUserSync = now
    return
  }

  const url = `${toHttpUrl(serverUrl)}/api/sync/${hash}/user`

  try {
    const resp = await fetch(url, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(data),
      signal: AbortSignal.timeout(10_000),
    })

    if (!resp.ok) {
      const body = await resp.text()
      log(`User sync failed: ${resp.status} ${body}`)
      return
    }

    lastUserSync = now
    log(`User data synced (skills: ${Object.keys(data.skills).length} [${Object.keys(pluginSkills).length} from plugins], agents: ${Object.keys(data.agents).length} [${Object.keys(pluginAgents).length} from plugins], commands: ${Object.keys(data.commands).length} [${Object.keys(pluginCommands).length} from plugins], settings: ${!!data.settings}, claudeMd: ${!!data.claudeMd})`)
  } catch (err) {
    log('User sync error:', err instanceof Error ? err.message : String(err))
  }
}

/**
 * Upload project-level files (.claude/ dir + CLAUDE.md + .claude.json) to bridge.
 * Debounced per project path: skips if synced less than 60s ago.
 */
export async function syncProjectData(serverUrl: string, token: string, projectPath: string): Promise<void> {
  const now = Date.now()
  const lastSync = lastProjectSync.get(projectPath) ?? 0
  if (now - lastSync < SYNC_DEBOUNCE_MS) {
    return // debounced
  }

  const hash = tokenHash(token)
  const dotClaudeDir = path.join(projectPath, '.claude')

  const [skills, rules, agents, commands, claudeMd, dotClaudeMd, claudeJson] = await Promise.all([
    readEntrypointMd(path.join(dotClaudeDir, 'skills'), 'skills'),
    readEntrypointMd(path.join(dotClaudeDir, 'rules'), 'commands'),  // rules: flat .md, treat like commands
    readEntrypointMd(path.join(dotClaudeDir, 'agents'), 'agents'),
    readEntrypointMd(path.join(dotClaudeDir, 'commands'), 'commands'),
    safeReadFile(path.join(projectPath, 'CLAUDE.md')),
    safeReadFile(path.join(dotClaudeDir, 'CLAUDE.md')),
    safeReadFile(path.join(projectPath, '.claude.json')),
  ])
  const data: SyncProjectData = { skills, rules, agents, commands, claudeMd, dotClaudeMd, claudeJson, projectPath }

  // Skip if nothing to sync
  const hasContent =
    Object.keys(data.skills).length > 0 ||
    Object.keys(data.rules).length > 0 ||
    Object.keys(data.agents).length > 0 ||
    Object.keys(data.commands).length > 0 ||
    data.claudeMd ||
    data.dotClaudeMd ||
    data.claudeJson
  if (!hasContent) {
    lastProjectSync.set(projectPath, now)
    return
  }

  const url = `${toHttpUrl(serverUrl)}/api/sync/${hash}/project`

  try {
    const resp = await fetch(url, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(data),
      signal: AbortSignal.timeout(10_000),
    })

    if (!resp.ok) {
      const body = await resp.text()
      log(`Project sync failed: ${resp.status} ${body}`)
      return
    }

    lastProjectSync.set(projectPath, now)
    log(`Project data synced for ${path.basename(projectPath)} (skills: ${Object.keys(data.skills).length}, rules: ${Object.keys(data.rules).length}, agents: ${Object.keys(data.agents).length}, commands: ${Object.keys(data.commands).length})`)
  } catch (err) {
    log('Project sync error:', err instanceof Error ? err.message : String(err))
  }
}

/**
 * Force reset debounce timers (e.g., when user explicitly requests re-sync).
 */
export function resetSyncTimers(): void {
  lastUserSync = 0
  lastProjectSync.clear()
}

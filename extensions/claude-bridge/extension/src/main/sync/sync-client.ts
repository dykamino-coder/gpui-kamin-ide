// ============================================================================
// Sync Client — uploads user-level and project-level files to bridge server
// ============================================================================

import fs from 'fs'
import path from 'path'
import os from 'os'
import crypto from 'crypto'
import type { SyncUserData, SyncProjectData } from '../../shared/sync-types'
import { buildPluginSnapshots } from './plugin-snapshot'

// ---------------------------------------------------------------------------
// Debounce state
// ---------------------------------------------------------------------------

const SYNC_DEBOUNCE_MS = 60_000  // Don't re-sync if synced < 60s ago
const lastUserSync = new Map<string, number>()
const lastProjectSync = new Map<string, number>()
const userSyncInFlight = new Map<string, Promise<SyncResult>>()
const projectSyncInFlight = new Map<string, Promise<SyncResult>>()

export interface SyncResult {
  ok: boolean
  skipped?: boolean
  error?: string
}

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
async function syncUserDataOnce(serverUrl: string, token: string): Promise<SyncResult> {
  const now = Date.now()
  const syncKey = `${toHttpUrl(serverUrl)}\0${tokenHash(token)}`
  if (now - (lastUserSync.get(syncKey) ?? 0) < SYNC_DEBOUNCE_MS) {
    return { ok: true, skipped: true }
  }

  const hash = tokenHash(token)
  const claudeDir = path.join(os.homedir(), '.claude')

  const [skills, agents, commands, plugins, settings, claudeMd] = await Promise.all([
    readEntrypointMd(path.join(claudeDir, 'skills'), 'skills'),
    readEntrypointMd(path.join(claudeDir, 'agents'), 'agents'),
    readEntrypointMd(path.join(claudeDir, 'commands'), 'commands'),
    buildPluginSnapshots(),
    safeReadFile(path.join(claudeDir, 'settings.json')),
    safeReadFile(path.join(claudeDir, 'CLAUDE.md')),
  ])
  const data: SyncUserData = { skills, agents, commands, plugins, settings, claudeMd }

  const url = `${toHttpUrl(serverUrl)}/api/sync/${hash}/user`

  try {
    const resp = await fetch(url, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${token}` },
      body: JSON.stringify(data),
      signal: AbortSignal.timeout(10_000),
    })

    if (!resp.ok) {
      const body = await resp.text()
      const error = `User sync failed: ${resp.status} ${body}`
      log(error)
      return { ok: false, error }
    }

    lastUserSync.set(syncKey, now)
    log(`User data synced (skills: ${Object.keys(data.skills).length}, agents: ${Object.keys(data.agents).length}, commands: ${Object.keys(data.commands).length}, plugins: ${Object.keys(data.plugins).length}, settings: ${!!data.settings}, claudeMd: ${!!data.claudeMd})`)
    return { ok: true }
  } catch (err) {
    const error = `User sync error: ${err instanceof Error ? err.message : String(err)}`
    log(error)
    return { ok: false, error }
  }
}

export function syncUserData(serverUrl: string, token: string): Promise<SyncResult> {
  const key = `${toHttpUrl(serverUrl)}\0${tokenHash(token)}`
  const existing = userSyncInFlight.get(key)
  if (existing) return existing
  const pending = syncUserDataOnce(serverUrl, token)
  userSyncInFlight.set(key, pending)
  return pending.finally(() => {
    if (userSyncInFlight.get(key) === pending) userSyncInFlight.delete(key)
  })
}

/**
 * Upload project-level files (.claude/ dir + CLAUDE.md + .claude.json) to bridge.
 * Debounced per project path: skips if synced less than 60s ago.
 */
async function syncProjectDataOnce(serverUrl: string, token: string, projectPath: string): Promise<SyncResult> {
  const now = Date.now()
  const syncKey = `${toHttpUrl(serverUrl)}\0${tokenHash(token)}\0${projectPath}`
  const lastSync = lastProjectSync.get(syncKey) ?? 0
  if (now - lastSync < SYNC_DEBOUNCE_MS) {
    return { ok: true, skipped: true }
  }

  const hash = tokenHash(token)
  const dotClaudeDir = path.join(projectPath, '.claude')

  const [skills, rules, agents, commands, settings, claudeMd, dotClaudeMd, claudeJson] = await Promise.all([
    readEntrypointMd(path.join(dotClaudeDir, 'skills'), 'skills'),
    readEntrypointMd(path.join(dotClaudeDir, 'rules'), 'commands'),  // rules: flat .md, treat like commands
    readEntrypointMd(path.join(dotClaudeDir, 'agents'), 'agents'),
    readEntrypointMd(path.join(dotClaudeDir, 'commands'), 'commands'),
    safeReadFile(path.join(dotClaudeDir, 'settings.json')),
    safeReadFile(path.join(projectPath, 'CLAUDE.md')),
    safeReadFile(path.join(dotClaudeDir, 'CLAUDE.md')),
    safeReadFile(path.join(projectPath, '.claude.json')),
  ])
  const data: SyncProjectData = { skills, rules, agents, commands, settings, claudeMd, dotClaudeMd, claudeJson, projectPath }

  const url = `${toHttpUrl(serverUrl)}/api/sync/${hash}/project`

  try {
    const resp = await fetch(url, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${token}` },
      body: JSON.stringify(data),
      signal: AbortSignal.timeout(10_000),
    })

    if (!resp.ok) {
      const body = await resp.text()
      const error = `Project sync failed: ${resp.status} ${body}`
      log(error)
      return { ok: false, error }
    }

    lastProjectSync.set(syncKey, now)
    log(`Project data synced for ${path.basename(projectPath)} (skills: ${Object.keys(data.skills).length}, rules: ${Object.keys(data.rules).length}, agents: ${Object.keys(data.agents).length}, commands: ${Object.keys(data.commands).length})`)
    return { ok: true }
  } catch (err) {
    const error = `Project sync error: ${err instanceof Error ? err.message : String(err)}`
    log(error)
    return { ok: false, error }
  }
}

export function syncProjectData(serverUrl: string, token: string, projectPath: string): Promise<SyncResult> {
  const key = `${toHttpUrl(serverUrl)}\0${tokenHash(token)}\0${projectPath}`
  const existing = projectSyncInFlight.get(key)
  if (existing) return existing
  const pending = syncProjectDataOnce(serverUrl, token, projectPath)
  projectSyncInFlight.set(key, pending)
  return pending.finally(() => {
    if (projectSyncInFlight.get(key) === pending) projectSyncInFlight.delete(key)
  })
}

/**
 * Force reset debounce timers (e.g., when user explicitly requests re-sync).
 */
export function resetSyncTimers(): void {
  lastUserSync.clear()
  lastProjectSync.clear()
}

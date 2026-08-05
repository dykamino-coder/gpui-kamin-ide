// ============================================================================
// Per-token Sync API — stores user-level and project-level files for sessions
// ============================================================================

import { Hono } from 'hono'
import crypto from 'crypto'
import fs from 'fs'
import fsp from 'fs/promises'
import path from 'path'
import os from 'os'
import { debugLog, warnLog } from '../logging'
import type { SyncUserData, SyncProjectData } from '../pty/types'
import { getAllSessions } from '../pty/session-core'
import { requestMaintenanceSubmission } from '../pty/session-input-coordinator'
import { refreshSessionSkills } from '../pty/session-settings'
import { withProjectSyncLock, withUserSyncLock } from './lock'

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const SYNC_BASE = path.join(os.homedir(), 'bridge-sync')

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/** First 16 chars of sha256(token) — deterministic, safe for dir names */
export function tokenHash(token: string): string {
  return crypto.createHash('sha256').update(token).digest('hex').slice(0, 16)
}

/** Hash a project path for storage keying */
function projectHash(projectPath: string): string {
  return crypto.createHash('sha256').update(projectPath).digest('hex').slice(0, 16)
}

/** Get the user sync directory for a given token hash */
export function getUserSyncDir(hash: string): string {
  return path.join(SYNC_BASE, 'users', hash)
}

/** Get the project sync directory for a given token hash + project path */
export function getProjectSyncDir(hash: string, projectPath: string): string {
  return path.join(SYNC_BASE, 'projects', hash, projectHash(projectPath))
}

/** Write a Record<relativePath, content> into a directory. Async so the sync
 *  upload handler (fires on every client poll) doesn't block the event loop on
 *  a recursive mkdir+write over the whole skills/agents/commands tree. */
async function writeFileMap(baseDir: string, files: Record<string, string>): Promise<void> {
  for (const [relPath, content] of Object.entries(files)) {
    // Sanitize: no path traversal
    const safe = relPath.replace(/\.\./g, '_').replace(/^\//, '')
    if (!safe) continue
    const fullPath = path.join(baseDir, safe)
    await fsp.mkdir(path.dirname(fullPath), { recursive: true })
    await fsp.writeFile(fullPath, content, 'utf-8')
  }
}

/** Skills payloads are complete snapshots. Removing the previous tree first
 * makes disabled/deleted skills disappear instead of accumulating forever. */
async function replaceFileMap(baseDir: string, files: Record<string, string>): Promise<void> {
  await fsp.rm(baseDir, { recursive: true, force: true })
  await writeFileMap(baseDir, files)
}

/**
 * After a live skills upload, make the freshly-synced skills visible in
 * already-running sessions for this token WITHOUT a restart. Rebuild the exact
 * session-local user + project overlay, then leave `/reload-skills` pending in
 * the PTY coordinator. It runs only at an attached, clean, prompt-ready boundary.
 *
 * @param tokenId     the sync token hash (equals PtySession.bearerHash)
 * @param projectPath when set, only project sessions whose cwd matches are
 *                    reloaded (and their on-disk copy is refreshed first)
 */
/** Compare an incoming {relPath: content} map against what's already on disk —
 *  used to skip the /reload-skills injection when a sync upload didn't actually
 *  change anything (the client re-uploads on a poll, so most uploads are no-ops). */
async function fileMapEqual(dir: string, incoming: Record<string, string>): Promise<boolean> {
  const current = await readFileMap(dir)
  const ck = Object.keys(current), ik = Object.keys(incoming)
  if (ck.length !== ik.length) return false
  for (const k of ik) if (current[k] !== incoming[k]) return false
  return true
}

function reloadSkillsForRunningSessions(tokenId: string, projectPath?: string): void {
  for (const s of getAllSessions()) {
    if (s.bearerHash !== tokenId || s.state !== 'running') continue
    if (projectPath && s.cwd !== projectPath) continue
    try { refreshSessionSkills(s.settingsDir, tokenId, s.cwd || undefined) }
    catch { /* pending reload still refreshes any intact tree */ }
    requestMaintenanceSubmission(s, 'reload-skills', '/reload-skills')
  }
}

/** Read all files recursively from a directory into a Record<relativePath,
 *  content>. Async: this recursive walk runs on every upload (via fileMapEqual)
 *  and on every GET /status — a sync readdir+readFile over the whole tree stalled
 *  the event loop. A missing dir just yields an empty map (readdir throws). */
async function readFileMap(baseDir: string): Promise<Record<string, string>> {
  const result: Record<string, string> = {}
  const walk = async (dir: string, prefix: string): Promise<void> => {
    let entries
    try { entries = await fsp.readdir(dir, { withFileTypes: true }) } catch { return }
    for (const entry of entries) {
      const rel = prefix ? `${prefix}/${entry.name}` : entry.name
      if (entry.isDirectory()) {
        await walk(path.join(dir, entry.name), rel)
      } else {
        try {
          result[rel] = await fsp.readFile(path.join(dir, entry.name), 'utf-8')
        } catch { /* unreadable file — skip */ }
      }
    }
  }
  await walk(baseDir, '')
  return result
}

// ---------------------------------------------------------------------------
// Routes
// ---------------------------------------------------------------------------

export function createSyncRoutes(): Hono {
  const api = new Hono()

  // POST /api/sync/:tokenId/user — upload user-level files
  api.post('/api/sync/:tokenId/user', async (c) => {
    const tokenId = c.req.param('tokenId')
    if (!tokenId || tokenId.length < 8) {
      return c.json({ error: 'Invalid tokenId' }, 400)
    }

    let body: SyncUserData
    try {
      body = await c.req.json<SyncUserData>()
    } catch {
      return c.json({ error: 'Invalid JSON body' }, 400)
    }

    const hash = tokenId // tokenId is already the hash from Electron
    const userDir = getUserSyncDir(hash)

    try {
      // A present skills field is a complete snapshot; omission keeps the
      // previous snapshot for compatibility with older partial-sync clients.
      if (body.skills !== undefined) {
        const skills = body.skills
        await withUserSyncLock(hash, async () => {
          const skillsDir = path.join(userDir, 'skills')
          // The client re-uploads unchanged maps during polling. Avoid a reload
          // unless the complete snapshot actually changed.
          const changed = !await fileMapEqual(skillsDir, skills)
          if (changed) await replaceFileMap(skillsDir, skills)
          debugLog('[sync] User skills synced', { tokenId: hash, count: Object.keys(skills).length, changed })
          if (changed) reloadSkillsForRunningSessions(hash)
        })
      }

      // Write agents
      if (body.agents && Object.keys(body.agents).length > 0) {
        const agentsDir = path.join(userDir, 'agents')
        await fsp.mkdir(agentsDir, { recursive: true })
        await writeFileMap(agentsDir, body.agents)
        debugLog('[sync] User agents synced', { tokenId: hash, count: Object.keys(body.agents).length })
      }

      // Write commands (custom slash commands)
      if (body.commands && Object.keys(body.commands).length > 0) {
        const commandsDir = path.join(userDir, 'commands')
        await fsp.mkdir(commandsDir, { recursive: true })
        await writeFileMap(commandsDir, body.commands)
        debugLog('[sync] User commands synced', { tokenId: hash, count: Object.keys(body.commands).length })
      }

      // Write settings.json
      if (body.settings) {
        await fsp.mkdir(userDir, { recursive: true })
        await fsp.writeFile(path.join(userDir, 'settings.json'), body.settings, 'utf-8')
        debugLog('[sync] User settings.json synced', { tokenId: hash })
      }

      // Write CLAUDE.md
      if (body.claudeMd) {
        await fsp.mkdir(userDir, { recursive: true })
        await fsp.writeFile(path.join(userDir, 'CLAUDE.md'), body.claudeMd, 'utf-8')
        debugLog('[sync] User CLAUDE.md synced', { tokenId: hash })
      }

      return c.json({ ok: true })
    } catch (err) {
      const errMsg = err instanceof Error ? err.message : String(err)
      warnLog('[sync] Failed to sync user data', { tokenId: hash, error: errMsg })
      return c.json({ error: 'Sync failed', details: errMsg }, 500)
    }
  })

  // POST /api/sync/:tokenId/project — upload project-level files
  api.post('/api/sync/:tokenId/project', async (c) => {
    const tokenId = c.req.param('tokenId')
    if (!tokenId || tokenId.length < 8) {
      return c.json({ error: 'Invalid tokenId' }, 400)
    }

    let body: SyncProjectData
    try {
      body = await c.req.json<SyncProjectData>()
    } catch {
      return c.json({ error: 'Invalid JSON body' }, 400)
    }

    if (!body.projectPath) {
      return c.json({ error: 'Missing projectPath' }, 400)
    }

    const hash = tokenId
    const projDir = getProjectSyncDir(hash, body.projectPath)

    try {
      await fsp.mkdir(projDir, { recursive: true })

      // Like user skills, a present field is an exact snapshot, including {}.
      if (body.skills !== undefined) {
        const skills = body.skills
        await withProjectSyncLock(hash, body.projectPath, async () => {
          const skillsDir = path.join(projDir, 'skills')
          const changed = !await fileMapEqual(skillsDir, skills)
          if (changed) await replaceFileMap(skillsDir, skills)
          debugLog('[sync] Project skills synced', { tokenId: hash, project: body.projectPath, count: Object.keys(skills).length, changed })
          if (changed) reloadSkillsForRunningSessions(hash, body.projectPath)
        })
      }

      // Write rules
      if (body.rules && Object.keys(body.rules).length > 0) {
        await writeFileMap(path.join(projDir, 'rules'), body.rules)
        debugLog('[sync] Project rules synced', { tokenId: hash, project: body.projectPath, count: Object.keys(body.rules).length })
      }

      // Write agents
      if (body.agents && Object.keys(body.agents).length > 0) {
        await writeFileMap(path.join(projDir, 'agents'), body.agents)
        debugLog('[sync] Project agents synced', { tokenId: hash, project: body.projectPath, count: Object.keys(body.agents).length })
      }

      // Write commands
      if (body.commands && Object.keys(body.commands).length > 0) {
        await writeFileMap(path.join(projDir, 'commands'), body.commands)
        debugLog('[sync] Project commands synced', { tokenId: hash, project: body.projectPath, count: Object.keys(body.commands).length })
      }

      // Write root CLAUDE.md
      if (body.claudeMd) {
        await fsp.writeFile(path.join(projDir, 'CLAUDE.md'), body.claudeMd, 'utf-8')
        debugLog('[sync] Project CLAUDE.md synced', { tokenId: hash, project: body.projectPath })
      }

      // Write .claude/CLAUDE.md
      if (body.dotClaudeMd) {
        const dotClaudeDir = path.join(projDir, '.claude')
        await fsp.mkdir(dotClaudeDir, { recursive: true })
        await fsp.writeFile(path.join(dotClaudeDir, 'CLAUDE.md'), body.dotClaudeMd, 'utf-8')
        debugLog('[sync] Project .claude/CLAUDE.md synced', { tokenId: hash, project: body.projectPath })
      }

      // Write .claude.json
      if (body.claudeJson) {
        await fsp.writeFile(path.join(projDir, '.claude.json'), body.claudeJson, 'utf-8')
        debugLog('[sync] Project .claude.json synced', { tokenId: hash, project: body.projectPath })
      }

      // Store the projectPath for later lookup
      await fsp.writeFile(path.join(projDir, '_projectPath.txt'), body.projectPath, 'utf-8')

      return c.json({ ok: true })
    } catch (err) {
      const errMsg = err instanceof Error ? err.message : String(err)
      warnLog('[sync] Failed to sync project data', { tokenId: hash, error: errMsg })
      return c.json({ error: 'Sync failed', details: errMsg }, 500)
    }
  })

  // GET /api/sync/:tokenId/status — what's synced
  api.get('/api/sync/:tokenId/status', async (c) => {
    const tokenId = c.req.param('tokenId')
    if (!tokenId || tokenId.length < 8) {
      return c.json({ error: 'Invalid tokenId' }, 400)
    }

    const hash = tokenId
    const userDir = getUserSyncDir(hash)
    const projectsBase = path.join(SYNC_BASE, 'projects', hash)

    const status: {
      user: { skills: number; agents: number; commands: number; hasSettings: boolean; hasClaudeMd: boolean } | null
      projects: Array<{ projectPath: string; skills: number; rules: number; agents: number; commands: number; hasClaudeMd: boolean; hasDotClaudeMd: boolean; hasClaudeJson: boolean }>
    } = {
      user: null,
      projects: [],
    }

    // User data status
    if (fs.existsSync(userDir)) {
      const skills = await readFileMap(path.join(userDir, 'skills'))
      const agents = await readFileMap(path.join(userDir, 'agents'))
      const commands = await readFileMap(path.join(userDir, 'commands'))
      status.user = {
        skills: Object.keys(skills).length,
        agents: Object.keys(agents).length,
        commands: Object.keys(commands).length,
        hasSettings: fs.existsSync(path.join(userDir, 'settings.json')),
        hasClaudeMd: fs.existsSync(path.join(userDir, 'CLAUDE.md')),
      }
    }

    // Project data status
    if (fs.existsSync(projectsBase)) {
      try {
        for (const entry of fs.readdirSync(projectsBase, { withFileTypes: true })) {
          if (!entry.isDirectory()) continue
          const projDir = path.join(projectsBase, entry.name)
          const projectPathFile = path.join(projDir, '_projectPath.txt')
          const projectPath = fs.existsSync(projectPathFile) ? fs.readFileSync(projectPathFile, 'utf-8') : entry.name

          const skills = await readFileMap(path.join(projDir, 'skills'))
          const rules = await readFileMap(path.join(projDir, 'rules'))
          const agents = await readFileMap(path.join(projDir, 'agents'))
          const commands = await readFileMap(path.join(projDir, 'commands'))

          status.projects.push({
            projectPath,
            skills: Object.keys(skills).length,
            rules: Object.keys(rules).length,
            agents: Object.keys(agents).length,
            commands: Object.keys(commands).length,
            hasClaudeMd: fs.existsSync(path.join(projDir, 'CLAUDE.md')),
            hasDotClaudeMd: fs.existsSync(path.join(projDir, '.claude', 'CLAUDE.md')),
            hasClaudeJson: fs.existsSync(path.join(projDir, '.claude.json')),
          })
        }
      } catch {}
    }

    return c.json(status)
  })

  // GET /api/sync/:tokenId/tree — returns absolute base path + recursive file tree
  // (name + size + children). Used by the UI to show what was actually written.
  api.get('/api/sync/:tokenId/tree', (c) => {
    const tokenId = c.req.param('tokenId')
    if (!tokenId || tokenId.length < 8) {
      return c.json({ error: 'Invalid tokenId' }, 400)
    }
    const hash = tokenId
    const userDir = getUserSyncDir(hash)
    const projectsBase = path.join(SYNC_BASE, 'projects', hash)

    interface Node {
      name: string
      type: 'dir' | 'file'
      size?: number
      children?: Node[]
    }

    function buildTree(dir: string): Node[] {
      if (!fs.existsSync(dir)) return []
      const out: Node[] = []
      try {
        for (const entry of fs.readdirSync(dir, { withFileTypes: true }).sort((a, b) => a.name.localeCompare(b.name))) {
          const full = path.join(dir, entry.name)
          if (entry.isDirectory()) {
            out.push({ name: entry.name, type: 'dir', children: buildTree(full) })
          } else {
            let size = 0
            try { size = fs.statSync(full).size } catch {}
            out.push({ name: entry.name, type: 'file', size })
          }
        }
      } catch {}
      return out
    }

    const payload = {
      tokenId: hash,
      basePath: SYNC_BASE,
      user: {
        path: userDir,
        exists: fs.existsSync(userDir),
        tree: buildTree(userDir),
      },
      projects: [] as Array<{ projectPath: string; path: string; tree: Node[] }>,
    }

    if (fs.existsSync(projectsBase)) {
      try {
        for (const entry of fs.readdirSync(projectsBase, { withFileTypes: true })) {
          if (!entry.isDirectory()) continue
          const projDir = path.join(projectsBase, entry.name)
          const projectPathFile = path.join(projDir, '_projectPath.txt')
          const projectPath = fs.existsSync(projectPathFile) ? fs.readFileSync(projectPathFile, 'utf-8') : entry.name
          payload.projects.push({ projectPath, path: projDir, tree: buildTree(projDir) })
        }
      } catch {}
    }

    return c.json(payload)
  })

  // GET /api/sync/:tokenId/file?path=<abs-path-under-sync-base>
  // Returns raw content of a single synced file, with a hard cap so the
  // response never blows up the client. Refuses anything outside SYNC_BASE.
  api.get('/api/sync/:tokenId/file', (c) => {
    const tokenId = c.req.param('tokenId')
    if (!tokenId || tokenId.length < 8) return c.json({ error: 'Invalid tokenId' }, 400)
    const q = c.req.query('path')
    if (!q) return c.json({ error: 'Missing path query' }, 400)

    const abs = path.resolve(q)
    // Sandbox: must be inside SYNC_BASE and tied to this tokenId
    const sandboxA = path.resolve(SYNC_BASE, 'users', tokenId) + path.sep
    const sandboxB = path.resolve(SYNC_BASE, 'projects', tokenId) + path.sep
    if (!(abs.startsWith(sandboxA) || abs.startsWith(sandboxB))) {
      return c.json({ error: 'Path outside sync sandbox' }, 403)
    }
    try {
      const stat = fs.statSync(abs)
      if (stat.isDirectory()) return c.json({ error: 'Is a directory' }, 400)
      const CAP = 512 * 1024  // 512 KB cap for viewer
      const truncated = stat.size > CAP
      const buf = fs.readFileSync(abs)
      const content = buf.subarray(0, CAP).toString('utf-8')
      return c.json({
        path: abs,
        size: stat.size,
        truncated,
        content,
      })
    } catch (err) {
      return c.json({ error: err instanceof Error ? err.message : String(err) }, 404)
    }
  })

  return api
}

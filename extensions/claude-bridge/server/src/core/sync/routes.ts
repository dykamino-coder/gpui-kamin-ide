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
import type { PtySession } from '../pty/types'
import { requestMaintenanceSubmission } from '../pty/session-input-coordinator'
import { copyDirRecursive } from '../pty/session-settings'
import { resolveToken } from '../auth/tokens'
import { withProjectSyncLock, withUserSyncLock } from './lock'

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const SYNC_BASE = path.join(os.homedir(), 'bridge-sync')
const TOKEN_HASH_RE = /^[a-f0-9]{16}$/
const MAX_SYNC_BODY_BYTES = 10 * 1024 * 1024
const MAX_SYNC_STRING_BYTES = 1024 * 1024
const MAX_SYNC_NODES = 25_000

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/** First 16 chars of sha256(token) — deterministic, safe for dir names */
export function tokenHash(token: string): string {
  return crypto.createHash('sha256').update(token).digest('hex').slice(0, 16)
}

function assertTokenHash(hash: string): void {
  if (!TOKEN_HASH_RE.test(hash)) throw new Error('Invalid tokenId')
}

async function requireOwnedToken(c: import('hono').Context): Promise<Response | null> {
  const hash = c.req.param('tokenId')
  if (!hash || !TOKEN_HASH_RE.test(hash)) return c.json({ error: 'Invalid tokenId' }, 400)
  const auth = c.req.header('Authorization') || ''
  const bearer = auth.startsWith('Bearer ') ? auth.slice(7) : ''
  if (!bearer || !(await resolveToken(bearer))) return c.json({ error: 'Unauthorized' }, 401)
  if (tokenHash(bearer) !== hash) return c.json({ error: 'Forbidden' }, 403)
  return null
}

async function readJsonBodyLimited<T>(c: import('hono').Context): Promise<T> {
  const declared = Number(c.req.header('Content-Length') || 0)
  if (Number.isFinite(declared) && declared > MAX_SYNC_BODY_BYTES) throw new Error('Sync payload too large')
  const stream = c.req.raw.body
  if (!stream) throw new Error('Invalid JSON body')
  const reader = stream.getReader()
  const chunks: Uint8Array[] = []
  let size = 0
  while (true) {
    const { done, value } = await reader.read()
    if (done) break
    size += value.byteLength
    if (size > MAX_SYNC_BODY_BYTES) {
      await reader.cancel()
      throw new Error('Sync payload too large')
    }
    chunks.push(value)
  }
  const merged = new Uint8Array(size)
  let offset = 0
  for (const chunk of chunks) {
    merged.set(chunk, offset)
    offset += chunk.byteLength
  }
  let parsed: unknown
  try { parsed = JSON.parse(new TextDecoder().decode(merged)) } catch { throw new Error('Invalid JSON body') }
  validateSnapshotShape(parsed)
  return parsed as T
}

function validateSnapshotShape(root: unknown): void {
  if (!root || typeof root !== 'object' || Array.isArray(root)) throw new Error('Invalid sync snapshot')
  const stack: Array<{ value: unknown; depth: number }> = [{ value: root, depth: 0 }]
  let nodes = 0
  while (stack.length > 0) {
    const { value, depth } = stack.pop()!
    if (++nodes > MAX_SYNC_NODES || depth > 20) throw new Error('Sync snapshot is too complex')
    if (typeof value === 'string') {
      if (Buffer.byteLength(value, 'utf-8') > MAX_SYNC_STRING_BYTES) throw new Error('Sync snapshot contains an oversized value')
      continue
    }
    if (!value || typeof value !== 'object') continue
    if (Array.isArray(value)) {
      for (const child of value) stack.push({ value: child, depth: depth + 1 })
    } else {
      for (const [key, child] of Object.entries(value)) {
        if (Buffer.byteLength(key, 'utf-8') > 4_096) throw new Error('Sync snapshot contains an oversized key')
        stack.push({ value: child, depth: depth + 1 })
      }
    }
  }
}

/** Hash a project path for storage keying */
function projectHash(projectPath: string): string {
  return crypto.createHash('sha256').update(projectPath).digest('hex').slice(0, 16)
}

/** Get the user sync directory for a given token hash */
export function getUserSyncDir(hash: string): string {
  assertTokenHash(hash)
  return path.join(SYNC_BASE, 'users', hash)
}

/** Get the project sync directory for a given token hash + project path */
export function getProjectSyncDir(hash: string, projectPath: string): string {
  assertTokenHash(hash)
  return path.join(SYNC_BASE, 'projects', hash, projectHash(projectPath))
}

/** Write a Record<relativePath, content> into a directory. Async so the sync
 *  upload handler (fires on every client poll) doesn't block the event loop on
 *  a recursive mkdir+write over the whole skills/agents/commands tree. */
async function writeFileMap(baseDir: string, files: Record<string, string>): Promise<void> {
  // A sync payload is a complete snapshot, not an additive patch. Removing the
  // old tree first is what makes disable/uninstall/delete visible instead of
  // leaving stale components discoverable forever.
  await fsp.rm(baseDir, { recursive: true, force: true })
  for (const [relPath, content] of Object.entries(files)) {
    if (typeof content !== 'string' || !relPath || relPath.includes('\\')) continue
    const fullPath = path.resolve(baseDir, relPath)
    const relative = path.relative(path.resolve(baseDir), fullPath)
    if (!relative || relative === '..' || relative.startsWith('..' + path.sep) || path.isAbsolute(relative)) continue
    await fsp.mkdir(path.dirname(fullPath), { recursive: true })
    await fsp.writeFile(fullPath, content, 'utf-8')
  }
}

/** Merge skills without deleting absent paths. Exact replacement belongs to
 * plugin snapshots; user/project skills retain the existing sync contract. */
async function writeSkillsMap(baseDir: string, files: Record<string, string>): Promise<void> {
  for (const [relPath, content] of Object.entries(files)) {
    if (typeof content !== 'string' || !relPath || relPath.includes('\\')) continue
    const fullPath = path.resolve(baseDir, relPath)
    const relative = path.relative(path.resolve(baseDir), fullPath)
    if (!relative || relative === '..' || relative.startsWith('..' + path.sep) || path.isAbsolute(relative)) continue
    await fsp.mkdir(path.dirname(fullPath), { recursive: true })
    await fsp.writeFile(fullPath, content, 'utf-8')
  }
}

function safePluginDirName(pluginId: string): string {
  const slug = pluginId.replace(/[^a-zA-Z0-9_.-]/g, '-') || 'plugin'
  return `${slug}-${crypto.createHash('sha256').update(pluginId).digest('hex').slice(0, 8)}`
}

async function writePluginSnapshots(userDir: string, plugins: SyncUserData['plugins']): Promise<void> {
  const pluginsDir = path.join(userDir, 'plugins')
  await fsp.rm(pluginsDir, { recursive: true, force: true })
  await fsp.mkdir(pluginsDir, { recursive: true })

  const index: Array<{ id: string; dirName: string; sourceRoot: string; hooks: Record<string, unknown> }> = []
  for (const [pluginId, plugin] of Object.entries(plugins ?? {})) {
    if (!plugin || plugin.id !== pluginId || typeof plugin.name !== 'string') continue
    const dirName = safePluginDirName(pluginId)
    const root = path.join(pluginsDir, dirName)
    await fsp.mkdir(path.join(root, '.claude-plugin'), { recursive: true })
    await fsp.writeFile(
      path.join(root, '.claude-plugin', 'plugin.json'),
      JSON.stringify({ ...plugin.manifest, name: plugin.name }, null, 2),
      'utf-8',
    )
    await writeFileMap(path.join(root, 'skills'), plugin.skills ?? {})
    await writeFileMap(path.join(root, 'agents'), plugin.agents ?? {})
    await writeFileMap(path.join(root, 'commands'), plugin.commands ?? {})
    await writeFileMap(path.join(root, 'workflows'), plugin.workflows ?? {})
    await writeFileMap(path.join(root, 'output-styles'), plugin.outputStyles ?? {})
    await writeFileMap(path.join(root, 'themes'), plugin.themes ?? {})
    if (plugin.settings) await fsp.writeFile(path.join(root, 'settings.json'), plugin.settings, 'utf-8')
    index.push({ id: pluginId, dirName, sourceRoot: plugin.sourceRoot, hooks: plugin.hooks ?? {} })
  }
  await fsp.writeFile(path.join(userDir, 'plugins.json'), JSON.stringify(index, null, 2), 'utf-8')
}

/**
 * After a live skills upload, make the freshly-synced skills visible in
 * already-running sessions for this token WITHOUT a restart. User skills are
 * copied into `.claude/skills` at spawn, and project skills are overlaid there.
 * For project changes we copy the fresh files before injecting the CLI's
 * `/reload-skills` slash command. Best-effort: a mid-turn injection is queued by
 * the CLI after the current turn.
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
    if (projectPath) {
      if (s.cwd !== projectPath) continue
      try {
        const src = path.join(getProjectSyncDir(tokenId, projectPath), 'skills')
        if (fs.existsSync(src)) copyDirRecursive(src, path.join(s.settingsDir, '.claude', 'skills'))
      } catch { /* re-copy best-effort — /reload-skills still refreshes the cache */ }
    }
    // Через координатор, а не впрыском: он держит команду до чистой границы
    // приглашения. Впрыск в середине хода съедал набранный ввод.
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
      } else if (entry.isFile()) {
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
    const authError = await requireOwnedToken(c)
    if (authError) return authError

    let body: SyncUserData
    try {
      body = await readJsonBodyLimited<SyncUserData>(c)
    } catch (err) {
      const message = err instanceof Error ? err.message : 'Invalid JSON body'
      return c.json({ error: message }, message.includes('too large') ? 413 : 400)
    }

    const hash = tokenId // tokenId is already the hash from the client host
    const userDir = getUserSyncDir(hash)

    return withUserSyncLock(hash, async () => {
      try {
      // Write skills when present. Missing paths are not treated as deletions.
      if (body.skills && Object.keys(body.skills).length > 0) {
        const skillsDir = path.join(userDir, 'skills')
        const skillsChanged = !await fileMapEqual(skillsDir, body.skills)
        await fsp.mkdir(skillsDir, { recursive: true })
        await writeSkillsMap(skillsDir, body.skills)
        debugLog('[sync] User skills synced', { tokenId: hash, count: Object.keys(body.skills).length, changed: skillsChanged })
        if (skillsChanged) reloadSkillsForRunningSessions(hash)
      }

      // Write agents
      await writeFileMap(path.join(userDir, 'agents'), body.agents ?? {})
      debugLog('[sync] User agents synced', { tokenId: hash, count: Object.keys(body.agents ?? {}).length })

      // Write commands (custom slash commands)
      await writeFileMap(path.join(userDir, 'commands'), body.commands ?? {})
      debugLog('[sync] User commands synced', { tokenId: hash, count: Object.keys(body.commands ?? {}).length })

      // Namespaced plugin proxy roots + hook metadata. Executables are not
      // materialised here; the host-side harness owns their processes.
      await writePluginSnapshots(userDir, body.plugins ?? {})
      debugLog('[sync] User plugins synced', { tokenId: hash, count: Object.keys(body.plugins ?? {}).length })

      // Write settings.json
      await fsp.mkdir(userDir, { recursive: true })
      if (body.settings !== undefined) await fsp.writeFile(path.join(userDir, 'settings.json'), body.settings, 'utf-8')
      else await fsp.rm(path.join(userDir, 'settings.json'), { force: true })
      debugLog('[sync] User settings.json synced', { tokenId: hash, present: body.settings !== undefined })

      // Write CLAUDE.md
      if (body.claudeMd !== undefined) await fsp.writeFile(path.join(userDir, 'CLAUDE.md'), body.claudeMd, 'utf-8')
      else await fsp.rm(path.join(userDir, 'CLAUDE.md'), { force: true })
      debugLog('[sync] User CLAUDE.md synced', { tokenId: hash, present: body.claudeMd !== undefined })

        return c.json({ ok: true })
      } catch (err) {
        const errMsg = err instanceof Error ? err.message : String(err)
        warnLog('[sync] Failed to sync user data', { tokenId: hash, error: errMsg })
        return c.json({ error: 'Sync failed', details: errMsg }, 500)
      }
    })
  })

  // POST /api/sync/:tokenId/project — upload project-level files
  api.post('/api/sync/:tokenId/project', async (c) => {
    const tokenId = c.req.param('tokenId')
    const authError = await requireOwnedToken(c)
    if (authError) return authError

    let body: SyncProjectData
    try {
      body = await readJsonBodyLimited<SyncProjectData>(c)
    } catch (err) {
      const message = err instanceof Error ? err.message : 'Invalid JSON body'
      return c.json({ error: message }, message.includes('too large') ? 413 : 400)
    }

    if (typeof body.projectPath !== 'string' || !body.projectPath || body.projectPath.length > 4_096) {
      return c.json({ error: 'Missing projectPath' }, 400)
    }

    const hash = tokenId
    const projDir = getProjectSyncDir(hash, body.projectPath)

    try {
      await fsp.mkdir(projDir, { recursive: true })

      // Write skills when present. Missing paths are not treated as deletions.
      if (body.skills && Object.keys(body.skills).length > 0) {
        const skillsDir = path.join(projDir, 'skills')
        const skillsChanged = !await fileMapEqual(skillsDir, body.skills)
        await writeSkillsMap(skillsDir, body.skills)
        debugLog('[sync] Project skills synced', { tokenId: hash, project: body.projectPath, count: Object.keys(body.skills).length, changed: skillsChanged })
        if (skillsChanged) reloadSkillsForRunningSessions(hash, body.projectPath)
      }

      // Write rules
      await writeFileMap(path.join(projDir, 'rules'), body.rules ?? {})
      debugLog('[sync] Project rules synced', { tokenId: hash, project: body.projectPath, count: Object.keys(body.rules ?? {}).length })

      // Write agents
      await writeFileMap(path.join(projDir, 'agents'), body.agents ?? {})
      debugLog('[sync] Project agents synced', { tokenId: hash, project: body.projectPath, count: Object.keys(body.agents ?? {}).length })

      // Write commands
      await writeFileMap(path.join(projDir, 'commands'), body.commands ?? {})
      debugLog('[sync] Project commands synced', { tokenId: hash, project: body.projectPath, count: Object.keys(body.commands ?? {}).length })

      if (body.settings !== undefined) await fsp.writeFile(path.join(projDir, 'settings.json'), body.settings, 'utf-8')
      else await fsp.rm(path.join(projDir, 'settings.json'), { force: true })
      debugLog('[sync] Project settings.json synced', { tokenId: hash, project: body.projectPath, present: body.settings !== undefined })

      // Write root CLAUDE.md
      if (body.claudeMd !== undefined) await fsp.writeFile(path.join(projDir, 'CLAUDE.md'), body.claudeMd, 'utf-8')
      else await fsp.rm(path.join(projDir, 'CLAUDE.md'), { force: true })
      debugLog('[sync] Project CLAUDE.md synced', { tokenId: hash, project: body.projectPath, present: body.claudeMd !== undefined })

      // Write .claude/CLAUDE.md
      if (body.dotClaudeMd !== undefined) {
        const dotClaudeDir = path.join(projDir, '.claude')
        await fsp.mkdir(dotClaudeDir, { recursive: true })
        await fsp.writeFile(path.join(dotClaudeDir, 'CLAUDE.md'), body.dotClaudeMd, 'utf-8')
        debugLog('[sync] Project .claude/CLAUDE.md synced', { tokenId: hash, project: body.projectPath })
      } else await fsp.rm(path.join(projDir, '.claude', 'CLAUDE.md'), { force: true })

      // Write .claude.json
      if (body.claudeJson !== undefined) await fsp.writeFile(path.join(projDir, '.claude.json'), body.claudeJson, 'utf-8')
      else await fsp.rm(path.join(projDir, '.claude.json'), { force: true })
      debugLog('[sync] Project .claude.json synced', { tokenId: hash, project: body.projectPath, present: body.claudeJson !== undefined })

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
    const authError = await requireOwnedToken(c)
    if (authError) return authError

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
  api.get('/api/sync/:tokenId/tree', async (c) => {
    const tokenId = c.req.param('tokenId')
    const authError = await requireOwnedToken(c)
    if (authError) return authError
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
          } else if (entry.isFile()) {
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
  api.get('/api/sync/:tokenId/file', async (c) => {
    const tokenId = c.req.param('tokenId')
    const authError = await requireOwnedToken(c)
    if (authError) return authError
    const q = c.req.query('path')
    if (!q) return c.json({ error: 'Missing path query' }, 400)

    const abs = path.resolve(q)
    // Sandbox: must be inside SYNC_BASE and tied to this tokenId
    const sandboxA = getUserSyncDir(tokenId) + path.sep
    const sandboxB = path.resolve(SYNC_BASE, 'projects', tokenId) + path.sep
    if (!(abs.startsWith(sandboxA) || abs.startsWith(sandboxB))) {
      return c.json({ error: 'Path outside sync sandbox' }, 403)
    }
    try {
      const real = fs.realpathSync(abs)
      const allowedRoots = [getUserSyncDir(tokenId), path.resolve(SYNC_BASE, 'projects', tokenId)]
        .filter(root => fs.existsSync(root))
        .map(root => fs.realpathSync(root))
      const insideRealRoot = allowedRoots.some((root) => {
        const relative = path.relative(root, real)
        return relative !== '..' && !relative.startsWith('..' + path.sep) && !path.isAbsolute(relative)
      })
      if (!insideRealRoot) return c.json({ error: 'Path outside sync sandbox' }, 403)
      const stat = fs.statSync(real)
      if (stat.isDirectory()) return c.json({ error: 'Is a directory' }, 400)
      const CAP = 512 * 1024  // 512 KB cap for viewer
      const truncated = stat.size > CAP
      const buf = fs.readFileSync(real)
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

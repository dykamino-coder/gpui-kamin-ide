// Caller scope for the project session read tools (INC-2026-0072).
//
// The Bridge server is shared by several tokens (users). A CLI session may read
// only the conversations of its OWN token AND its OWN project, so every path
// here is derived from the authenticated session — never from a tool argument
// except the conversation/agent id, which must be a plain token and is looked up
// only inside the already-resolved directories.
//
// Layout (session-core.ts createSession):
//   <sessionsBase>/<tokenId>[-<sanitizeDirName(cwd)>]/<sessionId>   settings dirs
//   <projectsDir>/<slug(settingsDir)>/<conversationId>.jsonl           transcripts
//   <projectsDir>/<slug>/<conversationId>/subagents/agent-<id>.jsonl   subagents
//   <archiveRoot>/<conversationId>/{transcript.jsonl,meta.json}         durable copy
//
// Hardening mirrors deleteSessionByConversationId (#390): identifiers are
// refused, not sanitised; SESSIONS_BASE is never enumerated; directories and
// files must be plain (lstat, no symlink) and stay inside their real roots.

import fs from 'node:fs'
import fsp from 'node:fs/promises'
import path from 'node:path'

export const CONVERSATION_ID_RE = /^[A-Za-z0-9][A-Za-z0-9_-]{0,127}$/
const AGENT_ID_RE = /^[A-Za-z0-9_-]{1,128}$/
const TOKEN_ID_RE = /^[A-Za-z0-9_-]+$/
/** Archive entries examined per listing — a bound, not a retention policy. */
const ARCHIVE_SCAN_LIMIT = 5000

export interface LiveConversation {
  /** Current and earlier (compact chain) conversation ids of one live session. */
  ids: string[]
  attached: boolean
}

/** Everything a tool may know about its caller. Built by the MCP glue from the
 *  authenticated PtySession; tests build it directly over temp directories. */
export interface CallerContext {
  tokenId: string
  /** The caller's client project directory ('' when the session has none). */
  cwd: string
  settingsDir: string
  /** The caller's own conversation ids (current + compact aliases). */
  selfIds: string[]
  /** Live sessions of the SAME token only. */
  live: LiveConversation[]
  sessionsBase: string
  projectsDir: string
  archiveRoot: string | null
  /** Must be the same function createSession uses to name the token dir. */
  sanitizeDirName: (cwd: string) => string
}

export class ScopeError extends Error {
  constructor(
    message: string,
    readonly outcome: 'not_found' | 'invalid',
  ) {
    super(message)
  }
}

/** One message for unknown, foreign and malformed ids: no existence oracle. */
export const NOT_FOUND = 'Conversation not found in this project'

export const slugOf = (dir: string): string => path.resolve(dir).replace(/[^a-zA-Z0-9]/g, '-')

export function isPlainDir(p: string): boolean {
  try {
    const st = fs.lstatSync(p)
    return st.isDirectory() && !st.isSymbolicLink()
  } catch {
    return false
  }
}

export function isPlainFile(p: string): boolean {
  try {
    const st = fs.lstatSync(p)
    return st.isFile() && !st.isSymbolicLink()
  } catch {
    return false
  }
}

function inside(root: string, file: string): boolean {
  const rel = path.relative(root, file)
  return rel !== '' && rel !== '..' && !rel.startsWith(`..${path.sep}`) && !path.isAbsolute(rel)
}

/** Real path of `file` must stay under the real path of `root`. */
export async function containedIn(root: string, file: string): Promise<boolean> {
  try {
    const [r, f] = await Promise.all([fsp.realpath(root), fsp.realpath(file)])
    return inside(r, f)
  } catch {
    return false
  }
}

export interface ProjectScope {
  /** Slug directory names under projectsDir owned by the caller's project. */
  slugs: Set<string>
}

/** The caller's project directories, by exact name only. */
function projectDirs(ctx: CallerContext): string[] {
  if (!TOKEN_ID_RE.test(ctx.tokenId)) throw new ScopeError('Caller token is not addressable', 'invalid')
  const base = path.resolve(ctx.sessionsBase)
  const expectedName = ctx.tokenId + (ctx.cwd ? '-' + ctx.sanitizeDirName(ctx.cwd) : '')
  const expected = path.join(base, expectedName)
  const dirs: string[] = []
  if (path.dirname(expected) === base && isPlainDir(expected)) dirs.push(expected)
  // A settings dir rebuilt from a slug (reverseSlugToPath legacy branch) can sit
  // in a differently spelled token dir with the SAME slug. Accept the caller's own
  // parent only then, and only with a strict token separator.
  const own = path.dirname(path.resolve(ctx.settingsDir || base))
  const ownName = path.basename(own)
  if (
    own !== expected &&
    path.dirname(own) === base &&
    (ownName === ctx.tokenId || ownName.startsWith(ctx.tokenId + '-')) &&
    slugOf(own) === slugOf(expected) &&
    isPlainDir(own)
  ) {
    dirs.push(own)
  }
  return dirs
}

export function resolveScope(ctx: CallerContext): ProjectScope {
  const slugs = new Set<string>()
  for (const dir of projectDirs(ctx)) {
    let entries: fs.Dirent[]
    try {
      entries = fs.readdirSync(dir, { withFileTypes: true })
    } catch {
      continue
    }
    for (const e of entries) {
      if (!e.isDirectory()) continue
      const settingsDir = path.join(dir, e.name)
      if (isPlainDir(settingsDir)) slugs.add(slugOf(settingsDir))
    }
  }
  return { slugs }
}

export interface TranscriptRef {
  conversationId: string
  file: string
  source: 'live' | 'archive'
  /** Slug dir (live) — subagents hang off it. */
  slugDir: string | null
}

export function validConversationId(id: unknown): id is string {
  return typeof id === 'string' && CONVERSATION_ID_RE.test(id)
}

/** Live transcripts of the caller's project plus archive-only ones. */
export function listTranscripts(ctx: CallerContext, scope: ProjectScope, includeArchived: boolean): TranscriptRef[] {
  const out = new Map<string, TranscriptRef>()
  for (const slug of scope.slugs) {
    const slugDir = path.join(ctx.projectsDir, slug)
    if (!isPlainDir(slugDir)) continue
    let names: string[]
    try {
      names = fs.readdirSync(slugDir)
    } catch {
      continue
    }
    for (const name of names) {
      if (!name.endsWith('.jsonl')) continue
      const id = name.slice(0, -'.jsonl'.length)
      if (!CONVERSATION_ID_RE.test(id) || out.has(id)) continue
      const file = path.join(slugDir, name)
      if (isPlainFile(file)) out.set(id, { conversationId: id, file, source: 'live', slugDir })
    }
  }
  if (includeArchived && ctx.archiveRoot && isPlainDir(ctx.archiveRoot)) {
    let names: string[] = []
    try {
      names = fs.readdirSync(ctx.archiveRoot).slice(0, ARCHIVE_SCAN_LIMIT)
    } catch {
      /* no archive */
    }
    for (const id of names) {
      if (!CONVERSATION_ID_RE.test(id) || out.has(id)) continue
      const ref = archiveRef(ctx, scope, id)
      if (ref) out.set(id, ref)
    }
  }
  return [...out.values()]
}

function archiveRef(ctx: CallerContext, scope: ProjectScope, id: string): TranscriptRef | null {
  if (!ctx.archiveRoot) return null
  const dir = path.join(ctx.archiveRoot, id)
  if (!isPlainDir(dir)) return null
  const metaFile = path.join(dir, 'meta.json')
  const file = path.join(dir, 'transcript.jsonl')
  if (!isPlainFile(metaFile) || !isPlainFile(file)) return null
  try {
    const meta = JSON.parse(fs.readFileSync(metaFile, 'utf8')) as { slug?: unknown }
    if (typeof meta.slug !== 'string' || !scope.slugs.has(meta.slug)) return null
  } catch {
    return null
  }
  return { conversationId: id, file, source: 'archive', slugDir: null }
}

/** Locate one conversation inside the caller's scope or throw NOT_FOUND. */
export async function locateTranscript(
  ctx: CallerContext,
  scope: ProjectScope,
  conversationId: unknown,
): Promise<TranscriptRef> {
  if (!validConversationId(conversationId)) throw new ScopeError(NOT_FOUND, 'invalid')
  for (const slug of scope.slugs) {
    const slugDir = path.join(ctx.projectsDir, slug)
    const file = path.join(slugDir, `${conversationId}.jsonl`)
    if (isPlainDir(slugDir) && isPlainFile(file) && (await containedIn(ctx.projectsDir, file))) {
      return { conversationId, file, source: 'live', slugDir }
    }
  }
  const ref = archiveRef(ctx, scope, conversationId)
  if (ref && ctx.archiveRoot && (await containedIn(ctx.archiveRoot, ref.file))) return ref
  throw new ScopeError(NOT_FOUND, 'not_found')
}

export function normalizeAgentId(agentId: unknown): string | null {
  if (typeof agentId !== 'string') return null
  const id = agentId.replace(/^agent-/, '')
  return AGENT_ID_RE.test(id) ? id : null
}

/** `<slugDir>/<conversationId>/subagents` when every component is a plain dir. */
export function subagentDir(ref: TranscriptRef): string | null {
  if (!ref.slugDir) return null
  const convDir = path.join(ref.slugDir, ref.conversationId)
  const dir = path.join(convDir, 'subagents')
  return isPlainDir(convDir) && isPlainDir(dir) ? dir : null
}

export async function locateSubagent(ctx: CallerContext, ref: TranscriptRef, agentId: unknown): Promise<string> {
  const id = normalizeAgentId(agentId)
  const dir = subagentDir(ref)
  if (!id || !dir) throw new ScopeError('Subagent transcript not found in this conversation', 'not_found')
  const file = path.join(dir, `agent-${id}.jsonl`)
  if (!isPlainFile(file) || !(await containedIn(ctx.projectsDir, file))) {
    throw new ScopeError('Subagent transcript not found in this conversation', 'not_found')
  }
  return file
}

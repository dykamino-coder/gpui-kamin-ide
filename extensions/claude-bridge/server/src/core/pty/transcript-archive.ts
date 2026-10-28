// Durable Bridge-owned copy of a conversation transcript (INC-2026-0054).
//
// Claude Code deletes project transcripts under `~/.claude/projects/` after
// `cleanupPeriodDays` (documented default 30). A conversation that Bridge still
// lists then cannot be resumed at all: `findOrRecreateSettingsDir` finds no
// `<id>.jsonl` in any slug dir, the server drops the requested id and starts a
// fresh session. Database rows survive, but they cannot make a conversation
// resumable — only the transcript file can.
//
// The contract the owner asked for is "resumable until explicitly deleted", and
// raising `cleanupPeriodDays` does not provide it: the setting takes a positive
// number of days, has no unlimited value, and is owned by the CLI, not by us.
// So Bridge keeps its own copy OUTSIDE the swept tree — under the server data
// directory, which is a mounted volume in production — and puts it back when
// the live file is gone. Explicit deletion removes the copy too.
//
// Deliberately NOT done here: no retention timer, no automatic restore of a
// repair `.bak-*`, and never an overwrite of an existing live transcript. A
// stale archive replacing a newer live file would be exactly the data loss this
// module exists to prevent.

import fs from 'fs'
import fsp from 'fs/promises'
import os from 'os'
import path from 'path'

import { getDataDir } from '../stats/database/lifecycle'
import { debugLog, warnLog } from '../logging'
import { readRange } from './jsonl-lines'

/** Conversation ids come from the CLI and from client messages. The id becomes
 *  a directory name, so anything that is not a plain token is refused rather
 *  than sanitized — a "cleaned" id would silently address another archive. */
const SAFE_ID = /^[A-Za-z0-9][A-Za-z0-9._-]{0,127}$/

/** The contract is "resumable until explicitly deleted", so nothing here prunes
 *  by age — that is precisely the behaviour this module exists to undo. The
 *  cost is disk: the archive grows with the conversations the user keeps, and it
 *  holds full transcript text, same as the live files. An operator who does not
 *  want that copy on this host sets `KAMIN_TRANSCRIPT_ARCHIVE=off`; resume then
 *  behaves exactly as before, including losing expired conversations. */
function archivingEnabled(): boolean {
  return (process.env.KAMIN_TRANSCRIPT_ARCHIVE ?? '').toLowerCase() !== 'off'
}

export interface ArchiveMeta {
  /** `~/.claude/projects/<slug>/` the transcript was taken from — restoring
   *  needs it, because the slug is what ties the file to its settingsDir. */
  slug: string
  bytes: number
  archivedAt: string
}

export function archiveRoot(): string {
  return path.join(getDataDir(), 'transcripts')
}

function entryDir(conversationId: string): string | null {
  if (!SAFE_ID.test(conversationId)) return null
  return path.join(archiveRoot(), conversationId)
}

export function archivedTranscriptPath(conversationId: string): string | null {
  const dir = entryDir(conversationId)
  return dir === null ? null : path.join(dir, 'transcript.jsonl')
}

/** A transcript is usable for resume only if it is non-empty JSONL. Checking
 *  the first and last records catches both an empty copy and a torn tail from a
 *  copy that raced a write. Only the first and the last record are read — a
 *  bounded, asynchronous probe, not a whole-file load on the event loop. */
export async function looksResumable(file: string): Promise<boolean> {
  let size: number
  try {
    size = (await fsp.stat(file)).size
  } catch {
    return false
  }
  if (size === 0) return false
  try {
    const first = await firstRecord(file, size)
    const last = await lastRecord(file, size)
    if (first === null || last === null) return false
    JSON.parse(first)
    JSON.parse(last)
    return true
  } catch {
    return false
  }
}

const PROBE = 64 * 1024

const isBlank = (b: number): boolean => b === 0x0a || b === 0x0d || b === 0x20

async function firstRecord(file: string, size: number): Promise<string | null> {
  // Skip leading blank bytes, then take the first line.
  for (let pos = 0; pos < size;) {
    const chunk = await readRange(file, pos, Math.min(size, pos + PROBE))
    let i = 0
    while (i < chunk.length && isBlank(chunk[i]!)) i++
    if (i < chunk.length) return readFromOffset(file, size, pos + i)
    pos += chunk.length
  }
  return null
}

async function readFromOffset(file: string, size: number, from: number): Promise<string | null> {
  const parts: Buffer[] = []
  for (let pos = from; pos < size; pos += PROBE) {
    const chunk = await readRange(file, pos, Math.min(size, pos + PROBE))
    const nl = chunk.indexOf(0x0a)
    if (nl !== -1) {
      parts.push(chunk.subarray(0, nl))
      break
    }
    parts.push(chunk)
  }
  const text = Buffer.concat(parts).toString('utf8')
  return text.trim() ? text : null
}

async function lastRecord(file: string, size: number): Promise<string | null> {
  // Walk back from EOF until a whole non-blank record is enclosed.
  let end = size
  // Trailing newline(s) / blanks are not a record.
  for (;;) {
    const from = Math.max(0, end - PROBE)
    const chunk = await readRange(file, from, end)
    let i = chunk.length
    while (i > 0 && isBlank(chunk[i - 1]!)) i--
    if (i > 0) {
      end = from + i
      break
    }
    if (from === 0) return null
    end = from
  }
  const parts: Buffer[] = []
  for (let stop = end; stop > 0;) {
    const from = Math.max(0, stop - PROBE)
    const chunk = await readRange(file, from, stop)
    const nl = chunk.lastIndexOf(0x0a)
    if (nl !== -1) {
      parts.unshift(chunk.subarray(nl + 1))
      break
    }
    parts.unshift(chunk)
    stop = from
  }
  return Buffer.concat(parts).toString('utf8')
}

/**
 * Keep a durable copy of a live transcript. No-op when the archive already
 * holds at least as many bytes: the transcript only grows, so a shorter live
 * file means a fork or a truncation, and overwriting with it would throw away
 * history the archive still has.
 */
export async function archiveTranscript(jsonlPath: string, conversationId: string): Promise<boolean> {
  if (!archivingEnabled()) return false
  const dir = entryDir(conversationId)
  if (dir === null) return false
  let size: number
  try {
    size = (await fsp.stat(jsonlPath)).size
  } catch {
    return false
  }
  if (size === 0) return false

  const target = path.join(dir, 'transcript.jsonl')
  try {
    const existing = await fsp.stat(target).catch(() => null)
    if (existing && existing.size >= size) return false
    await fsp.mkdir(dir, { recursive: true })
    // tmp + rename: a crash mid-copy must not leave a half transcript that the
    // restore path would happily hand to the CLI.
    // Asynchronous copy: a 100+ MiB transcript copied with copyFileSync froze
    // every session on this server for the duration of the copy.
    const tmp = `${target}.tmp`
    await fsp.copyFile(jsonlPath, tmp)
    await fsp.rename(tmp, target)
    const meta: ArchiveMeta = {
      slug: path.basename(path.dirname(jsonlPath)),
      bytes: size,
      archivedAt: new Date().toISOString(),
    }
    await fsp.writeFile(path.join(dir, 'meta.json'), JSON.stringify(meta, null, 2))
    debugLog('Transcript archived', { conversationId, bytes: size })
    return true
  } catch (e) {
    warnLog('Transcript archive failed', { conversationId, error: String(e) })
    return false
  }
}

export function readArchiveMeta(conversationId: string): ArchiveMeta | null {
  const dir = entryDir(conversationId)
  if (dir === null) return null
  try {
    const raw = JSON.parse(fs.readFileSync(path.join(dir, 'meta.json'), 'utf8')) as Partial<ArchiveMeta>
    if (typeof raw.slug !== 'string' || raw.slug.length === 0) return null
    return { slug: raw.slug, bytes: Number(raw.bytes) || 0, archivedAt: String(raw.archivedAt ?? '') }
  } catch {
    return null
  }
}

/**
 * Put an archived transcript back under `<projectsDir>/<slug>/<id>.jsonl` when
 * the live file is gone. Returns the restored path, or null when there is
 * nothing to restore, the copy fails its integrity check, or a live transcript
 * already exists — the live file always wins.
 */
export async function restoreArchivedTranscript(projectsDir: string, conversationId: string): Promise<string | null> {
  const source = archivedTranscriptPath(conversationId)
  const meta = readArchiveMeta(conversationId)
  if (source === null || meta === null) return null
  if (!fs.existsSync(source)) return null
  if (!(await looksResumable(source))) {
    warnLog('Archived transcript failed its integrity check; not restoring', { conversationId })
    return null
  }

  const targetDir = path.join(projectsDir, meta.slug)
  const target = path.join(targetDir, `${conversationId}.jsonl`)
  try {
    if (fs.existsSync(target)) return null
    await fsp.mkdir(targetDir, { recursive: true })
    const tmp = `${target}.restore-tmp`
    await fsp.copyFile(source, tmp)
    await fsp.rename(tmp, target)
    debugLog('Transcript restored from the Bridge archive', { conversationId, slug: meta.slug })
    return target
  } catch (e) {
    warnLog('Transcript restore failed', { conversationId, error: String(e) })
    return null
  }
}

/** Explicit deletion of a conversation removes its durable copy as well —
 *  otherwise "delete" would leave a resumable transcript behind. */
export function dropArchivedTranscript(conversationId: string): boolean {
  const dir = entryDir(conversationId)
  if (dir === null) return false
  try {
    if (!fs.existsSync(dir)) return false
    fs.rmSync(dir, { recursive: true, force: true })
    debugLog('Archived transcript dropped', { conversationId })
    return true
  } catch (e) {
    warnLog('Dropping the archived transcript failed', { conversationId, error: String(e) })
    return false
  }
}

/** Archive the transcript of a session identified the way the server does it:
 *  by its settingsDir, whose slug is what Claude Code derives the projects/
 *  directory name from. Called when a session ends — a conversation that is
 *  never reopened must survive the sweep too, and the resume path alone would
 *  never have taken a copy of it. */
export async function archiveTranscriptForSession(settingsDir: string, conversationId: string): Promise<boolean> {
  if (!settingsDir || !conversationId) return false
  const slug = path.resolve(settingsDir).replace(/[^a-zA-Z0-9]/g, '-')
  const jsonl = path.join(os.homedir(), '.claude', 'projects', slug, `${conversationId}.jsonl`)
  return archiveTranscript(jsonl, conversationId)
}

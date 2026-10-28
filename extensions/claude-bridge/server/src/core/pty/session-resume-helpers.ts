// Helpers for resuming + deleting Claude CLI sessions by their
// conversationId / settingsDir slug. Extracted from session-core.ts
// (Sprint 5 / Stage E1) — these are the FS-level reverse-engineering bits
// that bridge the gap between the JSONL projects/ slug and our own
// SESSIONS_BASE token-dir layout.

import fs from 'fs'
import fsp from 'fs/promises'
import path from 'path'
import os from 'os'
import { debugLog, warnLog } from '../logging'
import { SESSIONS_BASE } from './session-settings'
import { archiveTranscript, restoreArchivedTranscript } from './transcript-archive'
import { NO_REPAIR, repairJsonlTail, type JsonlRepairStats } from './jsonl-repair'
import { readJsonlLines, readRange } from './jsonl-lines'

export type { JsonlRepairStats } from './jsonl-repair'

/** Cross-platform basename: handles both / and \ separators (server may
 *  run on Linux with Windows paths inside JSONL). */
export function xbasename(p: string): string {
  const parts = p.split(/[/\\]/).filter(Boolean)
  return parts.length > 0 ? parts[parts.length - 1]! : ''
}

/**
 * Find or recreate the settingsDir for a previous conversation.
 *
 * The slug in ~/.claude/projects/ is computed from settingsDir via
 * path.resolve(settingsDir).replace(/[^a-zA-Z0-9]/g, '-').
 *
 * Strategy:
 * 1. Find slug dir containing <conversationId>.jsonl
 * 2. Try to find existing settingsDir in SESSIONS_BASE matching that slug
 * 3. If not found (e.g. /tmp cleared after container restart), reverse-
 *    compute the settingsDir path from the slug and recreate the dir.
 */
export async function findOrRecreateSettingsDir(conversationId: string, tokenId: string): Promise<string | null> {
  const projectsDir = path.join(os.homedir(), '.claude', 'projects')
  const found = await locateSettingsDir(projectsDir, conversationId, tokenId)
  if (found) return found

  // Nothing under projects/ holds this conversation. Claude Code sweeps that
  // tree by `cleanupPeriodDays`, so an idle conversation Bridge still lists can
  // simply have expired. Put our durable copy back and look once more — without
  // it the only outcome is a fresh session and a lost history (INC-2026-0054).
  if ((await restoreArchivedTranscript(projectsDir, conversationId)) === null) return null
  return locateSettingsDir(projectsDir, conversationId, tokenId)
}

async function locateSettingsDir(
  projectsDir: string,
  conversationId: string,
  tokenId?: string,
): Promise<string | null> {
  const jsonlFilename = `${conversationId}.jsonl`

  try {
    if (!fs.existsSync(projectsDir)) return null
    const slugDirs = fs.readdirSync(projectsDir, { withFileTypes: true })
    for (const slugDir of slugDirs) {
      if (!slugDir.isDirectory()) continue
      const jsonlPath = path.join(projectsDir, slugDir.name, jsonlFilename)
      if (!fs.existsSync(jsonlPath)) continue

      // Repair before CLI reads it — without this CLI will pick a
      // synthetic <synthetic> UUID as previous_message_id → 400 forever,
      // or choke on a torn line / broken parentUuid chain left by a
      // mid-write kill (network drop → instant reconnect).
      try {
        await repairJsonl(jsonlPath)
      } catch (e) {
        warnLog('repairJsonl failed', { jsonlPath, error: String(e) })
      }
      // Archive AFTER the repair, so the durable copy is the resumable one.
      await archiveTranscript(jsonlPath, conversationId)

      const slug = slugDir.name
      const existing = scanForSettingsDirBySlug(slug)
      if (existing) return existing

      // Not found — reverse the slug back to a path and recreate.
      // Slug format: -tmp-claude-bridge-sessions-TOKENID-CWD-SESSIONUUID
      // Original path: /tmp/claude-bridge-sessions/TOKENID-CWD/SESSIONUUID
      const reconstructed = reverseSlugToPath(slug, tokenId)
      if (reconstructed) {
        fs.mkdirSync(reconstructed, { recursive: true })
        debugLog('Resume: recreated settingsDir from slug', { slug, path: reconstructed })
        return reconstructed
      }
    }
  } catch {
    /* ignore */
  }
  return null
}

/** Scan SESSIONS_BASE for a directory matching the given slug. */
export function scanForSettingsDirBySlug(slug: string): string | null {
  try {
    if (!fs.existsSync(SESSIONS_BASE)) return null
    const tokenDirs = fs.readdirSync(SESSIONS_BASE, { withFileTypes: true })
    for (const tokenDir of tokenDirs) {
      if (!tokenDir.isDirectory()) continue
      const tokenPath = path.join(SESSIONS_BASE, tokenDir.name)
      const sessionDirs = fs.readdirSync(tokenPath, { withFileTypes: true })
      for (const sessionDir of sessionDirs) {
        if (!sessionDir.isDirectory()) continue
        const candidate = path.join(tokenPath, sessionDir.name)
        const candidateSlug = path.resolve(candidate).replace(/[^a-zA-Z0-9]/g, '-')
        if (candidateSlug === slug) return candidate
      }
    }
  } catch {
    /* ignore */
  }
  return null
}

const slugOf = (p: string): string => path.resolve(p).replace(/[^a-zA-Z0-9]/g, '-')

/** An existing token directory under SESSIONS_BASE whose slug is `slug`. */
function findTokenDirBySlug(slug: string): string | null {
  try {
    for (const entry of fs.readdirSync(SESSIONS_BASE, { withFileTypes: true })) {
      if (entry.isDirectory() && slugOf(path.join(SESSIONS_BASE, entry.name)) === slug) return entry.name
    }
  } catch {
    /* no base yet */
  }
  return null
}

/**
 * Reverse a slug back to a filesystem path. Slug is generated by
 * path.resolve(dir).replace(/[^a-zA-Z0-9]/g, '-') so we can't perfectly
 * reverse, but we know the prefix is SESSIONS_BASE and the trailing
 * 36-char block is a UUID (preserved literally — its hyphens survive the
 * slugify pass since they were already hyphens).
 *
 * After the base prefix the structure is TOKEN_DIR/SESSION_UUID, where
 * TOKEN_DIR = `<tokenId>[-<sanitizeDirName(cwd)>]`. Every candidate has the
 * same slug, so the CLI finds the transcript either way; the choice only
 * decides whether a DUPLICATE token directory appears (INC-2026-0070 side
 * issue: an underscored copy of the token id next to the real one). In order
 * of fidelity:
 * 1. an existing token directory with this exact slug (the usual case: the
 *    token's other sessions keep it alive);
 * 2. the caller's token id restored literally when the slug starts with it
 *    (its hyphens are real), the cwd remainder with -→_ because
 *    sanitizeDirName produces underscores;
 * 3. legacy: the whole token segment with -→_.
 */
export function reverseSlugToPath(slug: string, tokenId?: string): string | null {
  const basePath = path.resolve(SESSIONS_BASE)
  const baseSlug = basePath.replace(/[^a-zA-Z0-9]/g, '-')
  if (!slug.startsWith(baseSlug + '-')) return null

  const remainder = slug.slice(baseSlug.length + 1)
  const uuidMatch = remainder.match(/^(.+)-([0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12})$/)
  if (!uuidMatch) {
    // No UUID found — treat entire remainder as token dir name
    return path.join(SESSIONS_BASE, remainder)
  }
  const tokenDirSlug = uuidMatch[1]!
  const sessionUuid = uuidMatch[2]!
  const existing = findTokenDirBySlug(`${baseSlug}-${tokenDirSlug}`)
  if (existing) return path.join(SESSIONS_BASE, existing, sessionUuid)
  const tokenSlug = tokenId ? tokenId.replace(/[^a-zA-Z0-9]/g, '-') : ''
  if (tokenId && tokenSlug && (tokenDirSlug === tokenSlug || tokenDirSlug.startsWith(tokenSlug + '-'))) {
    const rest = tokenDirSlug.slice(tokenSlug.length + 1)
    const tokenDirName = rest ? `${tokenId}-${rest.replace(/-/g, '_')}` : tokenId
    return path.join(SESSIONS_BASE, tokenDirName, sessionUuid)
  }
  const tokenDirName = tokenDirSlug.replace(/-/g, '_')
  return path.join(SESSIONS_BASE, tokenDirName, sessionUuid)
}

// ---------------------------------------------------------------------------
// JSONL repair on resume
// ---------------------------------------------------------------------------

/**
 * Repair a Claude CLI conversation JSONL before --resume. Bounded to the
 * transcript tail and fully asynchronous — see `jsonl-repair.ts` for the
 * corruption model and why the whole file is never read here.
 */
export async function repairJsonl(jsonlPath: string): Promise<JsonlRepairStats> {
  return repairJsonlTail(jsonlPath)
}

/**
 * Найти самый свежий файл ЦЕПОЧКИ беседы в slug-каталоге. Резюм по id из
 * клиентского стора мог отстать от реального tip (watcher остановлен до смерти
 * CLI, гонка compact-links, двойной писатель) — тогда CLI форкает от старой
 * точки и хвост «пропадает» в осиротевшем файле. Watcher всё равно тейлит
 * самый свежий файл каталога — резюмим его же.
 *
 * Проба «кандидат — потомок X»: форк копирует историю, значит либо есть
 * пересечение uuid с X, либо кандидат начинается не позже X. Чужая беседа в
 * том же каталоге стартует позже и uuid не разделяет. Не уверены — null
 * (fail-safe: резюмим что просили).
 */
export async function resolveNewestInChain(settingsDir: string, conversationId: string): Promise<string | null> {
  try {
    const slug = path.resolve(settingsDir).replace(/[^a-zA-Z0-9]/g, '-')
    const dir = path.join(os.homedir(), '.claude', 'projects', slug)
    let names: string[]
    try {
      names = await fsp.readdir(dir)
    } catch {
      return null
    }
    const files: { id: string; mtime: number; size: number }[] = []
    for (const f of names) {
      if (!f.endsWith('.jsonl')) continue
      try {
        const st = await fsp.stat(path.join(dir, f))
        if (st.size > 0) files.push({ id: f.slice(0, -'.jsonl'.length), mtime: st.mtimeMs, size: st.size })
      } catch {
        /* vanished */
      }
    }
    files.sort((a, b) => b.mtime - a.mtime)
    const newest = files[0]
    if (!newest) return null
    if (newest.id === conversationId) return null // уже tip
    const basePath = path.join(dir, `${conversationId}.jsonl`)
    if (!fs.existsSync(basePath)) return null // базы нет — не с чем сверять
    // Первые `max` uuid — потоково, без чтения всего файла: проба нужна только
    // голове, а целый 100+ МБ кандидат раньше читался синхронно на каждом резюме.
    const uuidsOf = async (p: string, max: number): Promise<Set<string>> => {
      const out = new Set<string>()
      const ac = new AbortController()
      try {
        for await (const line of readJsonlLines(p, { signal: ac.signal })) {
          const m = line.text.match(/"uuid":"([0-9a-f-]{36})"/)
          if (m?.[1]) out.add(m[1])
          if (out.size >= max) break
        }
      } catch {
        /* unreadable — what we have is the answer */
      }
      return out
    }
    const firstTs = async (p: string): Promise<string | null> => {
      const head = (await readRange(p, 0, 64 * 1024)).toString('utf8')
      const m = head.match(/"timestamp":"([^"]+)"/)
      return m?.[1] ?? null
    }
    const candPath = path.join(dir, `${newest.id}.jsonl`)
    const baseUuids = await uuidsOf(basePath, 200)
    const candUuids = await uuidsOf(candPath, 200)
    let related = false
    for (const u of candUuids) {
      if (baseUuids.has(u)) {
        related = true
        break
      }
    }
    if (!related) {
      const bt = await firstTs(basePath)
      const ct = await firstTs(candPath)
      related = bt !== null && ct !== null && ct <= bt
    }
    return related ? newest.id : null
  } catch (e) {
    warnLog('resolveNewestInChain failed', { conversationId, error: String(e) })
    return null
  }
}

/**
 * Repair the transcript for a session that resumes via reuseSettingsDir
 * (model/effort restart). The projects/ slug is derived from the settingsDir
 * the same way CLI derives it, so we can locate the JSONL without scanning.
 */
export async function repairTranscriptForResume(
  settingsDir: string,
  conversationId: string,
): Promise<JsonlRepairStats> {
  const slug = path.resolve(settingsDir).replace(/[^a-zA-Z0-9]/g, '-')
  const jsonlPath = path.join(os.homedir(), '.claude', 'projects', slug, `${conversationId}.jsonl`)
  if (!fs.existsSync(jsonlPath)) return NO_REPAIR
  try {
    const stats = await repairJsonl(jsonlPath)
    await archiveTranscript(jsonlPath, conversationId)
    return stats
  } catch (e) {
    warnLog('repairTranscriptForResume failed', { jsonlPath, error: String(e) })
    return NO_REPAIR
  }
}

/** Read the most recent real assistant model without loading a large transcript.
 * An explicit --model overrides Claude Code's restored model on --resume, so
 * the server must recover it before applying a new-session default. */
export function lastModelFromJsonlTail(jsonlPath: string): string | null {
  return lastModelEntryFromJsonlTail(jsonlPath)?.model ?? null
}

/** Как `lastModelFromJsonlTail`, но с меткой времени записи: выбор модели,
 *  сделанный ПОЗЖЕ последнего ответа, обязан пережить респаун (INC-2026-0053). */
export function lastModelEntryFromJsonlTail(jsonlPath: string): { model: string; timestamp: string } | null {
  let fd: number | undefined
  try {
    fd = fs.openSync(jsonlPath, 'r')
    const size = fs.fstatSync(fd).size
    const maxBytes = 8 * 1024 * 1024
    const start = Math.max(0, size - maxBytes)
    const buffer = Buffer.alloc(size - start)
    let read = 0
    while (read < buffer.length) {
      const count = fs.readSync(fd, buffer, read, buffer.length - read, start + read)
      if (count === 0) break
      read += count
    }
    const text = buffer.subarray(0, read).toString('utf8')
    const lines = text.slice(start === 0 ? 0 : text.indexOf('\n') + 1).split('\n')
    for (let i = lines.length - 1; i >= 0; i--) {
      let entry: { type?: string; model?: unknown; timestamp?: unknown; message?: { model?: unknown } }
      try {
        entry = JSON.parse(lines[i]!)
      } catch {
        continue
      }
      if (entry.type !== 'assistant') continue
      const model = entry.message?.model ?? entry.model
      if (typeof model === 'string' && model && model !== '<synthetic>') {
        return { model, timestamp: typeof entry.timestamp === 'string' ? entry.timestamp : '' }
      }
    }
  } catch {
    /* missing or unreadable transcript: let Claude Code restore it */
  } finally {
    if (fd !== undefined) fs.closeSync(fd)
  }
  return null
}

export function lastModelForResume(settingsDir: string, conversationId: string): string | null {
  return lastModelEntryForResume(settingsDir, conversationId)?.model ?? null
}

export function lastModelEntryForResume(
  settingsDir: string,
  conversationId: string,
): { model: string; timestamp: string } | null {
  const slug = path.resolve(settingsDir).replace(/[^a-zA-Z0-9]/g, '-')
  return lastModelEntryFromJsonlTail(path.join(os.homedir(), '.claude', 'projects', slug, `${conversationId}.jsonl`))
}

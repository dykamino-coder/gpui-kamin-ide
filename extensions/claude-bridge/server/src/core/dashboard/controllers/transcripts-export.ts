// ============================================================================
// Bulk transcript export — list + byte-range fetch of every JSONL a token owns
// ============================================================================
//
// The Bridge settings "Download all session logs" button drives this: it lists
// the caller's transcripts, then pulls each file in byte-range batches (so a
// multi-MB transcript never rides one request and can't hit the single-shot
// download timeout). The client writes each to <chosen-folder>/<id>.jsonl.
//
// SECURITY: every response is scoped to the CALLER's own token. A Bridge session
// always runs with cwd = its settingsDir (SESSIONS_BASE/<tokenId[-cwd]>/<sid>), so
// the CLI's project-slug for it — the `~/.claude/projects/<slug>/` dir holding its
// JSONL — is that path slugified, which embeds the tokenId VERBATIM (a UUID is
// alphanumeric, so it survives). We therefore enumerate `projects/` DIRECTLY,
// matching dirs whose name contains `-<tokenId>-`: that finds every transcript the
// token ever wrote — including ones whose settingsDir was since reaped, which a
// walk over the (surviving) settingsDirs would miss (the "only 22 of 177" bug).
// The `-…-` boundaries + a full UUID keep it from matching any other token. The
// range endpoint re-verifies ownership before reading a byte (IDOR class closed
// elsewhere in the 2026-07 audit).

import type { Context, Hono } from 'hono'
import fs from 'fs'
import fsp from 'fs/promises'
import path from 'path'
import os from 'os'
import { resolveTokenSync } from '../../auth/tokens'

const PROJECTS_DIR = path.join(os.homedir(), '.claude', 'projects')
/** Hard ceiling on a single range request, so a bad `len` can't ask the server
 *  to buffer an arbitrary slice. The client batches well under this. */
const MAX_RANGE_LEN = 8 * 1024 * 1024

/** Resolve the caller's tokenId. Prefers the middleware-stamped value (dashboard
 *  auth enabled), falls back to resolving the Bearer header directly so the
 *  endpoint also works when dashboard auth is off. Null → not a valid token. */
function callerTokenId(c: Context): string | null {
  const fromCtx = c.get('apiTokenId') as string | undefined
  if (fromCtx) return fromCtx
  const auth = c.req.header('authorization')
  if (!auth?.startsWith('Bearer ')) return null
  return resolveTokenSync(auth.slice(7))?.tokenId ?? null
}

/** A project-slug dir belongs to this token iff the slugified settingsDir path it
 *  was built from contained the tokenId — i.e. the dir name has `-<tokenId>-`. The
 *  surrounding `-` (path separators, always present around the id in the slug) plus
 *  a full 36-char UUID make this exact: no other token's dir can match. */
function ownsProjectDir(name: string, tokenId: string): boolean {
  return name.includes(`-${tokenId}-`)
}

/** Every JSONL the token owns → conversationId, byte size, and absolute path.
 *  Scans `projects/` directly (not the settingsDirs, which get reaped) so orphaned
 *  transcripts still surface. Deduped by conversationId. */
async function listTokenTranscripts(
  tokenId: string,
): Promise<Map<string, { size: number; filePath: string }>> {
  const out = new Map<string, { size: number; filePath: string }>()
  let dirs: fs.Dirent[]
  try {
    dirs = await fsp.readdir(PROJECTS_DIR, { withFileTypes: true })
  } catch {
    return out
  }
  for (const d of dirs) {
    if (!d.isDirectory() || !ownsProjectDir(d.name, tokenId)) continue
    const slugDir = path.join(PROJECTS_DIR, d.name)
    let files: string[]
    try {
      files = await fsp.readdir(slugDir)
    } catch {
      continue
    }
    for (const f of files) {
      if (!f.endsWith('.jsonl')) continue
      const conversationId = f.slice(0, -'.jsonl'.length)
      if (out.has(conversationId)) continue
      const filePath = path.join(slugDir, f)
      try {
        const st = await fsp.stat(filePath)
        if (st.isFile()) out.set(conversationId, { size: st.size, filePath })
      } catch { /* vanished between readdir and stat */ }
    }
  }
  return out
}

// Short-TTL cache of the (now `projects/`-wide) enumeration: the range endpoint
// re-checks ownership on EVERY 4MB batch, so a multi-MB transcript would otherwise
// re-scan the whole projects tree dozens of times. 10s is far shorter than an
// export runs but long enough to serve a file's batches from one scan; the set
// doesn't change mid-export.
const LIST_TTL_MS = 10_000
const listCache = new Map<string, { at: number; map: Map<string, { size: number; filePath: string }> }>()

async function listTokenTranscriptsCached(tokenId: string): Promise<Map<string, { size: number; filePath: string }>> {
  const now = Date.now()
  const hit = listCache.get(tokenId)
  if (hit && now - hit.at < LIST_TTL_MS) return hit.map
  const map = await listTokenTranscripts(tokenId)
  listCache.set(tokenId, { at: now, map })
  while (listCache.size > 50) {
    const oldest = listCache.keys().next().value
    if (oldest === undefined) break
    listCache.delete(oldest)
  }
  return map
}

export function registerTranscriptExportRoutes(api: Hono): void {
  // GET /api/dashboard/transcripts — list every transcript the caller's token
  // owns, so the client can loop over them. `{ conversationId, size }` only.
  api.get('/api/dashboard/transcripts', async (c) => {
    const tokenId = callerTokenId(c)
    if (!tokenId) return c.json({ error: 'Authentication required' }, 401)
    const map = await listTokenTranscriptsCached(tokenId)
    const transcripts = [...map.entries()].map(([conversationId, v]) => ({ conversationId, size: v.size }))
    return c.json({ transcripts })
  })

  // GET /api/dashboard/transcripts/:conversationId?offset=N&len=M — one byte
  // range of a transcript the caller owns. Batching lives on the client: it
  // walks offset→size in MAX_RANGE_LEN-bounded requests and appends each slice.
  api.get('/api/dashboard/transcripts/:conversationId', async (c) => {
    const tokenId = callerTokenId(c)
    if (!tokenId) return c.json({ error: 'Authentication required' }, 401)
    const conversationId = c.req.param('conversationId')
    // Ownership check BEFORE touching the filesystem for reads: only files the
    // token owns are ever served.
    const owned = (await listTokenTranscriptsCached(tokenId)).get(conversationId)
    if (!owned) return c.json({ error: 'Not found' }, 404)

    const offset = Math.max(0, Number.parseInt(c.req.query('offset') ?? '0', 10) || 0)
    const reqLen = Number.parseInt(c.req.query('len') ?? String(MAX_RANGE_LEN), 10)
    const len = Math.min(MAX_RANGE_LEN, Number.isFinite(reqLen) && reqLen > 0 ? reqLen : MAX_RANGE_LEN)
    if (offset >= owned.size) {
      return new Response('', { status: 200, headers: { 'X-Total-Size': String(owned.size) } })
    }
    const end = Math.min(owned.size, offset + len)
    const fd = await fsp.open(owned.filePath, 'r')
    try {
      const buf = Buffer.alloc(end - offset)
      const { bytesRead } = await fd.read(buf, 0, buf.length, offset)
      return new Response(buf.subarray(0, bytesRead), {
        status: 200,
        headers: { 'Content-Type': 'application/x-ndjson', 'X-Total-Size': String(owned.size) },
      })
    } finally {
      await fd.close()
    }
  })
}

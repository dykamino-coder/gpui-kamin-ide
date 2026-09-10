// Incremental JSONL → DuckDB sweeper.
//
// Reads only the newly-appended bytes of each `~/.claude/projects/<slug>/
// <conv>.jsonl` since the last scan (offset stored in `jsonl_offsets`),
// inserts every meaningful entry into `jsonl_events` keyed by uuid
// (idempotent — INSERT … ON CONFLICT DO NOTHING). Survives:
//  - JSONL deletion (we already have the rows in DuckDB).
//  - /branch rollback (existing uuids stay; new branch's uuids are added).
//  - File truncation/compaction (size < last_offset → reset offset to 0).
//
// Each user message is later annotated with the model the next assistant
// reply used. We do this in two passes per file: first INSERT … ON
// CONFLICT DO NOTHING every entry, then UPDATE jsonl_events SET model =
// <derived> WHERE type='user' AND model IS NULL — derivation walks
// parentUuid chain forward to the next assistant.
//
// Runs:
//  - Once at server boot (full catch-up).
//  - Every 5 min on a timer (catches dormant tabs / external `claude`
//    invocations that don't go through bridge's live watcher).
//  - Optionally also driven by JsonlWatcher per-tail event (live mode).
//
// DuckDB notes:
//  - No `db.prepare()` reuse — we build SQL strings + bind args per call.
//    DuckDB's plan cache amortises this; the pre-DuckDB SQLite handle had
//    explicit prepared statements but DuckDB Neo's API isn't structured
//    around long-lived statements. For batch inserts we wrap in a single
//    BEGIN / COMMIT so the 500-row buffer flushes atomically.
//  - Every WRITE goes through `withStatsWrite` (database/write-lock.ts).
//    Two concurrent write transactions can fail each other's COMMIT, and a
//    failed COMMIT whose rollback touches an index is a FatalException that
//    kills the process — this is how 6.3.131 died. Reads stay parallel.
//  - A batch is deduplicated by uuid before INSERT (jsonl-batch.ts):
//    ON CONFLICT only covers rows already committed, and a duplicate inside
//    one transaction surfaces at COMMIT — the same fatal path.

import fs from 'fs'
import fsp from 'fs/promises'
import path from 'path'
import os from 'os'
import type { DuckDBConnection } from '@duckdb/node-api'
import { getDb, getDbByIndex } from './database/lifecycle'
import { withStatsWrite } from './database/write-lock'
import { dedupeByUuid } from './jsonl-batch'
import { debugLog, warnLog } from '../logging'

const PROJECTS_DIR = path.join(os.homedir(), '.claude', 'projects')

interface UsageBlock {
  input_tokens?: number
  output_tokens?: number
  cache_read_input_tokens?: number
  cache_creation_input_tokens?: number
}

/** All time-grouping (heatmap, peakHour, streaks, daily timeseries) is
 *  done on the client in the user's local timezone — we only validate
 *  the timestamp here and store it as-is (UTC ISO from CLI). The legacy
 *  `day`/`hour` columns stay NULL on new inserts; SQL aggregates use
 *  `strftime(timestamp::TIMESTAMP, '%Y-%m-%dT%H')` instead so everything
 *  is UTC-honest regardless of the container's TZ. */
function isValidIso(tsStr: string): boolean {
  const t = Date.parse(tsStr)
  return Boolean(t) && !Number.isNaN(t)
}

const SQL_INSERT_EVENT = `
  INSERT INTO jsonl_events
    (uuid, session_id, timestamp, type, model, input_tokens, output_tokens,
     cache_read_tokens, cache_write_tokens, tool_name, message_id)
  VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
  ON CONFLICT (uuid) DO NOTHING
`

const SQL_GET_OFFSET = `
  SELECT last_offset, last_size FROM jsonl_offsets WHERE file_path = ?
`

const SQL_UPSERT_OFFSET = `
  INSERT INTO jsonl_offsets (file_path, last_offset, last_size, last_scanned_at)
  VALUES (?, ?, ?, ?)
  ON CONFLICT (file_path) DO UPDATE SET
    last_offset = excluded.last_offset,
    last_size = excluded.last_size,
    last_scanned_at = excluded.last_scanned_at
`

// Backfill model for user rows from the next assistant in the same session.
const SQL_FILL_USER_MODELS = `
  UPDATE jsonl_events
  SET model = (
    SELECT a.model FROM jsonl_events a
    WHERE a.session_id = jsonl_events.session_id
      AND a.type = 'assistant'
      AND a.timestamp >= jsonl_events.timestamp
      AND a.model IS NOT NULL AND a.model != '<synthetic>'
    ORDER BY a.timestamp ASC LIMIT 1
  )
  WHERE session_id = ? AND type = 'user' AND (model IS NULL OR model = '')
`

const SQL_LOOKUP_TOKEN = `
  SELECT token_id, user_name, deleted FROM session_tokens WHERE session_id = ?
`

// Apply token_id, user_name + is_subagent flag to all rows of a session.
// COALESCE with empty-string check heals legacy rows that got token_id=''
// from the old backfill path.
//
// The WHERE clause is CRITICAL for file size: DuckDB writes a new row version
// for every row an UPDATE touches, even when the value is unchanged, and those
// versions are only reclaimed on CHECKPOINT. The old unconditional
// `WHERE session_id = ?` rewrote every row of every session on every 5-minute
// sweep (thousands of files, forever) → the primary driver of proxy.duckdb
// bloating to 1.7 GB. The extra predicate makes the UPDATE a true no-op (0 rows
// matched) once a session is fully attributed, so steady-state sweeps stop
// churning. Bind order: SET(?,?,?,?) then WHERE(session_id, is_subagent, parent).
const SQL_APPLY_TOKEN_AND_SUBAGENT = `
  UPDATE jsonl_events
  SET token_id = CASE WHEN token_id IS NULL OR token_id = '' THEN ? ELSE token_id END,
      user_name = CASE WHEN user_name IS NULL OR user_name = '' THEN ? ELSE user_name END,
      is_subagent = ?,
      parent_session_id = COALESCE(parent_session_id, ?)
  WHERE session_id = ?
    AND (
      token_id IS NULL OR token_id = ''
      OR user_name IS NULL OR user_name = ''
      OR is_subagent IS DISTINCT FROM ?
      OR (parent_session_id IS NULL AND ? IS NOT NULL)
    )
`

// Auto-create a session_tokens row for a real bridge JSONL on first sight.
// Idempotent — won't overwrite existing rows (e.g. the onConversationId
// callback may have inserted a deleted=0 row first).
const SQL_ENSURE_SESSION_TOKEN = `
  INSERT INTO session_tokens
    (session_id, token_id, user_name, created_at, deleted)
  VALUES (?, ?, ?, ?, 0)
  ON CONFLICT (session_id) DO NOTHING
`

// Heal legacy rows where token_id was stored as '' but we now know the
// real UUID via the slug. Doesn't touch deleted flag or user_name.
const SQL_REFRESH_SESSION_TOKEN = `
  UPDATE session_tokens
  SET token_id = ?
  WHERE session_id = ? AND (token_id IS NULL OR token_id = '')
`

// Read pre-existing session metadata so the incremental sweep can resume
// accumulation (peak-in-segment, compact count, etc).
const SQL_GET_SESSION_META = `
  SELECT title, last_context, compact_boundaries, pre_boundary_snapshots
  FROM session_tokens WHERE session_id = ?
`

// Persist accumulated metadata so the dashboard sessions endpoint can
// answer with pure SQL (no JSONL file IO at request time).
const SQL_UPDATE_SESSION_META = `
  UPDATE session_tokens
  SET title = COALESCE(?, title),
      last_context = ?,
      compact_boundaries = ?,
      pre_boundary_snapshots = ?,
      context_tokens = ?
  WHERE session_id = ?
`

interface JsonlFileInfo {
  path: string
  isSubagent: boolean
  parentSessionId: string | null
}

async function* listJsonlFiles(): AsyncGenerator<JsonlFileInfo> {
  if (!fs.existsSync(PROJECTS_DIR)) return
  const slugs = await fsp.readdir(PROJECTS_DIR, { withFileTypes: true })
  for (const slug of slugs) {
    if (!slug.isDirectory()) continue
    const slugDir = path.join(PROJECTS_DIR, slug.name)
    let entries: fs.Dirent[]
    try { entries = await fsp.readdir(slugDir, { withFileTypes: true }) } catch { continue }
    for (const e of entries) {
      if (e.isFile() && e.name.endsWith('.jsonl')) {
        yield { path: path.join(slugDir, e.name), isSubagent: false, parentSessionId: null }
      } else if (e.isDirectory()) {
        // Subagent JSONLs live under <slug>/<convId>/subagents/*.jsonl —
        // CLI's Task tool spawns each in its own file. We still want
        // their tokens counted, but flagged so Stats can exclude them
        // from "Sessions" cardinality (they're not standalone chats).
        const parentSessionId = e.name
        const subDir = path.join(slugDir, e.name, 'subagents')
        let subFiles: string[]
        try { subFiles = await fsp.readdir(subDir) } catch { continue }
        for (const f of subFiles) {
          if (f.endsWith('.jsonl')) {
            yield { path: path.join(subDir, f), isSubagent: true, parentSessionId }
          }
        }
      }
    }
  }
}

/** Extract token attribution from a JSONL slug directory name.
 *
 *  CLI builds slugs as `path.resolve(settingsDir).replace(/[^a-zA-Z0-9]/g, '-')`.
 *  Bridge spawns the CLI with `cwd = SESSIONS_BASE/<tokenUUID><cwdSuffix>/<sessionUUID>/`,
 *  so the slug always contains the token's UUID (api_tokens.id) verbatim.
 *  We probe known token UUIDs against the slug — first match wins.
 *
 *  Returns the matching {tokenId, userName} or null when the slug isn't
 *  from a bridge-managed CLI invocation (e.g. user ran `claude` directly
 *  in some random directory). Non-matching JSONL still gets ingested into
 *  jsonl_events but won't be attributed to any user. */
let cachedTokens: Array<{ id: string; name: string }> | null = null
let cachedTokensAt = 0
async function tokenInfoFromSlug(db: DuckDBConnection, slug: string): Promise<{ tokenId: string; userName: string } | null> {
  const now = Date.now()
  if (!cachedTokens || now - cachedTokensAt > 30_000) {
    try {
      const rows = (await db.runAndReadAll('SELECT id, name FROM api_tokens')).getRowObjects() as Array<{ id: string; name: string }>
      cachedTokens = rows
      cachedTokensAt = now
    } catch { cachedTokens = [] }
  }
  for (const t of cachedTokens) {
    // UUID is hex+dashes — slugifying turns dashes into dashes, no
    // change. Substring match is safe: UUIDs collide with arbitrary
    // text only at astronomically low probability.
    if (slug.includes(t.id)) return { tokenId: t.id, userName: t.name }
  }
  return null
}

/** Files being processed right now. Two passes over the same file (a
 *  live-mode trigger landing during the bulk sweep) would read the same
 *  unswept bytes and race on the offset row; the second caller shares the
 *  first pass instead, and the next sweep picks up whatever was appended. */
const inFlight = new Map<string, Promise<{ inserted: number }>>()

async function processOneFile(info: JsonlFileInfo, dedicatedDb?: DuckDBConnection): Promise<{ inserted: number }> {
  const active = inFlight.get(info.path)
  if (active) return active
  const run = processOneFileUnguarded(info, dedicatedDb).finally(() => {
    inFlight.delete(info.path)
  })
  inFlight.set(info.path, run)
  return run
}

async function processOneFileUnguarded(info: JsonlFileInfo, dedicatedDb?: DuckDBConnection): Promise<{ inserted: number }> {
  const filePath = info.path
  // Use dedicated connection if caller provided one (sweeper worker pool
  // — one conn per worker so parallel files don't share a conn). Falls
  // back to round-robin pool for ad-hoc callers.
  const db = dedicatedDb ?? await getDb()
  // Single-statement write on this connection, serialised with every other
  // writer of the stats DB (see header: a failed COMMIT is fatal).
  const write = (sql: string, args: unknown[]) => withStatsWrite(() => db.run(sql, args as any[]))
  let stat: fs.Stats
  try { stat = await fsp.stat(filePath) } catch { return { inserted: 0 } }
  const size = stat.size

  const offsetRows = (await db.runAndReadAll(SQL_GET_OFFSET, [filePath])).getRowObjects() as Array<{ last_offset: bigint | number; last_size: bigint | number }>
  const offsetRow = offsetRows[0]
  let startByte = offsetRow ? Number(offsetRow.last_offset ?? 0) : 0
  if (offsetRow && Number(offsetRow.last_size ?? 0) > size) {
    // File shrank — most likely truncate / external rewrite. Re-scan.
    startByte = 0
  }
  const sessionId = path.basename(filePath, '.jsonl')

  // Resolve token attribution from (a) existing session_tokens row, or
  // (b) slug-derived UUID match. Slug is authoritative when session_tokens
  // is empty/legacy (token_id=''). For real bridge JSONLs (subagent files
  // included), auto-INSERT a session_tokens row so the soft-delete model
  // works uniformly.
  const slug = path.basename(path.dirname(filePath))
  const slugInfo = await tokenInfoFromSlug(db, slug)
  const tokenRows = (await db.runAndReadAll(SQL_LOOKUP_TOKEN, [sessionId])).getRowObjects() as Array<{ token_id: string | null; user_name: string | null; deleted: number | bigint | null }>
  const tokenRow = tokenRows[0]

  // Prefer existing session_tokens.user_name when set; fall back to slug.
  const userName = (tokenRow?.user_name && tokenRow.user_name.length > 0)
    ? tokenRow.user_name
    : slugInfo?.userName ?? null
  // For token_id, slug UUID is authoritative — heal '' rows.
  const tokenId = slugInfo?.tokenId
    ?? (tokenRow?.token_id && tokenRow.token_id.length > 0 ? tokenRow.token_id : null)

  // Auto-create the session_tokens row for any non-subagent JSONL whose
  // slug carries a known token UUID. Subagent files share the parent
  // session's mapping — they don't get their own row. Skipping subagents
  // also keeps Sessions cardinality clean.
  if (slugInfo && !info.isSubagent) {
    if (!tokenRow) {
      await write(SQL_ENSURE_SESSION_TOKEN, [sessionId, slugInfo.tokenId, slugInfo.userName, new Date().toISOString()])
    } else if (!tokenRow.token_id || tokenRow.token_id.length === 0) {
      // Heal the legacy '' rows that the old backfill produced.
      await write(SQL_REFRESH_SESSION_TOKEN, [slugInfo.tokenId, sessionId])
    }
  }

  const subFlag = info.isSubagent ? 1 : 0
  await write(SQL_APPLY_TOKEN_AND_SUBAGENT, [tokenId, userName, subFlag, info.parentSessionId, sessionId, subFlag, info.parentSessionId])

  if (startByte >= size) {
    // Only touch jsonl_offsets when the position actually moved — an
    // unconditional upsert on every idle sweep is more needless MVCC churn.
    if (!offsetRow || Number(offsetRow.last_offset ?? -1) !== size || Number(offsetRow.last_size ?? -1) !== size) {
      await write(SQL_UPSERT_OFFSET, [filePath, size, size, new Date().toISOString()])
    }
    return { inserted: 0 }
  }
  // Buffered read of the byte range. We previously used
  // fs.createReadStream + readline async-iterator, which hung
  // intermittently on Node 22 in our Linux container — the iterator
  // never yielded the first line for some files. Files are <100KB so
  // buffering is cheap and avoids the streaming bug entirely.
  const fd = await fsp.open(filePath, 'r')
  let chunkText: string
  try {
    const buf = Buffer.alloc(size - startByte)
    await fd.read(buf, 0, buf.length, startByte)
    chunkText = buf.toString('utf8')
  } finally {
    await fd.close()
  }
  const lines = chunkText.split('\n')

  // DuckDB transaction wrapper — flush a buffer of N rows atomically.
  // The pre-DuckDB SQLite path used `db.transaction(fn)(rows)` for the
  // same effect; here we explicitly BEGIN / COMMIT around the per-row
  // INSERT … ON CONFLICT DO NOTHING so the 500-row buffer is one unit.
  // (Appender API would be faster but bypasses ON CONFLICT — see header
  // comment for tradeoff. Idempotency wins for the incremental path.)
  async function insertMany(buffered: unknown[][]): Promise<void> {
    const rows = dedupeByUuid(buffered)
    if (rows.length === 0) return
    await withStatsWrite(async () => {
      await db.run('BEGIN')
      try {
        for (const args of rows) {
          await db.run(SQL_INSERT_EVENT, args as any[])
        }
        await db.run('COMMIT')
      } catch (err) {
        try { await db.run('ROLLBACK') } catch { /* ignore */ }
        throw err
      }
    })
  }

  // Resume per-session metadata accumulators from session_tokens so the
  // incremental scan can pick up where the previous sweep left off.
  // Stored columns become the single source of truth for the dashboard
  // sessions endpoint — no JSONL re-reading at request time.
  const metaRows = (await db.runAndReadAll(SQL_GET_SESSION_META, [sessionId])).getRowObjects() as Array<{
    title: string | null
    last_context: number | bigint | null
    compact_boundaries: number | bigint | null
    pre_boundary_snapshots: string | null
  }>
  const metaRow = metaRows[0]
  let metaTitle: string | null = metaRow?.title ?? null
  let metaCompactBoundaries: number = Number(metaRow?.compact_boundaries ?? 0)
  let metaPreBoundarySnapshots: number[] = (() => {
    try { return JSON.parse(metaRow?.pre_boundary_snapshots ?? '[]') as number[] } catch { return [] }
  })()
  // peakInSegment continues across sweep runs — was last_context from
  // previous sweep (peak so far in the current segment).
  let metaPeakInSegment: number = Number(metaRow?.last_context ?? 0)
  let metaTitleChanged = false

  let inserted = 0
  let buffer: unknown[][] = []
  for (const line of lines) {
    if (!line || !line.trim()) continue
    if (line.includes('"type":"file-history-snapshot"')) continue
    let e: any
    try { e = JSON.parse(line) } catch { continue }
    // Capture per-session metadata from auxiliary entry types — they
    // don't go into jsonl_events but drive the sessions list UI.
    if (e?.type === 'ai-title' && typeof e.aiTitle === 'string') {
      if (e.aiTitle !== metaTitle) { metaTitle = e.aiTitle; metaTitleChanged = true }
      continue
    }
    if (e?.type === 'custom-title' && typeof e.customTitle === 'string') {
      if (!metaTitle && e.customTitle) { metaTitle = e.customTitle; metaTitleChanged = true }
      continue
    }
    if (e?.type === 'system' && e.subtype === 'compact_boundary') {
      metaCompactBoundaries++
      if (metaPeakInSegment > 0) metaPreBoundarySnapshots.push(metaPeakInSegment)
      metaPeakInSegment = 0
      continue
    }
    const type = e.type
    if (type !== 'user' && type !== 'assistant') continue
    const uuid = e.uuid
    if (typeof uuid !== 'string') continue
    const tsStr = e.timestamp
    if (typeof tsStr !== 'string' || !isValidIso(tsStr)) continue

    if (type === 'user') {
      // Skip auto-injected tool_result + meta + caveat/interrupt markers.
      if (e.isMeta) continue
      const c = e.message?.content
      const isToolResult = Array.isArray(c) && c.some((b: any) => b?.type === 'tool_result')
      if (isToolResult) {
        // Tool result — keep as 'tool_result' so we can count tool work.
        const toolUseId = Array.isArray(c)
          ? (c.find((b: any) => b?.type === 'tool_result') as any)?.tool_use_id
          : undefined
        buffer.push([uuid, sessionId, tsStr, 'tool_result', null, 0, 0, 0, 0, null, toolUseId ?? null])
        continue
      }
      const text = typeof c === 'string'
        ? c
        : Array.isArray(c) ? c.filter((b: any) => b?.type === 'text').map((b: any) => b.text || '').join('\n') : ''
      const trimmed = text.trim()
      if (!trimmed) continue
      if (trimmed.startsWith('[Caveat:')) continue
      if (trimmed === '[Request interrupted by user]') continue
      if (trimmed === '[Request interrupted by user for tool use]') continue
      if (trimmed === '[Skipped by user]') continue
      // Fallback title — first real user message, truncated. CLI only
      // generates ai-title via Haiku side-quest for non-trivial chats,
      // and OSC-title race may miss the UPDATE before cliConversationId
      // is set. Without this, the dashboard sessions list shows blank
      // for short test chats (e.g. "test", "авпавпва") forever.
      if (!metaTitle) {
        metaTitle = trimmed.slice(0, 60).replace(/\s+/g, ' ').trim()
        if (metaTitle) metaTitleChanged = true
      }
      // model left NULL — backfilled per-session below from next assistant.
      // hour/day are NULL — clients group by local TZ from `timestamp`.
      buffer.push([uuid, sessionId, tsStr, 'user', null, 0, 0, 0, 0, null, null])
    } else {
      // assistant — store raw token components separately so SQL doesn't
      // double-count when summing input + cache_read + cache_write.
      const model: string | undefined = e.model ?? e.message?.model
      const usage: UsageBlock | undefined = e.usage ?? e.message?.usage
      const messageId: string | undefined = e.message?.id
      const inT = usage?.input_tokens ?? 0
      const outT = usage?.output_tokens ?? 0
      const cacheR = usage?.cache_read_input_tokens ?? 0
      const cacheW = usage?.cache_creation_input_tokens ?? 0
      // Track peak effective input (input + cache_read + cache_write)
      // within the current segment. <synthetic> rows would tank the peak
      // (zero-token usage stubs) — skip them.
      if (model !== '<synthetic>') {
        const eff = inT + cacheR + cacheW
        if (eff > metaPeakInSegment) metaPeakInSegment = eff
      }
      const content = e.message?.content
      if (Array.isArray(content)) {
        let toolIdx = 0
        for (const b of content) {
          if (b?.type === 'tool_use' && b.name) {
            const toolUuid = `${uuid}#tool${toolIdx++}`
            buffer.push([toolUuid, sessionId, tsStr, 'tool_use', model || null, 0, 0, 0, 0, b.name, messageId ?? null])
          }
        }
      }
      buffer.push([uuid, sessionId, tsStr, 'assistant', model || null, inT, outT, cacheR, cacheW, null, messageId ?? null])
    }

    if (buffer.length >= 500) {
      await insertMany(buffer)
      inserted += buffer.length
      buffer = []
    }
  }
  if (buffer.length > 0) {
    await insertMany(buffer)
    inserted += buffer.length
  }

  // Backfill model for user-rows (next-assistant lookup, per session).
  try { await write(SQL_FILL_USER_MODELS, [sessionId]) } catch (err) {
    warnLog('fillUserModels failed', { sessionId, error: String(err) })
  }

  // Re-apply token + subagent flag in case the live onConversationId
  // callback raced with this sweep and inserted session_tokens mid-flight.
  const tokenRowsAfter = (await db.runAndReadAll(SQL_LOOKUP_TOKEN, [sessionId])).getRowObjects() as Array<{ token_id: string | null; user_name: string | null }>
  const tokenRowAfter = tokenRowsAfter[0]
  const userNameAfter = (tokenRowAfter?.user_name && tokenRowAfter.user_name.length > 0)
    ? tokenRowAfter.user_name
    : slugInfo?.userName ?? null
  const tokenIdAfter = slugInfo?.tokenId
    ?? (tokenRowAfter?.token_id && tokenRowAfter.token_id.length > 0 ? tokenRowAfter.token_id : null)
  await write(SQL_APPLY_TOKEN_AND_SUBAGENT, [
    tokenIdAfter,
    userNameAfter,
    subFlag,
    info.parentSessionId,
    sessionId,
    subFlag,
    info.parentSessionId,
  ])

  // Persist accumulated session metadata (only for non-subagent files —
  // subagents share parent's title etc).
  if (!info.isSubagent) {
    const conversationContext = metaPeakInSegment + metaPreBoundarySnapshots.reduce((a, b) => a + b, 0)
    await write(SQL_UPDATE_SESSION_META, [
      metaTitleChanged ? metaTitle : null,
      metaPeakInSegment,
      metaCompactBoundaries,
      JSON.stringify(metaPreBoundarySnapshots),
      conversationContext,
      sessionId,
    ])
  }

  await write(SQL_UPSERT_OFFSET, [filePath, size, size, new Date().toISOString()])
  return { inserted }
}

let running = false
let timer: ReturnType<typeof setInterval> | null = null

// Sweep N JSONL files concurrently: the file reads, parsing and DuckDB
// READS run in parallel; the writes of all workers are serialised by
// `withStatsWrite` (concurrent write transactions were the crash of
// 6.3.131, see header). Cap at 8 to balance FS read concurrency on
// spinning disks; SSDs handle higher fine.
const SWEEP_CONCURRENCY = 8

export async function sweepAll(): Promise<{ files: number; inserted: number }> {
  if (running) return { files: 0, inserted: 0 }
  running = true
  let files = 0
  let inserted = 0
  try {
    // Drain the async generator into an array first so we can dispatch
    // chunks of CONCURRENCY in parallel without losing the queue.
    // Sort by mtime DESC so fresh sessions (live activity) ingest first
    // — old subagent files trickle in afterwards and don't block the UI.
    const collected: JsonlFileInfo[] = []
    for await (const info of listJsonlFiles()) collected.push(info)
    // Stat in parallel — the sweeper traverses thousands of files on
    // boot and serial fsp.stat (one threadpool task each) used to add
    // many seconds of stall before any worker started. Promise.all
    // batches them across the libuv pool.
    const stated = await Promise.all(collected.map(async info => {
      try {
        const mtimeMs = (await fsp.stat(info.path)).mtimeMs
        return { info, mtimeMs }
      } catch { return null }
    }))
    const drained = stated.filter((x): x is { info: JsonlFileInfo; mtimeMs: number } => x !== null)
    drained.sort((a, b) => b.mtimeMs - a.mtimeMs)
    const queue: JsonlFileInfo[] = drained.map(d => d.info)

    // Worker pool — N parallel readers pull from the shared queue.
    // Each worker gets a DEDICATED DuckDB connection (via getDbByIndex)
    // so two workers never share a conn, which DuckDB requires for
    // independent in-flight queries.
    let cursor = 0
    async function worker(workerIdx: number): Promise<void> {
      const conn = await getDbByIndex(workerIdx)
      while (true) {
        const idx = cursor++
        if (idx >= queue.length) return
        const info = queue[idx]!
        const r = await processOneFile(info, conn).catch((err) => { warnLog('jsonl-sweeper processOneFile failed', { path: info.path, error: err instanceof Error ? err.message : String(err) }); return { inserted: 0 } })
        files++
        inserted += r.inserted
      }
    }
    await Promise.all(Array.from({ length: SWEEP_CONCURRENCY }, (_, i) => worker(i)))
  } finally {
    running = false
  }
  if (inserted > 0 || files > 0) {
    debugLog('jsonl-sweeper run', { files, inserted })
  }
  return { files, inserted }
}

/** Sweep ONE specific JSONL file (live-mode trigger from JsonlWatcher
 *  on every append). Bypasses the running-flag so it can run in
 *  parallel with sweepAll's bulk pass. processOneFile itself is
 *  idempotent on uuid (INSERT … ON CONFLICT DO NOTHING) and on offset
 *  (last_offset advances atomically), so concurrent invocations on
 *  different files are safe. */
export async function sweepOneByPath(filePath: string, isSubagent = false, parentSessionId: string | null = null): Promise<void> {
  await processOneFile({ path: filePath, isSubagent, parentSessionId }).catch(err => {
    warnLog('sweepOneByPath failed', { filePath, error: String(err) })
  })
}

export function startJsonlSweeper(intervalMs = 5 * 60_000): void {
  // Initial catch-up on boot — fire-and-forget, don't block server start.
  // session_tokens auto-populates inside processOneFile via tokenInfoFromSlug.
  sweepAll().catch(err => warnLog('initial sweep failed', { error: String(err) }))
  if (timer) clearInterval(timer)
  timer = setInterval(() => {
    sweepAll().catch(err => warnLog('periodic sweep failed', { error: String(err) }))
  }, intervalMs)
}

export function stopJsonlSweeper(): void {
  if (timer) { clearInterval(timer); timer = null }
}

// ============================================================================
// DuckDB maintenance — diagnostics, retention, and safe in-process compaction.
// ============================================================================
//
// The server holds an exclusive lock on proxy.duckdb, so maintenance CANNOT
// run from a second process (verified: even a READ_ONLY attach fails with a
// conflicting-lock error). Everything here runs on the server's own pool via
// the lifecycle gate.
//
// Background: DuckDB never shrinks its file below the high-water mark on its
// own — deleted/updated row versions are only reclaimed on CHECKPOINT, and
// even then the file's max size persists. Historic churn (the sweeper's
// unconditional per-row UPDATE, now fixed) inflated proxy.duckdb to ~1.7 GB.
// Retention + CHECKPOINT stops growth; a full COPY-FROM-DATABASE rewrite is
// the only way to reclaim the existing high-water mark.

import fs from 'fs'
import path from 'path'
import {
  getDb, closeDb, getDbForMaintenance,
  beginMaintenance, endMaintenance, getDataDir, getDuckDbPath,
} from './lifecycle'
import { infoLog, warnLog, errorLog } from '../../logging'

// Retention windows. Only tables whose old rows are provably never re-read are
// aged out (see the DB audit): otel_events (dashboard reads only the most
// recent ~50 + by-id) and api_errors (timeseries capped at 30d, recent capped
// at 500). otel_metrics powers full-history cost/token summaries, so it is NOT
// aged out here — that would silently change historical totals.
const OTEL_EVENTS_RETENTION_DAYS = Number(process.env.OCB_OTEL_EVENTS_DAYS || 30)
const API_ERRORS_RETENTION_DAYS = Number(process.env.OCB_API_ERRORS_DAYS || 90)

export interface TableDiag { table: string; rows: number; textMB: number }
export interface DbDiag {
  fileMB: number
  tables: TableDiag[]
  sumTextMB: number
  freeBlockRatio: number | null
}

function num(v: unknown): number { return typeof v === 'bigint' ? Number(v) : Number(v ?? 0) }

async function listTables(db: Awaited<ReturnType<typeof getDb>>): Promise<string[]> {
  const rows = (await db.runAndReadAll(
    `SELECT table_name FROM information_schema.tables
     WHERE table_catalog = current_catalog() AND table_schema = 'main'
     ORDER BY table_name`,
  )).getRowObjects()
  return rows.map(r => String((r as { table_name: unknown }).table_name))
}

async function varcharCols(db: Awaited<ReturnType<typeof getDb>>, table: string): Promise<string[]> {
  const rows = (await db.runAndReadAll(
    `SELECT column_name FROM information_schema.columns
     WHERE table_catalog = current_catalog() AND table_schema = 'main'
       AND table_name = ? AND data_type = 'VARCHAR'`,
    [table],
  )).getRowObjects()
  return rows.map(r => String((r as { column_name: unknown }).column_name))
}

/** Per-table row counts + VARCHAR text megabytes + file size. Mirrors the
 *  user's manual MODE=diag script, but in-process. */
export async function dbDiag(): Promise<DbDiag> {
  const db = await getDb()
  const out: TableDiag[] = []
  let sumText = 0
  for (const table of await listTables(db)) {
    const cols = await varcharCols(db, table)
    const lenExpr = cols.length ? cols.map(c => `COALESCE(strlen("${c}"),0)`).join('+') : '0'
    const row = (await db.runAndReadAll(
      `SELECT COUNT(*) AS n, COALESCE(SUM(${lenExpr}),0) AS b FROM "${table}"`,
    )).getRowObjects()[0] as { n: unknown; b: unknown }
    const mb = num(row.b) / 1048576
    sumText += mb
    out.push({ table, rows: num(row.n), textMB: Math.round(mb) })
  }
  out.sort((a, b) => b.textMB - a.textMB)

  let fileMB = 0
  try { fileMB = Math.round(fs.statSync(getDuckDbPath()).size / 1048576) } catch { /* ignore */ }

  // PRAGMA database_size exposes block accounting → fragmentation signal.
  let freeBlockRatio: number | null = null
  try {
    const dsRows = (await db.runAndReadAll(`PRAGMA database_size`)).getRowObjects() as Array<Record<string, unknown>>
    const ds = dsRows[0]
    if (ds) {
      const total = num(ds.total_blocks)
      const free = num(ds.free_blocks)
      if (total > 0) freeBlockRatio = Math.round((free / total) * 100) / 100
    }
  } catch { /* PRAGMA shape varies by version — non-fatal */ }

  return { fileMB, tables: out, sumTextMB: Math.round(sumText), freeBlockRatio }
}

/** Nightly retention: delete provably-unread old rows, then CHECKPOINT to
 *  release their blocks for reuse (stops growth without a full rewrite). */
export async function runRetention(): Promise<{ otelEventsDeleted: number; apiErrorsDeleted: number }> {
  const db = await getDb()
  const otelCut = new Date(Date.now() - OTEL_EVENTS_RETENTION_DAYS * 86400_000).toISOString()
  const errCut = new Date(Date.now() - API_ERRORS_RETENTION_DAYS * 86400_000).toISOString()

  // Count-then-delete per table (DuckDB's run() doesn't surface affected-row
  // counts uniformly). `table` is a fixed literal below, never user input.
  const ageOut = async (table: string, tsCol: string, cut: string): Promise<number> => {
    try {
      const before = (await db.runAndReadAll(
        `SELECT COUNT(*) AS n FROM "${table}" WHERE "${tsCol}" < ?`, [cut],
      )).getRowObjects()[0] as { n: unknown }
      const n = num(before.n)
      if (n > 0) await db.run(`DELETE FROM "${table}" WHERE "${tsCol}" < ?`, [cut])
      return n
    } catch (err) {
      warnLog('[db-maint] retention delete failed', { table, error: String(err) })
      return 0
    }
  }

  const otelEventsDeleted = await ageOut('otel_events', 'timestamp', otelCut)
  const apiErrorsDeleted = await ageOut('api_errors', 'timestamp', errCut)

  if (otelEventsDeleted + apiErrorsDeleted > 0) {
    try { await db.run(`CHECKPOINT`) } catch (err) { warnLog('[db-maint] checkpoint failed', { error: String(err) }) }
  }
  infoLog('[db-maint] retention complete', { otelEventsDeleted, apiErrorsDeleted })
  return { otelEventsDeleted, apiErrorsDeleted }
}

// ── Safe in-process compaction (rewrite + atomic swap) ─────────────────────

function paths() {
  const main = getDuckDbPath()
  return {
    main,
    wal: `${main}.wal`,
    tmp: path.join(getDataDir(), 'proxy.compacting.duckdb'),
    next: path.join(getDataDir(), 'proxy.duckdb.next'),
    old: path.join(getDataDir(), 'proxy.duckdb.old'),
  }
}

function rmQuiet(p: string): void { try { fs.rmSync(p, { force: true }) } catch { /* ignore */ } }

/**
 * Recover from a compaction interrupted by a crash. Called at boot BEFORE the
 * pool opens. Invariant: the original DB is never removed until the rewritten
 * copy passed a sanity check, and the rename sequence is:
 *   1. tmp    → next   (COPY complete + committed)
 *   2. main   → old
 *   3. next   → main
 * so at any crash point the DB is deterministically recoverable.
 */
export function recoverFromInterruptedCompaction(): void {
  const p = paths()
  // A half-written temp (crash during COPY) is always garbage.
  rmQuiet(p.tmp)

  if (!fs.existsSync(p.main)) {
    if (fs.existsSync(p.next)) {
      // Crashed between renames 2 and 3 — the rewritten DB is complete.
      fs.renameSync(p.next, p.main)
      rmQuiet(p.old)
      warnLog('[db-maint] recovered DB from proxy.duckdb.next after interrupted compaction')
    } else if (fs.existsSync(p.old)) {
      // Crashed between renames 1(done via next) and 2 with main already gone
      // but next consumed — fall back to the pre-compaction original.
      fs.renameSync(p.old, p.main)
      warnLog('[db-maint] restored DB from proxy.duckdb.old after interrupted compaction')
    }
    return
  }
  // main present → either no compaction was in flight, or it finished. Clean
  // up any leftover intermediates from a successful-but-uncleaned run.
  rmQuiet(p.next)
  rmQuiet(p.old)
}

let compacting = false

export interface CompactResult {
  ok: boolean
  beforeMB: number
  afterMB: number
  error?: string
}

/**
 * Rewrite proxy.duckdb into a fresh file (dropping dead row versions + index
 * bloat), then atomically swap it in. Blocks new DB work for the duration via
 * the lifecycle maintenance gate. Safe to call while the server is up; refuses
 * to run concurrently with itself.
 */
export async function compactDatabase(): Promise<CompactResult> {
  if (compacting) return { ok: false, beforeMB: 0, afterMB: 0, error: 'Compaction already running' }
  compacting = true
  const p = paths()
  const beforeMB = (() => { try { return Math.round(fs.statSync(p.main).size / 1048576) } catch { return 0 } })()

  // Clean any stale intermediates from a prior aborted attempt.
  rmQuiet(p.tmp); rmQuiet(p.next); rmQuiet(p.old)

  beginMaintenance()
  try {
    // 1. Produce the compacted copy on a live connection (original untouched).
    //    beginMaintenance already parked NEW getDb() callers; give the last
    //    in-flight query a beat to finish, then checkpoint + rewrite. We use
    //    the gate-EXEMPT accessor — getDb() would await our own gate.
    await new Promise(r => setTimeout(r, 250))
    const conn = await getDbForMaintenance()
    await conn.run(`CHECKPOINT`)
    const esc = (s: string) => s.replace(/'/g, "''")
    const cat = (await conn.runAndReadAll(`SELECT current_catalog() AS c`)).getRowObjects()[0] as { c: unknown }
    await conn.run(`ATTACH '${esc(p.tmp)}' AS clean`)
    await conn.run(`COPY FROM DATABASE "${String(cat.c)}" TO clean`)
    await conn.run(`DETACH clean`)

    // 2. Close the pool → releases the OS lock on proxy.duckdb.
    await closeDb()

    // 3. Atomic three-step rename (see recover invariant).
    fs.renameSync(p.tmp, p.next)
    if (fs.existsSync(p.main)) fs.renameSync(p.main, p.old)
    rmQuiet(p.wal) // stale WAL belongs to the old file
    fs.renameSync(p.next, p.main)

    // 4. Reopen + sanity check.
    const check = await getDbForMaintenance()
    await check.runAndReadAll(`SELECT COUNT(*) FROM jsonl_events`)

    rmQuiet(p.old)
    const afterMB = (() => { try { return Math.round(fs.statSync(p.main).size / 1048576) } catch { return 0 } })()
    infoLog('[db-maint] compaction complete', { beforeMB, afterMB })
    return { ok: true, beforeMB, afterMB }
  } catch (err) {
    errorLog('[db-maint] compaction FAILED — rolling back', { error: String(err) })
    // Roll back: restore the original if the swap didn't complete.
    try {
      if (!fs.existsSync(p.main) && fs.existsSync(p.old)) fs.renameSync(p.old, p.main)
    } catch (e) { errorLog('[db-maint] rollback rename failed', { error: String(e) }) }
    rmQuiet(p.tmp); rmQuiet(p.next)
    // Make sure the pool is usable again.
    try { await getDbForMaintenance() } catch { /* boot path will retry */ }
    return { ok: false, beforeMB, afterMB: beforeMB, error: err instanceof Error ? err.message : String(err) }
  } finally {
    endMaintenance()
    compacting = false
  }
}

// ── Scheduler ──────────────────────────────────────────────────────────────

// Auto-compact when the file exceeds this AND has meaningful free-block
// fragmentation. Default 512 MB — well above a healthy steady-state size but
// far below the 1.7 GB the churn bug produced.
const COMPACT_THRESHOLD_MB = Number(process.env.OCB_DB_COMPACT_MB || 512)
// Hard ceiling: above this the file is compacted EVEN mid-session (idle gate
// bypassed). Carrying a multi-GB DB — and the churn that got it there — is worse
// than the sub-second write stall a compaction costs. This is the backstop for
// "user worked all day, server was never idle, file ballooned". Default 2 GB.
const COMPACT_HARD_MB = Number(process.env.OCB_DB_COMPACT_HARD_MB || 2048)
const MAINT_INTERVAL_MS = Number(process.env.OCB_DB_MAINT_INTERVAL_MS || 60 * 60_000) // hourly
let maintTimer: ReturnType<typeof setInterval> | null = null
let lastRetentionDay = ''

/** True when no PTY session is mid-work — a safe window to compact. */
async function serverIsIdle(): Promise<boolean> {
  try {
    const { getAllSessions } = await import('../../pty/session-manager')
    return getAllSessions().every(s => s.state !== 'running')
  } catch { return true }
}

async function maintenanceTick(): Promise<void> {
  try {
    // Retention once per calendar day (UTC).
    const today = new Date().toISOString().slice(0, 10)
    if (today !== lastRetentionDay) {
      lastRetentionDay = today
      await runRetention()
    }

    // Hourly CHECKPOINT regardless of deletions. DuckDB only reclaims dead row
    // versions (from request-log churn) into REUSABLE free blocks on CHECKPOINT;
    // without a periodic one the previous logic checkpointed only when the daily
    // retention pass deleted rows, so during a full day of active use the file's
    // high-water mark climbed unbounded (observed: 8.3 GB before an overnight
    // idle compaction reclaimed it to ~67 MB). A plain CHECKPOINT is cheap and
    // caps that growth by letting new versions reuse freed space in-file.
    try {
      const db = await getDb()
      await db.run(`CHECKPOINT`)
    } catch (err) {
      warnLog('[db-maint] periodic checkpoint failed', { error: String(err) })
    }

    // Compaction (shrinks the file below its high-water mark). Normal path: only
    // when fragmented AND the server is quiet, since it briefly stalls DB writes.
    // Hard-ceiling path: if the file grew past COMPACT_HARD_MB anyway (active use
    // meant "idle" never came), compact regardless — a big file is worse than
    // the stall.
    const diag = await dbDiag()
    const fragmented = diag.freeBlockRatio === null || diag.freeBlockRatio > 0.2
    const idle = await serverIsIdle()
    const overHardCeiling = diag.fileMB >= COMPACT_HARD_MB
    if ((diag.fileMB >= COMPACT_THRESHOLD_MB && fragmented && idle) || overHardCeiling) {
      infoLog('[db-maint] compacting', {
        fileMB: diag.fileMB, freeBlockRatio: diag.freeBlockRatio, idle, hardCeiling: overHardCeiling,
      })
      await compactDatabase()
    }
  } catch (err) {
    warnLog('[db-maint] tick failed', { error: String(err) })
  }
}

export function startDbMaintenance(): void {
  if (maintTimer) clearInterval(maintTimer)
  // First tick after a short delay so it doesn't pile onto boot.
  setTimeout(() => { void maintenanceTick() }, 60_000)
  maintTimer = setInterval(() => { void maintenanceTick() }, MAINT_INTERVAL_MS)
  infoLog('[db-maint] scheduler started', { intervalMs: MAINT_INTERVAL_MS, thresholdMB: COMPACT_THRESHOLD_MB })
}

export function stopDbMaintenance(): void {
  if (maintTimer) { clearInterval(maintTimer); maintTimer = null }
}

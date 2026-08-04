// ============================================================================
// Database Lifecycle — DuckDB connection + one-time SQLite migration
// ============================================================================
//
// One Connection per process — DuckDB allows concurrent multi-thread
// writes within a process via MVCC, so callers share this single
// connection. All operations are Promise-based (Neo client).
//
// First-boot migration: if proxy.db (SQLite) exists and proxy.duckdb
// doesn't, attach SQLite read-only and copy every table over. The
// SQLite file is then renamed to proxy.db.pre-duckdb-backup so a manual
// rollback path exists.

import { DuckDBInstance, DuckDBConnection } from '@duckdb/node-api'
import path from 'path'
import fs from 'fs'
import { initSchema } from './schema'
import { warnLog, debugLog } from '../../logging'

const DATA_DIR = path.join(process.cwd(), 'data')
const SQLITE_PATH = path.join(DATA_DIR, 'proxy.db')
const SQLITE_BACKUP_PATH = path.join(DATA_DIR, 'proxy.db.pre-duckdb-backup')
const DUCKDB_PATH = path.join(DATA_DIR, 'proxy.duckdb')

export function getDataDir(): string { return DATA_DIR }
export function getDuckDbPath(): string { return DUCKDB_PATH }

// Maintenance gate: while a compaction is swapping the DB file, new getDb()
// callers park on this promise so no query races the file rename. Writers all
// take a connection per-call (verified: sweeper per-run, otlp/api_errors/
// session_tokens/settings per-call), so gating acquisition is enough to
// quiesce — no long-held connection can straddle the swap.
let maintenanceGate: Promise<void> | null = null
let releaseMaintenance: (() => void) | null = null

/** Begin a maintenance window — subsequent getDb()/getDbByIndex() calls block
 *  until endMaintenance(). Idempotent-safe: throws if already open. */
export function beginMaintenance(): void {
  if (maintenanceGate) throw new Error('Maintenance already in progress')
  maintenanceGate = new Promise<void>((resolve) => { releaseMaintenance = resolve })
}

/** End the maintenance window — unblocks parked getDb() callers. */
export function endMaintenance(): void {
  releaseMaintenance?.()
  maintenanceGate = null
  releaseMaintenance = null
}

// Connection pool size. DuckDB allows N independent connections to the
// same instance — each runs its own query in parallel (true CPU
// parallelism, no global lock). 8 matches the sweeper's worker pool so
// each parallel JSONL reader gets a dedicated connection (no cross-talk
// during bulk loads); endpoint queries share whichever conn round-robin
// hands them, which is fine because each endpoint's queries run
// sequentially on a single conn.
const POOL_SIZE = 8

let instance: DuckDBInstance | null = null
const pool: DuckDBConnection[] = []
let nextConn = 0
let initPromise: Promise<DuckDBConnection> | null = null

/** Round-robin connection acquisition. Each `await getDb()` returns the
 *  NEXT connection in the pool. JS is single-threaded so two getDb()
 *  calls within the same microtask burst get distinct connections —
 *  safe to issue concurrent queries via Promise.all when each branch
 *  calls getDb() inside it.
 *
 *  IMPORTANT: do NOT share one returned connection across parallel
 *  awaits (e.g. `const db = await getDb(); await Promise.all([db.run...
 *  db.run...])` — that fires two queries on the same connection
 *  concurrently which DuckDB serialises and may error). Always call
 *  getDb() inside each parallel branch. */
export async function getDb(): Promise<DuckDBConnection> {
  if (maintenanceGate) await maintenanceGate
  if (pool.length === 0) {
    if (!initPromise) initPromise = initDb()
    await initPromise
  }
  // Modulo with pre-increment is atomic in single-threaded JS.
  const idx = nextConn++ % pool.length
  return pool[idx]!
}

/** Synchronous accessor — only valid AFTER `await initStatsDb()` has
 *  resolved at server boot. Returns the FIRST pool connection. Used by
 *  hot paths that can't be made async (none currently). */
export function getDbSync(): DuckDBConnection {
  if (pool.length === 0) throw new Error('DuckDB not initialized — call await initStatsDb() at boot')
  return pool[0]!
}

/** Eager initializer — call once during server startup. */
export async function initStatsDb(): Promise<DuckDBConnection> {
  return getDb()
}

/** Gate-EXEMPT connection accessor for the maintenance module itself. The
 *  compaction routine HOLDS the maintenance gate, so it can't call getDb()
 *  (that would await its own gate → deadlock). This bypasses the gate and
 *  (re-)inits the pool if needed. Do NOT use anywhere else. */
export async function getDbForMaintenance(): Promise<DuckDBConnection> {
  if (pool.length === 0) {
    if (!initPromise) initPromise = initDb()
    await initPromise
  }
  return pool[0]!
}

/** Get a SPECIFIC pool connection by index (0..POOL_SIZE-1). Used by
 *  worker pools (e.g. sweeper) that need a stable per-worker connection
 *  to avoid issuing concurrent queries against the same DuckDB conn —
 *  which DuckDB serialises and can error. Index is taken mod pool size. */
export async function getDbByIndex(idx: number): Promise<DuckDBConnection> {
  if (maintenanceGate) await maintenanceGate
  if (pool.length === 0) {
    if (!initPromise) initPromise = initDb()
    await initPromise
  }
  return pool[idx % pool.length]!
}

/** Pool size — exposed so worker counts can size themselves. */
export function getDbPoolSize(): number {
  return pool.length || POOL_SIZE
}

async function initDb(): Promise<DuckDBConnection> {
  fs.mkdirSync(DATA_DIR, { recursive: true })

  // Recover from a compaction interrupted by a crash BEFORE opening the pool
  // (may rename proxy.duckdb.next/.old back into place). Lazy import avoids a
  // cycle: maintenance imports lifecycle.
  try {
    const { recoverFromInterruptedCompaction } = await import('./maintenance')
    recoverFromInterruptedCompaction()
  } catch (err) {
    warnLog('[db-maint] boot recovery skipped', { error: String(err) })
  }

  // One-time migration: SQLite → DuckDB.
  if (fs.existsSync(SQLITE_PATH) && fs.statSync(SQLITE_PATH).size > 0 && !fs.existsSync(DUCKDB_PATH)) {
    debugLog('Migrating SQLite stats DB to DuckDB...', { sqlite: SQLITE_PATH, duckdb: DUCKDB_PATH })
    await migrateFromSqlite()
  }

  // Single instance, multiple connections off it. We use `create` (not
  // `fromCache`) because compaction closes the instance and swaps the file
  // at this same path — a cached handle would come back closed/stale. We're
  // single-process and closeDb() fully drains the previous instance before a
  // reopen, so a fresh create never races a second live handle.
  instance = await DuckDBInstance.create(DUCKDB_PATH)
  for (let i = 0; i < POOL_SIZE; i++) {
    pool.push(await instance.connect())
  }
  // Schema migrations run on conn[0] only — the changes are visible to
  // every connection sharing the same DB file via DuckDB's catalog.
  await initSchema(pool[0]!)
  debugLog('DuckDB pool initialized', { size: POOL_SIZE, path: DUCKDB_PATH })
  return pool[0]!
}

/** Open SQLite with DuckDB's `sqlite_scanner` and copy every user table
 *  into a fresh DuckDB file. On any failure: delete the partial DuckDB
 *  file and re-throw — boot will retry on next start, no data lost. */
async function migrateFromSqlite(): Promise<void> {
  let inst: DuckDBInstance | null = null
  let conn: DuckDBConnection | null = null
  try {
    inst = await DuckDBInstance.create(DUCKDB_PATH)
    conn = await inst.connect()
    // Run schema FIRST so all DuckDB tables exist with their proper
    // PRIMARY KEY / UNIQUE constraints. We then INSERT data from the
    // attached SQLite — `CREATE TABLE x AS SELECT * FROM legacy.x`
    // does NOT preserve constraints (DuckDB strips PK/UNIQUE from
    // CTAS), which would later break `INSERT … ON CONFLICT (col)`
    // queries that reference the PK.
    await initSchema(conn)
    await conn.run(`INSTALL sqlite`)
    await conn.run(`LOAD sqlite`)
    await conn.run(`ATTACH '${SQLITE_PATH.replace(/'/g, "''")}' AS legacy (TYPE SQLITE, READ_ONLY)`)
    // Discover SQLite tables (only those that also exist in DuckDB
    // post-initSchema — legacy/dropped tables are skipped).
    const sqliteRows = (await conn.runAndReadAll(
      `SELECT table_name AS name FROM information_schema.tables
       WHERE table_catalog = 'legacy' AND table_schema = 'main'`
    )).getRowObjects()
    const sqliteTables = sqliteRows.map(r => String((r as any).name))
    const duckRows = (await conn.runAndReadAll(
      `SELECT table_name AS name FROM information_schema.tables
       WHERE table_catalog = current_catalog() AND table_schema = 'main'`
    )).getRowObjects()
    const duckTables = new Set(duckRows.map(r => String((r as any).name)))

    for (const t of sqliteTables) {
      if (!duckTables.has(t)) {
        debugLog(`skip table "${t}" — not in current DuckDB schema (legacy/dropped)`)
        continue
      }
      // Match column lists — INSERT only the columns DuckDB knows about.
      const duckColsRows = (await conn.runAndReadAll(
        `SELECT column_name FROM information_schema.columns
         WHERE table_catalog = current_catalog() AND table_schema = 'main' AND table_name = ?`,
        [t]
      )).getRowObjects()
      const duckCols = duckColsRows.map(r => String((r as any).column_name))
      const sqliteColsRows = (await conn.runAndReadAll(
        `SELECT column_name FROM information_schema.columns
         WHERE table_catalog = 'legacy' AND table_schema = 'main' AND table_name = ?`,
        [t]
      )).getRowObjects()
      const sqliteCols = new Set(sqliteColsRows.map(r => String((r as any).column_name)))
      const sharedCols = duckCols.filter(c => sqliteCols.has(c))
      if (sharedCols.length === 0) {
        debugLog(`skip "${t}" — no common columns with legacy`)
        continue
      }
      const colList = sharedCols.map(c => `"${c}"`).join(', ')
      // ON CONFLICT DO NOTHING — initSchema may have seeded sentinel
      // rows (e.g. `settings.session_meta_v1_seeded`); legacy SQLite
      // may have its own row for the same key. Skip duplicates.
      await conn.run(`INSERT OR IGNORE INTO "${t}" (${colList}) SELECT ${colList} FROM legacy."${t}"`)
      const cReader = await conn.runAndReadAll(`SELECT COUNT(*) AS n FROM "${t}"`)
      const n = (cReader.getRowObjects()[0] as any).n
      debugLog(`migrated table "${t}"`, { rows: Number(n), cols: sharedCols.length })
    }
    await conn.run(`DETACH legacy`)
    fs.renameSync(SQLITE_PATH, SQLITE_BACKUP_PATH)
    debugLog('SQLite → DuckDB migration complete; SQLite renamed to .pre-duckdb-backup')
  } catch (err) {
    warnLog('SQLite → DuckDB migration FAILED, rolling back', { error: String(err) })
    try { conn?.disconnectSync?.() } catch {}
    try { inst?.closeSync?.() } catch {}
    try { fs.rmSync(DUCKDB_PATH, { force: true }) } catch {}
    throw err
  } finally {
    try { conn?.disconnectSync?.() } catch {}
    try { inst?.closeSync?.() } catch {}
  }
}

export async function closeDb(): Promise<void> {
  // Drain pool: close every connection first, then the instance.
  for (const c of pool) {
    try { c.disconnectSync?.() } catch {}
  }
  pool.length = 0
  try { instance?.closeSync?.() } catch {}
  instance = null
  initPromise = null
  nextConn = 0
}

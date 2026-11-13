// Incremental stats rollup — the dashboard reads aggregates, not raw events.
//
// Level 1, `stats_rollup`: one row per (session, UTC 15-minute bucket, model,
// engine, long-prompt flag) with the additive counters every dashboard metric
// is built from, plus the session's live owner `u` (session_tokens.user_name of
// a non-deleted session, NULL otherwise). 15 minutes is the coarsest UTC grain
// that maps exactly onto every real time-zone offset.
//
// Level 2/3 are what the dashboard actually reads, sized by users × time, not
// by sessions or raw events:
//   stats_session_live        one row per live session (first/last, context)
//   stats_user_model          per (owner, model, long-prompt) totals (roster, cost)
//   stats_user_hour           per (owner, UTC hour, model) counters (cards)
//   stats_user_hour_sessions  per (owner, UTC hour, g): sessions active in that
//                             hour whose previous active hour is g hours back
//                             (g = 26: none that recent). A session counts on a
//                             local day D at hour h iff g > hours since D began,
//                             which gives exact distinct sessions per local day
//                             in any whole-hour zone without a session axis.
//
// Maintenance is incremental, like a materialized view with delta updates:
// every writer that changes a session's events or owner records the session
// in the durable `stats_rollup_dirty` set (inside its own write transaction,
// or before the mutating statement). The drainer then, in one transaction per
// batch: subtracts the batch's old level-1 contribution from the level-3
// tables, replaces its level-1 rows from the accounting view, adds the new
// contribution back, and replaces its level-2 rows. Nothing else is touched.
// A crash leaves the mark, so the next boot repairs the session. The initial
// build over existing data runs once per database in the background in hash
// chunks, resumable via `settings`; until it completes readers compute the
// same rows inline.

import type { DuckDBConnection } from '@duckdb/node-api'
import { accountingEventsSelect } from './accounting-schema'
import { withStatsWrite } from './write-lock'
import { debugLog, infoLog, warnLog } from '../../logging'

/** Bump to rebuild the rollup when its definition changes. */
export const ROLLUP_VERSION = '3'
const BUILD_CHUNKS = 32
const DRAIN_BATCH = 256
/** g value for "no earlier active hour within a day's reach" (> any day length). */
export const NO_RECENT_HOUR = 26

let engineColumn = false
let built = false
let revision = 0
const bootNonce = Date.now().toString(36)
let drainTimer: ReturnType<typeof setTimeout> | null = null
let draining: Promise<void> | null = null
let building: Promise<void> | null = null

/** Monotonic data revision for cache keys and client polling. */
export function rollupRevision(): string {
  return `${bootNonce}.${revision}`
}
/** A write changed what readers see: new revision. */
export function touchRollupRevision(): void {
  revision++
}
export function rollupReady(): boolean {
  return built
}

const ctx = `COALESCE(e.input_tokens,0) + COALESCE(e.cache_read_tokens,0) + COALESCE(e.cache_write_tokens,0)`
const topLevelAssistant = `e.type = 'assistant' AND COALESCE(e.is_subagent,0) = 0`

/** Level-1 rows for the sessions matching `filter` (predicate on `session_id`). */
export function rollupSelect(filter = 'TRUE'): string {
  const engine = engineColumn ? `COALESCE(e.engine, 'claude')` : `'claude'`
  return `
    SELECT e.session_id, hash(e.session_id) AS sid,
      CASE WHEN st.session_id IS NULL THEN NULL ELSE COALESCE(st.user_name, '') END AS u,
      COALESCE(e.parent_session_id, e.session_id) AS session_key,
      CAST(COALESCE(e.is_subagent, 0) AS INTEGER) AS is_subagent,
      time_bucket(INTERVAL 15 MINUTE, e.timestamp::TIMESTAMP) AS bucket,
      e.model AS model,
      ${engine} AS engine,
      (e.type = 'assistant' AND ${ctx} > 100000) AS long_prompt,
      COUNT(*) AS events,
      SUM(CASE WHEN e.type = 'user' THEN 1 ELSE 0 END) AS user_msgs,
      SUM(CASE WHEN e.type = 'assistant' THEN 1 ELSE 0 END) AS assistant_msgs,
      SUM(CASE WHEN e.type = 'assistant' THEN COALESCE(e.input_tokens,0) ELSE 0 END) AS input_tokens,
      SUM(CASE WHEN e.type = 'assistant' THEN COALESCE(e.output_tokens,0) ELSE 0 END) AS output_tokens,
      SUM(CASE WHEN e.type = 'assistant' THEN COALESCE(e.cache_read_tokens,0) ELSE 0 END) AS cache_read_tokens,
      SUM(CASE WHEN e.type = 'assistant' THEN COALESCE(e.cache_write_tokens,0) ELSE 0 END) AS cache_write_tokens,
      SUM(CASE WHEN e.type = 'assistant' THEN COALESCE(e.cache_write_1h_tokens,0) ELSE 0 END) AS cache_write_1h_tokens,
      MIN(e.timestamp) AS first_ts,
      MAX(e.timestamp) AS last_ts,
      arg_max(${ctx}, {'t': e.timestamp, 'u': e.uuid}) FILTER (WHERE ${topLevelAssistant}) AS last_ctx,
      MAX({'t': e.timestamp, 'u': e.uuid}) FILTER (WHERE ${topLevelAssistant}) AS last_ctx_key
    FROM (${accountingEventsSelect(filter)}) e
    LEFT JOIN session_tokens st ON st.session_id = e.session_id AND st.deleted = 0
    WHERE (e.model IS NULL OR e.model != '<synthetic>')
    GROUP BY ALL`
}

const COLUMNS = `session_id, sid, u, session_key, is_subagent, bucket, model, engine, long_prompt, events, user_msgs,
  assistant_msgs, input_tokens, output_tokens, cache_read_tokens, cache_write_tokens, cache_write_1h_tokens,
  first_ts, last_ts, last_ctx, last_ctx_key`

// ── Derived levels: each is a pure function of level-1 rows ─────────────
// `src` is a relation of level-1 rows; only live sessions (u IS NOT NULL) count.

export function sessionLiveSelect(src: string): string {
  return `SELECT ANY_VALUE(session_id) AS session_id, sid, ANY_VALUE(u) AS u, MIN(first_ts) AS first_ts, MAX(last_ts) AS last_ts,
      date_trunc('hour', MAX(bucket)) AS last_hour, arg_max(last_ctx, last_ctx_key) AS ctx
    FROM ${src} WHERE u IS NOT NULL GROUP BY sid`
}

const USER_MODEL_SUMS = [
  'events',
  'user_msgs',
  'assistant_msgs',
  'input_tokens',
  'output_tokens',
  'cache_read_tokens',
  'cache_write_tokens',
  'cache_write_1h_tokens',
]
export function userModelSelect(src: string, sign = 1): string {
  return `SELECT u, COALESCE(model, '') AS model, long_prompt,
      ${USER_MODEL_SUMS.map((c) => `${sign} * SUM(${c}) AS ${c}`).join(', ')}
    FROM ${src} WHERE u IS NOT NULL GROUP BY ALL`
}

const USER_HOUR_SUMS = ['events', 'user_msgs', 'assistant_msgs', 'tk', 'ot']
export function userHourSelect(src: string, sign = 1): string {
  return `SELECT u, date_trunc('hour', bucket) AS hour, COALESCE(model, '') AS model,
      ${sign} * SUM(events) AS events, ${sign} * SUM(user_msgs) AS user_msgs, ${sign} * SUM(assistant_msgs) AS assistant_msgs,
      ${sign} * SUM(input_tokens + cache_read_tokens + cache_write_tokens) AS tk, ${sign} * SUM(output_tokens) AS ot
    FROM ${src} WHERE u IS NOT NULL GROUP BY ALL`
}

export function userHourSessionsSelect(src: string, sign = 1): string {
  return `SELECT u, hour, g, ${sign} * COUNT(*) AS n FROM (
      SELECT u, hour, CAST(LEAST(COALESCE(date_diff('hour', lag(hour) OVER (PARTITION BY sid ORDER BY hour), hour),
        ${NO_RECENT_HOUR}), ${NO_RECENT_HOUR}) AS INTEGER) AS g
      FROM (SELECT DISTINCT sid, u, date_trunc('hour', bucket) AS hour FROM ${src} WHERE u IS NOT NULL)
    ) GROUP BY ALL`
}

const DERIVED = {
  stats_user_model: { keys: 'u, model, long_prompt', sums: USER_MODEL_SUMS, select: userModelSelect },
  stats_user_hour: { keys: 'u, hour, model', sums: USER_HOUR_SUMS, select: userHourSelect },
  stats_user_hour_sessions: { keys: 'u, hour, g', sums: ['n'], select: userHourSessionsSelect },
} as const

/** Readers: the maintained table once built, else the same rows inline. */
export function derivedSource(table: keyof typeof DERIVED | 'stats_session_live'): string {
  if (built) return table
  const l1 = `(${rollupSelect()})`
  return `(${table === 'stats_session_live' ? sessionLiveSelect(l1) : DERIVED[table].select(l1)})`
}

/** Level-1 source for readers (time windows at 15-minute grain). */
export function rollupSource(): string {
  return built ? 'stats_rollup' : `(${rollupSelect()})`
}

/** Add (sign 1) or remove (sign -1) the contribution of the level-1 rows matching `l1Filter`. */
async function applyDerived(db: DuckDBConnection, l1Filter: string, sign: 1 | -1): Promise<void> {
  const src = `(SELECT * FROM stats_rollup WHERE ${l1Filter})`
  for (const [table, d] of Object.entries(DERIVED)) {
    await db.run(`INSERT INTO ${table} ${d.select(src, sign)}
      ON CONFLICT (${d.keys}) DO UPDATE SET ${d.sums.map((c) => `${c} = ${c} + excluded.${c}`).join(', ')}`)
  }
}

/** Replace the sessions matching both filters (`rawFilter` on the accounting
 *  view, `l1Filter` on level 1) at every level, as one delta. */
async function replaceSessions(db: DuckDBConnection, rawFilter: string, l1Filter: string): Promise<void> {
  await applyDerived(db, l1Filter, -1)
  await db.run(`DELETE FROM stats_rollup WHERE ${l1Filter}`)
  await db.run(`INSERT INTO stats_rollup (${COLUMNS}) ${rollupSelect(rawFilter)}`)
  await applyDerived(db, l1Filter, 1)
  for (const table of Object.keys(DERIVED)) {
    const zero = table === 'stats_user_hour_sessions' ? 'n = 0' : 'events = 0'
    await db.run(`DELETE FROM ${table} WHERE ${zero}`)
  }
  await db.run(`DELETE FROM stats_session_live WHERE ${l1Filter}`)
  await db.run(`INSERT INTO stats_session_live ${sessionLiveSelect(`(SELECT * FROM stats_rollup WHERE ${l1Filter})`)}`)
}

async function setting(db: DuckDBConnection, key: string): Promise<string | null> {
  const row = (await db.runAndReadAll('SELECT value FROM settings WHERE key = ?', [key])).getRowObjects()[0] as
    { value: string } | undefined
  return row?.value ?? null
}
const SET_SETTING = `INSERT INTO settings (key, value) VALUES (?, ?) ON CONFLICT (key) DO UPDATE SET value = excluded.value`
const TABLES = ['stats_rollup', 'stats_session_rollup', 'stats_session_live', ...Object.keys(DERIVED)]

/** Schema + version check. Runs at boot inside initSchema; cheap and idempotent. */
export async function initRollupSchema(db: DuckDBConnection): Promise<void> {
  const cols = (
    await db.runAndReadAll(
      `SELECT 1 FROM information_schema.columns WHERE table_name = 'jsonl_accounting_events' AND column_name = 'engine'`,
    )
  ).getRowObjects()
  engineColumn = cols.length > 0
  const version = await setting(db, 'stats_rollup_version')
  if (version !== ROLLUP_VERSION) {
    for (const t of TABLES) await db.run(`DROP TABLE IF EXISTS ${t}`)
    await db.run(`DELETE FROM settings WHERE key IN ('stats_rollup_built', 'stats_rollup_progress')`)
    await db.run(SET_SETTING, ['stats_rollup_version', ROLLUP_VERSION])
  }
  const sums = (cs: readonly string[]) => cs.map((c) => `${c} BIGINT NOT NULL`).join(', ')
  await db.run(`CREATE TABLE IF NOT EXISTS stats_rollup (
    session_id TEXT NOT NULL, sid UBIGINT NOT NULL, u TEXT, session_key TEXT NOT NULL, is_subagent INTEGER NOT NULL,
    bucket TIMESTAMP NOT NULL, model TEXT, engine TEXT NOT NULL, long_prompt BOOLEAN NOT NULL,
    events BIGINT NOT NULL, user_msgs BIGINT NOT NULL, assistant_msgs BIGINT NOT NULL,
    input_tokens BIGINT NOT NULL, output_tokens BIGINT NOT NULL, cache_read_tokens BIGINT NOT NULL,
    cache_write_tokens BIGINT NOT NULL, cache_write_1h_tokens BIGINT NOT NULL,
    first_ts TEXT NOT NULL, last_ts TEXT NOT NULL, last_ctx BIGINT, last_ctx_key STRUCT(t TEXT, u TEXT)
  )`)
  await db.run(`CREATE TABLE IF NOT EXISTS stats_session_live (
    session_id TEXT NOT NULL, sid UBIGINT NOT NULL, u TEXT NOT NULL, first_ts TEXT NOT NULL, last_ts TEXT NOT NULL,
    last_hour TIMESTAMP NOT NULL, ctx BIGINT
  )`)
  await db.run(`CREATE TABLE IF NOT EXISTS stats_user_model (
    u TEXT NOT NULL, model TEXT NOT NULL, long_prompt BOOLEAN NOT NULL, ${sums(USER_MODEL_SUMS)},
    PRIMARY KEY (u, model, long_prompt)
  )`)
  await db.run(`CREATE TABLE IF NOT EXISTS stats_user_hour (
    u TEXT NOT NULL, hour TIMESTAMP NOT NULL, model TEXT NOT NULL, ${sums(USER_HOUR_SUMS)},
    PRIMARY KEY (u, hour, model)
  )`)
  await db.run(`CREATE TABLE IF NOT EXISTS stats_user_hour_sessions (
    u TEXT NOT NULL, hour TIMESTAMP NOT NULL, g INTEGER NOT NULL, n BIGINT NOT NULL,
    PRIMARY KEY (u, hour, g)
  )`)
  await db.run(`CREATE TABLE IF NOT EXISTS stats_rollup_dirty (session_id TEXT PRIMARY KEY)`)
  built = (await setting(db, 'stats_rollup_built')) === ROLLUP_VERSION
}

/** SQL + params that record sessions as dirty. Run it inside the caller's
 *  write transaction (or before the mutation) so a crash cannot lose it. */
export function markDirtySql(sessionIds: string[]): { sql: string; params: string[] } {
  const ids = [...new Set(sessionIds.filter(Boolean))]
  return {
    sql: `INSERT INTO stats_rollup_dirty (session_id) VALUES ${ids.map(() => '(?)').join(', ')} ON CONFLICT DO NOTHING`,
    params: ids,
  }
}

/** Record dirty sessions in their own write turn and schedule a drain. */
export async function markRollupSessionsDirty(db: DuckDBConnection, sessionIds: string[]): Promise<void> {
  const { sql, params } = markDirtySql(sessionIds)
  if (params.length === 0) return
  await withStatsWrite(() => db.run(sql, params))
  scheduleRollupDrain()
}

/** Debounced drain: a burst of sweeper batches costs one recompute per session. */
export function scheduleRollupDrain(delayMs = 500): void {
  // Inline readers (before the build) already see the write.
  revision++
  if (drainTimer) return
  drainTimer = setTimeout(() => {
    drainTimer = null
    void drainRollup().catch((err) => warnLog('[stats-rollup] drain failed', { error: String(err) }))
  }, delayMs)
  drainTimer.unref?.()
}

const quote = (s: string): string => `'${s.replace(/'/g, "''")}'`

/** Recompute every dirty session. Each batch is one write transaction. */
export async function drainRollup(getConn?: () => Promise<DuckDBConnection>): Promise<void> {
  if (draining) return draining
  draining = (async () => {
    const db = await (getConn ?? (await import('./lifecycle')).getDb)()
    for (;;) {
      const done = await withStatsWrite(async () => {
        const ids = (
          await db.runAndReadAll(`SELECT session_id FROM stats_rollup_dirty LIMIT ${DRAIN_BATCH}`)
        ).getRowObjects() as Array<{ session_id: string }>
        if (ids.length === 0) return true
        const inList = `session_id IN (${ids.map((r) => quote(r.session_id)).join(', ')})`
        await db.run('BEGIN')
        try {
          await replaceSessions(db, inList, inList)
          await db.run(`DELETE FROM stats_rollup_dirty WHERE ${inList}`)
          await db.run('COMMIT')
        } catch (err) {
          await db.run('ROLLBACK').catch(() => {})
          throw err
        }
        revision++
        return ids.length < DRAIN_BATCH
      })
      if (done) return
    }
  })().finally(() => {
    draining = null
  })
  return draining
}

/** One-time background build from existing events; resumable and idempotent. */
export function startRollupBuild(getConn?: () => Promise<DuckDBConnection>): Promise<void> {
  if (built) {
    scheduleRollupDrain(0)
    return Promise.resolve()
  }
  building ??= (async () => {
    const db = await (getConn ?? (await import('./lifecycle')).getDb)()
    const started = Date.now()
    let chunk = Number((await setting(db, 'stats_rollup_progress')) ?? 0)
    for (; chunk < BUILD_CHUNKS; chunk++) {
      await withStatsWrite(async () => {
        await db.run('BEGIN')
        try {
          // hash(session_id) is the stored sid, so both filters pick the same sessions.
          await replaceSessions(db, `hash(session_id) % ${BUILD_CHUNKS} = ${chunk}`, `sid % ${BUILD_CHUNKS} = ${chunk}`)
          await db.run(SET_SETTING, ['stats_rollup_progress', String(chunk + 1)])
          await db.run('COMMIT')
        } catch (err) {
          await db.run('ROLLBACK').catch(() => {})
          throw err
        }
      })
      revision++
      debugLog('[stats-rollup] build chunk', { chunk, of: BUILD_CHUNKS })
    }
    await withStatsWrite(() => db.run(SET_SETTING, ['stats_rollup_built', ROLLUP_VERSION]))
    built = true
    revision++
    infoLog('[stats-rollup] built', { ms: Date.now() - started })
    await drainRollup(getConn)
  })().finally(() => {
    building = null
  })
  return building
}

/** Drop the rollup and rebuild it in the background (bulk deletes, repairs). */
export async function resetStatsRollup(db: DuckDBConnection): Promise<void> {
  await withStatsWrite(async () => {
    await db.run(`DELETE FROM settings WHERE key IN ('stats_rollup_built', 'stats_rollup_progress')`)
    for (const t of TABLES.filter((t) => t !== 'stats_session_rollup')) await db.run(`DELETE FROM ${t}`)
  })
  built = false
  revision++
  void startRollupBuild().catch((err) => warnLog('[stats-rollup] rebuild failed', { error: String(err) }))
}

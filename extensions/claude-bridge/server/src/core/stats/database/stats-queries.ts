// Dashboard read model over the stats rollup (see rollup.ts).
//
// Readers use the smallest level that answers the question: per-user totals
// and costs come from stats_user_model, rolling-window token flow from
// stats_user_hour, first/last activity and session context from
// stats_session_live. Only the legacy per-session hourly overview reads level
// 1. Every level carries the live owner, maintained through dirty marks on
// event, owner and soft-delete changes. The metric formulas are the ones the
// raw-event endpoints used before the rollup (user/assistant message counts,
// effective input = input + cache_read + cache_write, "session tokens" = last
// top-level context per session); only the source changed. Rolling 7d/30d
// windows start at the UTC hour containing now − N days.

import type { DuckDBConnection } from '@duckdb/node-api'
import { derivedSource, rollupReady, rollupSource } from './rollup'
import type { EngineFilter } from '../engine-filter'

export type Range = 'all' | '30d' | '7d'
const n = (v: unknown): number => Number(v ?? 0)

const TOKEN_SUM = `(input_tokens + cache_read_tokens + cache_write_tokens)`

/** Hour-aligned start of a rolling window, as a TIMESTAMP literal. */
export function rangeStart(range: Range, now = Date.now()): string | null {
  const days = range === '7d' ? 7 : range === '30d' ? 30 : null
  if (days == null) return null
  const ms = Math.floor((now - days * 86_400_000) / 3_600_000) * 3_600_000
  return new Date(ms).toISOString().slice(0, 19).replace('T', ' ')
}

async function rows<T>(db: DuckDBConnection, sql: string, params: unknown[] = []): Promise<T[]> {
  return (await db.runAndReadAll(sql, params as never[])).getRowObjects() as T[]
}

// ── /api/dashboard/users ────────────────────────────────────────────────

export interface UserTotalsRow {
  user_name: string
  user_inputs: number
  input_uncached: number
  output_tokens: number
  cache_read: number
  cache_creation: number
  first_seen: string | null
  last_seen: string | null
  tokens_7d: number
  tokens_30d: number
}

export async function userTotals(
  db: DuckDBConnection,
  now = Date.now(),
  engine: EngineFilter = null,
): Promise<UserTotalsRow[]> {
  const raw = await rows<Record<string, unknown>>(
    db,
    `WITH t AS (SELECT u, SUM(user_msgs) AS user_inputs, SUM(input_tokens) AS input_uncached,
        SUM(output_tokens) AS output_tokens, SUM(cache_read_tokens) AS cache_read,
        SUM(cache_write_tokens) AS cache_creation
        FROM ${derivedSource('stats_user_model', engine)} WHERE u != '' GROUP BY u),
      s AS (SELECT u, MIN(first_ts) AS first_seen, MAX(last_ts) AS last_seen
        FROM ${derivedSource('stats_session_live', engine)} WHERE u != '' GROUP BY u),
      w AS (SELECT u, SUM(CASE WHEN hour >= ?::TIMESTAMP THEN tk + ot ELSE 0 END) AS tokens_7d,
        SUM(tk + ot) AS tokens_30d
        FROM ${derivedSource('stats_user_hour', engine)} WHERE u != '' AND hour >= ?::TIMESTAMP GROUP BY u)
    SELECT t.*, t.u AS user_name, s.first_seen, s.last_seen,
      COALESCE(w.tokens_7d, 0) AS tokens_7d, COALESCE(w.tokens_30d, 0) AS tokens_30d
    FROM t LEFT JOIN s ON s.u = t.u LEFT JOIN w ON w.u = t.u`,
    [rangeStart('7d', now), rangeStart('30d', now)],
  )
  return raw.map((r) => ({
    user_name: String(r.user_name),
    user_inputs: n(r.user_inputs),
    input_uncached: n(r.input_uncached),
    output_tokens: n(r.output_tokens),
    cache_read: n(r.cache_read),
    cache_creation: n(r.cache_creation),
    first_seen: (r.first_seen as string | null) ?? null,
    last_seen: (r.last_seen as string | null) ?? null,
    tokens_7d: n(r.tokens_7d),
    tokens_30d: n(r.tokens_30d),
  }))
}

export interface UserCostRow {
  user_name: string
  model: string
  long_prompt: boolean
  engine: string
  long_context: boolean
  input_tokens: number
  output_tokens: number
  cache_read_tokens: number
  cache_write_tokens: number
  cache_write_1h_tokens: number
}

export async function userCostRows(db: DuckDBConnection, engine: EngineFilter = null): Promise<UserCostRow[]> {
  const raw = await rows<Record<string, unknown>>(
    db,
    `SELECT u AS user_name, model, engine, long_prompt, long_context, input_tokens, output_tokens, cache_read_tokens,
      cache_write_tokens, cache_write_1h_tokens
    FROM ${derivedSource('stats_user_model', engine)} WHERE u != '' AND assistant_msgs > 0 AND model != ''`,
  )
  return raw.map((r) => ({
    user_name: String(r.user_name),
    model: String(r.model),
    long_prompt: Boolean(r.long_prompt),
    engine: String(r.engine),
    long_context: Boolean(r.long_context),
    input_tokens: n(r.input_tokens),
    output_tokens: n(r.output_tokens),
    cache_read_tokens: n(r.cache_read_tokens),
    cache_write_tokens: n(r.cache_write_tokens),
    cache_write_1h_tokens: n(r.cache_write_1h_tokens),
  }))
}

/** Σ over sessions of the last top-level context, per user ('' key = all users). */
async function sessionContext(
  db: DuckDBConnection,
  where: string,
  params: unknown[],
  engine: EngineFilter = null,
): Promise<Map<string, number>> {
  const raw = await rows<{ u: string | null; g: number; ctx: unknown }>(
    db,
    `SELECT u, GROUPING(u) AS g, SUM(ctx) AS ctx FROM ${derivedSource('stats_session_live', engine)}
    WHERE ctx IS NOT NULL ${where} GROUP BY GROUPING SETS ((u), ())`,
    params,
  )
  const out = new Map<string, number>()
  for (const r of raw) out.set(Number(r.g) === 1 ? '' : (r.u ?? '\0null'), n(r.ctx))
  return out
}

export async function userContextTokens(
  db: DuckDBConnection,
  engine: EngineFilter = null,
): Promise<Map<string, number>> {
  const all = await sessionContext(db, `AND u != ''`, [], engine)
  all.delete('')
  return all
}

// ── /api/dashboard/stats/overview (single user or all) ──────────────────

export async function legacyOverview(
  db: DuckDBConnection,
  o: { range: Range; tokenId: string | null; compact: boolean; tzOffsetMin: number; engine?: EngineFilter },
): Promise<Record<string, unknown>> {
  const since = rangeStart(o.range)
  const own = o.tokenId ? 'AND u = ?' : ''
  const ownP = o.tokenId ? [o.tokenId] : []
  const win = since ? `AND bucket >= TIMESTAMP '${since}'` : ''
  const engine = o.engine ?? null
  const l1 = `${rollupSource(engine)} WHERE u IS NOT NULL ${win} ${own}`

  // Counters and models: the small levels whenever they can answer (all-time,
  // or once built: windows are hour-aligned). Level 1 only for the per-session
  // hourly breakdown of the non-compact answer.
  const small = !engine && (!since || rollupReady())
  const uh = `${derivedSource('stats_user_hour')} WHERE TRUE ${since ? `AND hour >= TIMESTAMP '${since}'` : ''} ${own}`
  const counters = !small
    ? (
        await rows<Record<string, unknown>>(
          db,
          `SELECT COUNT(DISTINCT sid) AS sessions, SUM(user_msgs) AS user_messages,
            SUM(assistant_msgs) AS assistant_messages FROM ${l1}`,
          ownP,
        )
      )[0]
    : (
        await rows<Record<string, unknown>>(
          db,
          `SELECT (SELECT COUNT(*) FROM ${derivedSource('stats_session_live')}
              WHERE TRUE ${since ? `AND last_hour >= TIMESTAMP '${since}'` : ''} ${own}) AS sessions,
            SUM(user_msgs) AS user_messages, SUM(assistant_msgs) AS assistant_messages
          FROM ${since ? uh : `${derivedSource('stats_user_model')} WHERE TRUE ${own}`}`,
          [...ownP, ...ownP],
        )
      )[0]
  const ctx = await sessionContext(db, own, ownP, engine)
  const splits = await engineRollupStats(db, { range: o.range, subject: o.tokenId ?? '*', engine })
  const models = splits[o.tokenId ?? '*']?.models ?? []
  // Compact (heatmap) answer in a whole-hour offset: per-(user, hour) level.
  const compactSmall = !engine && o.compact && rollupReady() && o.tzOffsetMin % 60 === 0
  const hourly = await rows<Record<string, unknown>>(
    db,
    compactSmall
      ? `SELECT strftime(hour, '%Y-%m-%dT%H') AS utc_hour, NULLIF(model, '') AS model, '' AS session_id,
          SUM(user_msgs) AS user_msgs, SUM(assistant_msgs) AS assistant_msgs, SUM(tk) AS model_tokens
        FROM ${uh} GROUP BY hour, model ORDER BY utc_hour`
      : `SELECT strftime(bucket, '%Y-%m-%dT%H') AS utc_hour, model,
          ${o.compact ? `'' AS session_id,` : 'session_id,'}
          SUM(user_msgs) AS user_msgs, SUM(assistant_msgs) AS assistant_msgs, SUM(${TOKEN_SUM}) AS model_tokens
        FROM ${l1} GROUP BY utc_hour, model${o.compact ? '' : ', session_id'} ORDER BY utc_hour`,
    ownP,
  )
  let dailySessions: Array<{ date: string; sessions: number }> | undefined
  if (o.compact) {
    // Distinct sessions per local day from the gap histogram (rollup.ts): a
    // session counts at hour h iff its previous active hour is before the day
    // (or window) start.
    const sinceTs = since ? `TIMESTAMP '${since}'` : `TIMESTAMP '1970-01-01'`
    const daily = await rows<Record<string, unknown>>(
      db,
      compactSmall
        ? `WITH span AS (SELECT COALESCE(MIN(hour), TIMESTAMP '2000-01-01') AS lo,
              COALESCE(MAX(hour), TIMESTAMP '2000-01-01') AS hi FROM stats_user_hour),
            tzm AS (
              SELECT hour, d, date_diff('hour', GREATEST(MIN(hour) OVER (PARTITION BY d), ${sinceTs}), hour) AS off
              FROM (SELECT range AS hour, CAST(range - to_minutes(CAST(? AS BIGINT)) AS DATE) AS d
                FROM range((SELECT lo FROM span), (SELECT hi FROM span) + INTERVAL 1 HOUR, INTERVAL 1 HOUR))
            )
          SELECT strftime(t.d, '%Y-%m-%d') AS local_day, SUM(x.n) FILTER (WHERE x.g > t.off) AS sessions
          FROM stats_user_hour_sessions x JOIN tzm t ON t.hour = x.hour
          WHERE TRUE ${since ? `AND x.hour >= ${sinceTs}` : ''} ${own.replace('u =', 'x.u =')}
          GROUP BY t.d ORDER BY t.d`
        : `SELECT strftime(bucket - to_minutes(CAST(? AS BIGINT)), '%Y-%m-%d') AS local_day,
            COUNT(DISTINCT sid) AS sessions FROM ${l1} GROUP BY local_day ORDER BY local_day`,
      [o.tzOffsetMin, ...ownP],
    )
    dailySessions = daily.map((r) => ({ date: String(r.local_day), sessions: n(r.sessions) }))
  }
  return {
    range: o.range,
    engine: engine ?? 'all',
    engines: splits[o.tokenId ?? '*']?.engines ?? {},
    sessions: n(counters?.sessions),
    userMessages: n(counters?.user_messages),
    assistantMessages: n(counters?.assistant_messages),
    totalTokens: ctx.get('') ?? 0,
    favoriteModel: models[0]?.name ?? null,
    models,
    hourly: hourly.map((r) => ({
      utcHour: String(r.utc_hour),
      model: (r.model as string | null) ?? null,
      ...(o.compact ? {} : { sessionId: r.session_id }),
      userMessages: n(r.user_msgs),
      assistantMessages: n(r.assistant_msgs),
      tokens: n(r.model_tokens),
    })),
    ...(dailySessions ? { dailySessions } : {}),
  }
}

export interface EngineStats {
  sessions: number
  userMessages: number
  assistantMessages: number
  inTokens: number
  outTokens: number
  tokens: { input: number; output: number; cacheRead: number; cacheCreation: number }
}
export interface EngineRollupStats {
  engines: Record<string, EngineStats>
  models: Array<{ name: string; engine: string; in: number; out: number; pct: number }>
}

/** One aggregate pass for the whole roster/grid. Mixed-engine sessions count
 *  in both engine groups; the page's overall session count remains distinct. */
export async function engineRollupStats(
  db: DuckDBConnection,
  o: { range?: Range; subject?: string | null; engine?: EngineFilter } = {},
): Promise<Record<string, EngineRollupStats>> {
  const since = rangeStart(o.range ?? 'all')
  const subject = o.subject
  const own = subject && subject !== '*' ? 'AND u = ?' : ''
  const params = own ? [subject] : []
  const src = `${rollupSource(o.engine)} WHERE u IS NOT NULL ${own} ${since ? `AND bucket >= TIMESTAMP '${since}'` : ''}`
  const out: Record<string, EngineRollupStats> = {}
  const get = (r: Record<string, unknown>) => {
    const k = Number(r.g) === 1 ? '*' : String(r.u)
    return (out[k] ??= { engines: {}, models: [] })
  }
  const groups = 'GROUPING SETS ((u, engine), (engine))'
  // Keep u in the grouping expression even for the all-only request; it is
  // omitted by GROUPING SETS and produces the all-users sentinel.
  const raw = await rows<Record<string, unknown>>(
    db,
    `SELECT GROUPING(u) AS g, u, engine,
      COUNT(DISTINCT sid) AS sessions, SUM(user_msgs) AS um, SUM(assistant_msgs) AS am,
      SUM(input_tokens) AS it, SUM(output_tokens) AS ot, SUM(cache_read_tokens) AS cr, SUM(cache_write_tokens) AS cw
    FROM ${src} GROUP BY ${groups}`,
    params,
  )
  for (const r of raw)
    get(r).engines[String(r.engine)] = {
      sessions: n(r.sessions),
      userMessages: n(r.um),
      assistantMessages: n(r.am),
      inTokens: n(r.it) + n(r.cr) + n(r.cw),
      outTokens: n(r.ot),
      tokens: { input: n(r.it), output: n(r.ot), cacheRead: n(r.cr), cacheCreation: n(r.cw) },
    }
  const modelSource = since ? src : `${derivedSource('stats_user_model', o.engine)} WHERE TRUE ${own}`
  const modelGroups = 'GROUPING SETS ((u, model, engine), (model, engine))'
  const models = await rows<Record<string, unknown>>(
    db,
    `SELECT GROUPING(u) AS g, u, model, engine,
      SUM(${TOKEN_SUM}) AS it, SUM(output_tokens) AS ot
    FROM (SELECT * FROM ${modelSource}) WHERE model IS NOT NULL AND model != ''
    GROUP BY ${modelGroups} HAVING SUM(assistant_msgs) > 0 ORDER BY (it + ot) DESC, model, engine`,
    params,
  )
  for (const r of models)
    get(r).models.push({ name: String(r.model), engine: String(r.engine), in: n(r.it), out: n(r.ot), pct: 0 })
  for (const v of Object.values(out)) {
    const total = v.models.reduce((s, m) => s + m.in + m.out, 0)
    for (const m of v.models) m.pct = total ? ((m.in + m.out) / total) * 100 : 0
  }
  return subject ? (out[subject] ? { [subject]: out[subject] } : {}) : out
}

// Batched user-stats cards: every card of the admin page in one statement over
// the per-(user, hour) rollup level (GROUPING SETS give the per-user and the
// all-users card in the same pass). Local day/hour are computed here in the
// viewer's time zone: per UTC hour for whole-hour zones, per 15-minute bucket
// of level 1 for :30/:45 zones. The browser receives finished numbers and a
// heatmap window instead of thousands of raw hourly buckets per card.
//
// Formulas match the former per-card client aggregation: active days = local
// days with any event, longest streak = consecutive such days, peak hour =
// local hour with most user messages (earliest wins a tie), heatmap tokens =
// effective input per local day, favorite = model with most in+out tokens.

import type { DuckDBConnection } from '@duckdb/node-api'
import { engineRollupStats, rangeStart, type Range, type EngineRollupStats } from './stats-queries'
import { derivedSource, rollupReady, rollupSource } from './rollup'
import type { EngineFilter } from '../engine-filter'

export interface CardData {
  sessions: number
  userMessages: number
  assistantMessages: number
  totalTokens: number
  favoriteModel: string | null
  models: Array<{ name: string; engine?: string; in: number; out: number; pct: number }>
  engines?: EngineRollupStats['engines']
  activeDays: number
  longestStreak: number
  peakHour: number | null
  /** Heatmap window only (26/8/5 weeks back from the local today). */
  days: Array<[string, number, number, number, number, Record<string, number>]>
}

export interface CardsRequest {
  engine?: EngineFilter
  range: Range
  /** IANA zone (preferred) — DST-exact local days. */
  tz?: string | null
  /** JS getTimezoneOffset() minutes, used when no IANA zone is given. */
  tzOffsetMin?: number
  /** Omitted: every user plus '*' (all users). A name: that user. '*': all only. */
  subject?: string | null
  /** Diagnostics/tests: force the 15-minute level-1 path. */
  grain?: 'auto' | 'bucket'
}

const n = (v: unknown): number => Number(v ?? 0)
const TOKEN_SUM = `(x.input_tokens + x.cache_read_tokens + x.cache_write_tokens)`
const WINDOW_WEEKS: Record<Range, number> = { all: 26, '30d': 8, '7d': 5 }

const emptyCard = (): CardData => ({
  sessions: 0,
  userMessages: 0,
  assistantMessages: 0,
  totalTokens: 0,
  favoriteModel: null,
  models: [],
  activeDays: 0,
  longestStreak: 0,
  peakHour: null,
  days: [],
})

/** Valid IANA-looking zone name or null (the value is bound, never spliced). */
export function safeZone(tz: string | null | undefined): string | null {
  return tz && /^[A-Za-z][A-Za-z0-9_+\-/]{0,63}$/.test(tz) ? tz : null
}

/** True when the zone has a non-whole-hour UTC offset anywhere in the data span. */
async function zoneHasPartialHours(db: DuckDBConnection, zone: string): Promise<boolean> {
  const row = (
    await db.runAndReadAll(
      `WITH span AS (SELECT MIN(hour) AS lo, MAX(hour) AS hi FROM stats_user_hour)
      SELECT COALESCE(bool_or(minute(timezone(?, timezone('UTC', range))) <> 0), false) AS odd
      FROM range((SELECT lo FROM span), (SELECT hi FROM span) + INTERVAL 1 HOUR, INTERVAL 1 HOUR)`,
      [zone],
    )
  ).getRowObjects()[0] as { odd?: boolean } | undefined
  return Boolean(row?.odd)
}

/** Cards keyed by user name, '*' = all users. One statement per request. */
export async function cardsOverview(db: DuckDBConnection, q: CardsRequest): Promise<Record<string, CardData>> {
  const zone = safeZone(q.tz)
  const lp: unknown = zone ?? Math.max(-840, Math.min(840, Math.trunc(q.tzOffsetMin ?? 0)))
  const local = (col: string) =>
    zone ? `timezone(?, timezone('UTC', ${col}))` : `(${col} - to_minutes(CAST(? AS BIGINT)))`
  const today = zone
    ? `CAST(timezone(?, now()) AS DATE)`
    : `CAST(timezone('UTC', now()) - to_minutes(CAST(? AS BIGINT)) AS DATE)`
  const winStart = `${today} - ${WINDOW_WEEKS[q.range] * 7 + 7}`
  const since = rangeStart(q.range)
  // A generated ISO literal, never user input.
  const window = (col: string) => (since ? `(${col} >= TIMESTAMP '${since}')` : 'TRUE')

  // page: GROUPING SETS give each user's card and the all-users card in one
  // pass; user: one card; all: one ungrouped card.
  const mode = q.subject == null ? 'page' : q.subject === '*' ? 'all' : 'user'
  const own = (col: string) => (mode === 'user' ? `AND ${col} = ?` : '')
  const sel = mode === 'all' ? `1 AS g, NULL::TEXT AS u` : mode === 'user' ? `0 AS g, u` : `GROUPING(u) AS g, u`
  const by = (keys: string) =>
    mode === 'page'
      ? `GROUP BY GROUPING SETS ((u${keys ? `, ${keys}` : ''}), (${keys}))`
      : mode === 'user'
        ? `GROUP BY u${keys ? `, ${keys}` : ''}`
        : keys
          ? `GROUP BY ${keys}`
          : ''
  // Every branch returns the same columns: k, g, u, model, d, h, ss, um, am, t, o, w, mm.
  // Plain DOUBLE / VARCHAR values: no BigInt or date objects to convert per row.
  const cols = (c: Partial<Record<'model' | 'd' | 'h' | 'ss' | 'um' | 'am' | 't' | 'o' | 'w' | 'mm', string>>) =>
    `${sel}, ${c.model ?? 'NULL::TEXT'} AS model,
     ${c.d ? `strftime(${c.d}, '%Y-%m-%d')` : 'NULL::VARCHAR'} AS d, CAST(${c.h ?? 'NULL'} AS DOUBLE) AS h,
     CAST(${c.ss ?? 'NULL'} AS DOUBLE) AS ss, CAST(${c.um ?? 'NULL'} AS DOUBLE) AS um,
     CAST(${c.am ?? 'NULL'} AS DOUBLE) AS am, CAST(${c.t ?? 'NULL'} AS DOUBLE) AS t,
     CAST(${c.o ?? 'NULL'} AS DOUBLE) AS o, CAST(${c.w ?? 'NULL'} AS BOOLEAN) AS w,
     ${c.mm ?? 'NULL::VARCHAR'} AS mm`
  const valid = `model IS NOT NULL AND model != ''`
  const subjects = (k: number) => (mode === 'user' ? Array<unknown>(k).fill(q.subject) : [])
  const live = derivedSource('stats_session_live', q.engine)

  // Shared tail: the per-hour (or per-bucket) rows `r` → model, day, day×model
  // and hour-of-day branches.
  const tail = `
    UNION ALL SELECT 'x', ${cols({ t: 'SUM(ctx)' })} FROM sl ${by('')}
    UNION ALL SELECT 'm', ${cols({ model: 'model', t: 'SUM(tk)', o: 'SUM(ot)' })}
      FROM r WHERE ${valid} ${by('model')} HAVING SUM(assistant_msgs) > 0
    UNION ALL SELECT 'dm', ${cols({ model: 'model', d: 'd', t: 'SUM(tk)' })}
      FROM r WHERE ${valid} AND in_win ${by('d, model')} HAVING SUM(tk) > 0
    UNION ALL SELECT 'h', ${cols({ h: 'h', um: 'SUM(user_msgs)' })} FROM r ${by('h')}`

  let sql: string
  let params: unknown[]
  if (
    !q.engine &&
    q.grain !== 'bucket' &&
    rollupReady() &&
    (zone ? !(await zoneHasPartialHours(db, zone)) : Number(lp) % 60 === 0)
  ) {
    // Whole-hour zone: read the per-(user, hour) level. The zone is applied
    // once per hour of the data's span (thousands of rows), and distinct
    // sessions per local day come from the gap histogram (rollup.ts).
    const sinceTs = since ? `TIMESTAMP '${since}'` : `TIMESTAMP '1970-01-01'`
    sql = `WITH span AS (SELECT COALESCE(MIN(hour), TIMESTAMP '2000-01-01') AS lo,
          COALESCE(MAX(hour), TIMESTAMP '2000-01-01') AS hi FROM stats_user_hour),
      hs AS (SELECT range AS hour FROM range((SELECT lo FROM span), (SELECT hi FROM span) + INTERVAL 1 HOUR, INTERVAL 1 HOUR)),
      tzl AS (SELECT hour, ${local('hour')} AS lt FROM hs),
      tzm AS (
        SELECT hour, CAST(lt AS DATE) AS d, CAST(hour(lt) AS INTEGER) AS h, CAST(lt AS DATE) >= ${winStart} AS in_win,
          date_diff('hour', GREATEST(MIN(hour) OVER (PARTITION BY CAST(lt AS DATE)), ${sinceTs}), hour) AS off
        FROM tzl
      ),
      r AS (
        SELECT x.u, x.model, t.d, t.h, t.in_win, x.user_msgs, x.assistant_msgs, x.tk, x.ot
        FROM stats_user_hour x JOIN tzm t ON t.hour = x.hour WHERE ${window('x.hour')} ${own('x.u')}
      ),
      ss AS (
        SELECT x.u, t.d, SUM(x.n) FILTER (WHERE x.g > t.off) AS n
        FROM stats_user_hour_sessions x JOIN tzm t ON t.hour = x.hour
        WHERE ${window('x.hour')} ${own('x.u')} GROUP BY x.u, t.d
      ),
      sl AS (SELECT u, last_hour, ctx FROM ${live} WHERE TRUE ${own('u')}),
      -- One pass over the hour rows yields both small inputs below: per
      -- (user, local day, model) and per (user, local hour).
      agg AS MATERIALIZED (
        SELECT GROUPING(d, h) AS gs, u, d, model, h, bool_or(in_win) AS w, SUM(user_msgs) AS um,
          SUM(assistant_msgs) AS am, SUM(tk) AS tk, SUM(ot) AS ot
        FROM r GROUP BY GROUPING SETS ((u, d, model), (u, h))
      ),
      rd AS (SELECT u, d, model, w, um, am, tk, ot FROM agg WHERE gs = 1),
      rh AS (SELECT u, h, um FROM agg WHERE gs = 2)
      SELECT 'c' AS k, ${cols({ um: 'SUM(um)', am: 'SUM(am)' })} FROM rd ${by('')}
      UNION ALL SELECT 'cs', ${cols({ ss: `COUNT(*) FILTER (WHERE ${window('last_hour')})` })} FROM sl ${by('')}
      UNION ALL SELECT 'd', ${cols({
        d: 'd',
        um: 'SUM(um)',
        am: 'SUM(am)',
        t: 'SUM(tk)',
        w: 'bool_or(w)',
        // Heatmap per-model split as a JSON object; for all users it comes pre-summed from 'dma'.
        mm:
          mode === 'all'
            ? undefined
            : `CASE WHEN ${mode === 'page' ? 'GROUPING(u) = 0' : 'TRUE'} THEN CAST(to_json(map_from_entries(list({'k': model, 'v': tk}) FILTER (WHERE w AND ${valid} AND tk > 0))) AS VARCHAR) END`,
      })}
        FROM rd ${by('d')}
      ${
        mode === 'user'
          ? ''
          : `UNION ALL SELECT 'dma', 1 AS g, NULL::TEXT AS u, NULL::TEXT, strftime(d, '%Y-%m-%d'), NULL::DOUBLE, NULL::DOUBLE,
          NULL::DOUBLE, NULL::DOUBLE, NULL::DOUBLE, NULL::DOUBLE, NULL::BOOLEAN, CAST(to_json(map_from_entries(list({'k': model, 'v': tk}))) AS VARCHAR)
        FROM (SELECT d, model, SUM(tk) AS tk FROM rd WHERE w AND ${valid} AND tk > 0 GROUP BY d, model) GROUP BY d`
      }
      UNION ALL SELECT 'ds', ${cols({ d: 'd', ss: 'SUM(n)' })} FROM ss ${by('d')}
      UNION ALL SELECT 'x', ${cols({ t: 'SUM(ctx)' })} FROM sl ${by('')}
      UNION ALL SELECT 'm', ${cols({ model: 'model', t: 'SUM(tk)', o: 'SUM(ot)' })}
        FROM rd WHERE ${valid} ${by('model')} HAVING SUM(am) > 0
      UNION ALL SELECT 'h', ${cols({ h: 'h', um: 'SUM(um)' })} FROM rh ${by('h')}`
    params = [lp, lp, ...subjects(3)]
  } else {
    // Zones with :30/:45 offsets (and the inline fallback before the build)
    // need the 15-minute level 1; the zone is applied per distinct bucket.
    sql = `WITH tzm AS (
        SELECT b AS bucket, CAST(lt AS DATE) AS d, CAST(hour(lt) AS INTEGER) AS h, CAST(lt AS DATE) >= ${winStart} AS in_win
        FROM (SELECT b, ${local('b')} AS lt FROM (SELECT DISTINCT bucket AS b FROM ${rollupSource(q.engine)} WHERE ${window('bucket')}))
      ),
      r AS MATERIALIZED (
        SELECT x.u, x.sid, x.model, t.d, t.h, t.in_win, x.user_msgs, x.assistant_msgs, ${TOKEN_SUM} AS tk, x.output_tokens AS ot
        FROM ${rollupSource(q.engine)} x JOIN tzm t ON t.bucket = x.bucket
        WHERE x.u IS NOT NULL AND ${window('x.bucket')} ${own('x.u')}
      ),
      -- A session has exactly one owner, so per-(user, session, day) rows turn
      -- every distinct-session count into COUNT(*) and the all-users count into
      -- the sum over users: no DISTINCT inside the grouping sets.
      ud AS MATERIALIZED (
        SELECT u, sid, d, SUM(user_msgs) AS um, SUM(assistant_msgs) AS am, SUM(tk) AS t, bool_or(in_win) AS w
        FROM r GROUP BY u, sid, d
      ),
      us AS (SELECT u, sid, SUM(um) AS um, SUM(am) AS am FROM ud GROUP BY u, sid),
      sl AS (SELECT u, ctx FROM ${live} WHERE TRUE ${own('u')})
      SELECT 'c' AS k, ${cols({ ss: 'COUNT(*)', um: 'SUM(um)', am: 'SUM(am)' })} FROM us ${by('')}
      UNION ALL SELECT 'd', ${cols({ d: 'd', um: 'SUM(um)', am: 'SUM(am)', t: 'SUM(t)', ss: 'COUNT(*)', w: 'bool_or(w)' })}
        FROM ud ${by('d')}
      ${tail}`
    params = [lp, lp, ...subjects(2)]
  }
  const result = (await db.runAndReadAll(sql, params as never[])).getRowObjects() as Array<Record<string, unknown>>

  const cards = new Map<string, CardData>()
  const card = (k: string) => {
    let c = cards.get(k)
    if (!c) cards.set(k, (c = emptyCard()))
    return c
  }
  card(mode === 'user' ? String(q.subject) : '*')
  const keyOf = (r: Record<string, unknown>) => (Number(r.g) === 1 ? '*' : String(r.u ?? ''))
  const day = (v: unknown) => String(v).slice(0, 10)

  const dayLists = new Map<string, string[]>()
  const dayModels: Array<Record<string, unknown>> = []
  const daySessions = new Map<string, number>()
  const allSplits = new Map<string, Record<string, number>>()
  const peak = new Map<CardData, number>()
  for (const r of result) {
    const c = card(keyOf(r))
    switch (r.k) {
      case 'c':
        if (r.ss != null) c.sessions = n(r.ss)
        c.userMessages = n(r.um)
        c.assistantMessages = n(r.am)
        break
      case 'cs':
        c.sessions = n(r.ss)
        break
      case 'ds':
        daySessions.set(`${keyOf(r)}\u0000${day(r.d)}`, n(r.ss))
        break
      case 'x':
        // Session tokens keep their historical meaning: all-time, not windowed.
        c.totalTokens = n(r.t)
        break
      case 'm':
        c.models.push({ name: String(r.model), in: n(r.t), out: n(r.o), pct: 0 })
        break
      case 'd': {
        const d = day(r.d)
        const k = keyOf(r)
        const list = dayLists.get(k) ?? []
        list.push(d)
        dayLists.set(k, list)
        if (r.w) {
          c.days.push([
            d,
            n(r.um),
            n(r.am),
            n(r.t),
            n(r.ss),
            r.mm ? (JSON.parse(String(r.mm)) as Record<string, number>) : {},
          ])
        }
        break
      }
      case 'dma':
        allSplits.set(day(r.d), JSON.parse(String(r.mm)) as Record<string, number>)
        break
      case 'dm':
        dayModels.push(r)
        break
      case 'h': {
        // Peak = most user messages; the earliest hour wins a tie.
        const cnt = n(r.um)
        const h = n(r.h)
        const best = peak.get(c) ?? 0
        if (cnt > 0 && (cnt > best || (cnt === best && c.peakHour != null && h < c.peakHour))) {
          c.peakHour = h
          peak.set(c, cnt)
        }
        break
      }
    }
  }
  for (const [k, list] of dayLists) {
    list.sort()
    card(k).activeDays = list.length
    card(k).longestStreak = longestRun(list)
  }
  const slots = new Map<string, Record<string, number>>()
  for (const [k, c] of cards) {
    c.days.sort((a, b) => (a[0] < b[0] ? -1 : a[0] > b[0] ? 1 : 0))
    for (const d of c.days) {
      const key = `${k}\u0000${d[0]}`
      slots.set(key, d[5])
      d[4] = daySessions.get(key) ?? d[4]
      if (k === '*') d[5] = allSplits.get(d[0]) ?? d[5]
    }
  }
  for (const r of dayModels) {
    const slot = slots.get(`${keyOf(r)}\u0000${day(r.d)}`)
    if (slot) slot[String(r.model)] = n(r.t)
  }

  const out: Record<string, CardData> = {}
  const splits = await engineRollupStats(db, q)
  for (const [k, c] of cards) {
    if (k === '') continue // sessions without an owner only count in "all"
    c.models.sort((a, b) => b.in + b.out - (a.in + a.out) || (a.name < b.name ? -1 : a.name > b.name ? 1 : 0))
    const total = c.models.reduce((s, m) => s + m.in + m.out, 0)
    for (const m of c.models) m.pct = total > 0 ? ((m.in + m.out) / total) * 100 : 0
    c.favoriteModel = c.models[0]?.name ?? null
    c.engines = splits[k]?.engines ?? {}
    c.models = splits[k]?.models ?? []
    c.favoriteModel = c.models[0]?.name ?? null
    out[k] = c
  }
  return out
}
/** Longest run of consecutive calendar dates in a sorted YYYY-MM-DD list. */
export function longestRun(sorted: string[]): number {
  let longest = 0
  let run = 0
  let prev = NaN
  for (const d of sorted) {
    const t = Date.parse(`${d}T00:00:00Z`)
    run = t - prev === 86_400_000 ? run + 1 : 1
    if (run > longest) longest = run
    prev = t
  }
  return longest
}

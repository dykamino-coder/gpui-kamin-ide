import 'reflect-metadata'
import { mkdtempSync, mkdirSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { afterAll, beforeAll, describe, expect, it, vi } from 'vitest'
import type { DuckDBConnection } from '@duckdb/node-api'

// Oracle = the raw jsonl_accounting_events SQL the endpoints ran before the
// rollup. Every rollup-derived number must match it on a synthetic corpus.

let dir: string
let lc: typeof import('./lifecycle')
let rollup: typeof import('./rollup')
let q: typeof import('./stats-queries')
let cards: typeof import('./cards-overview')
let db: DuckDBConnection

const RAW = `FROM jsonl_accounting_events e
  INNER JOIN session_tokens st ON st.session_id = e.session_id AND st.deleted = 0
  WHERE (e.model IS NULL OR e.model != '<synthetic>')`
const TOKEN_SUM = `(COALESCE(e.input_tokens,0) + COALESCE(e.cache_read_tokens,0) + COALESCE(e.cache_write_tokens,0))`

async function all(sql: string, params: unknown[] = []): Promise<Array<Record<string, unknown>>> {
  return (await db.runAndReadAll(sql, params as never[])).getRowObjects() as Array<Record<string, unknown>>
}
const num = (rs: Array<Record<string, unknown>>) =>
  rs.map((r) => Object.fromEntries(Object.entries(r).map(([k, v]) => [k, typeof v === 'bigint' ? Number(v) : v])))

beforeAll(async () => {
  dir = mkdtempSync(join(tmpdir(), 'bridge-rollup-'))
  vi.spyOn(process, 'cwd').mockReturnValue(dir)
  vi.resetModules()
  lc = await import('./lifecycle')
  rollup = await import('./rollup')
  q = await import('./stats-queries')
  cards = await import('./cards-overview')
  await lc.initStatsDb()
  db = await lc.getDb()
  const { generateSyntheticStats } = await import('./synthetic-stats')
  await generateSyntheticStats(db, {
    users: 6,
    sessions: 400,
    eventsPerSession: 45,
    days: 60,
    endIso: new Date().toISOString().slice(0, 19),
  })
}, 120_000)

afterAll(async () => {
  await lc.closeDb()
  vi.restoreAllMocks()
  rmSync(dir, { recursive: true, force: true })
})

async function rawUserTotals() {
  return num(
    await all(`SELECT st.user_name AS user_name, SUM(CASE WHEN e.type='user' THEN 1 ELSE 0 END) AS user_inputs,
      SUM(CASE WHEN e.type='assistant' THEN COALESCE(e.input_tokens,0) ELSE 0 END) AS input_uncached,
      SUM(CASE WHEN e.type='assistant' THEN COALESCE(e.output_tokens,0) ELSE 0 END) AS output_tokens,
      SUM(CASE WHEN e.type='assistant' THEN COALESCE(e.cache_read_tokens,0) ELSE 0 END) AS cache_read,
      SUM(CASE WHEN e.type='assistant' THEN COALESCE(e.cache_write_tokens,0) ELSE 0 END) AS cache_creation,
      MIN(e.timestamp) AS first_seen, MAX(e.timestamp) AS last_seen
      ${RAW} AND st.user_name IS NOT NULL AND st.user_name != '' GROUP BY 1 ORDER BY 1`),
  )
}

async function rawCost() {
  return num(
    await all(`SELECT st.user_name AS user_name, e.model AS model,
      (${TOKEN_SUM} > 100000) AS long_prompt,
      SUM(COALESCE(e.input_tokens,0)) AS input_tokens, SUM(COALESCE(e.output_tokens,0)) AS output_tokens,
      SUM(COALESCE(e.cache_read_tokens,0)) AS cache_read_tokens, SUM(COALESCE(e.cache_write_tokens,0)) AS cache_write_tokens,
      SUM(COALESCE(e.cache_write_1h_tokens,0)) AS cache_write_1h_tokens
      ${RAW} AND st.user_name IS NOT NULL AND st.user_name != '' AND e.type = 'assistant'
        AND e.model IS NOT NULL AND e.model != ''
      GROUP BY 1, 2, 3 ORDER BY 1, 2, 3`),
  )
}

async function rawCtx(userFilter = '') {
  return num(
    await all(`SELECT st.user_name AS u, SUM(ctx) AS ctx FROM (
      SELECT e.session_id, arg_max(${TOKEN_SUM}, {'t': e.timestamp, 'u': e.uuid}) AS ctx FROM jsonl_accounting_events e
      WHERE e.type = 'assistant' AND COALESCE(e.is_subagent,0) = 0 AND (e.model IS NULL OR e.model != '<synthetic>')
      GROUP BY 1) s INNER JOIN session_tokens st ON st.session_id = s.session_id AND st.deleted = 0
      WHERE st.user_name IS NOT NULL AND st.user_name != '' ${userFilter} GROUP BY 1 ORDER BY 1`),
  )
}

async function checkUsers(): Promise<void> {
  const totals = (await q.userTotals(db)).sort((a, b) => a.user_name.localeCompare(b.user_name))
  expect(totals.map(({ tokens_7d: _a, tokens_30d: _b, ...r }) => r)).toEqual(await rawUserTotals())
  const cost = (await q.userCostRows(db)).sort(
    (a, b) =>
      a.user_name.localeCompare(b.user_name) || a.model.localeCompare(b.model) || +a.long_prompt - +b.long_prompt,
  )
  expect(cost).toEqual(await rawCost())
  const ctx = await q.userContextTokens(db)
  for (const r of await rawCtx()) expect(ctx.get(String(r.u))).toBe(r.ctx)
}

async function checkOverview(range: 'all' | '30d' | '7d', tokenId: string | null): Promise<void> {
  const since = q.rangeStart(range)
  const f = [
    since ? `e.timestamp >= '${since.replace(' ', 'T')}'` : 'TRUE',
    tokenId ? `st.user_name = '${tokenId}'` : 'TRUE',
  ].join(' AND ')
  const got = (await q.legacyOverview(db, { range, tokenId, compact: false, tzOffsetMin: 0 })) as any
  const counters = num(
    await all(`SELECT COUNT(DISTINCT e.session_id) AS sessions, SUM(CASE WHEN e.type='user' THEN 1 ELSE 0 END) AS um,
      SUM(CASE WHEN e.type='assistant' THEN 1 ELSE 0 END) AS am ${RAW} AND ${f}`),
  )[0]!
  expect([got.sessions, got.userMessages, got.assistantMessages]).toEqual([
    counters.sessions,
    counters.um ?? 0,
    counters.am ?? 0,
  ])
  const models = num(
    await all(`SELECT e.model AS name, SUM(${TOKEN_SUM}) AS "in", SUM(COALESCE(e.output_tokens,0)) AS "out"
      ${RAW} AND ${f} AND e.type='assistant' AND e.model IS NOT NULL AND e.model != ''
      GROUP BY 1 ORDER BY ("in" + "out") DESC, 1`),
  )
  expect(got.models.map((m: any) => ({ name: m.name, in: m.in, out: m.out }))).toEqual(models)
  const hourly = num(
    await all(`SELECT strftime(e.timestamp::TIMESTAMP, '%Y-%m-%dT%H') AS utcHour, e.model AS model, e.session_id AS sessionId,
      SUM(CASE WHEN e.type='user' THEN 1 ELSE 0 END) AS userMessages,
      SUM(CASE WHEN e.type='assistant' THEN 1 ELSE 0 END) AS assistantMessages,
      SUM(CASE WHEN e.type='assistant' THEN ${TOKEN_SUM} ELSE 0 END) AS tokens
      ${RAW} AND ${f} GROUP BY 1, 2, 3`),
  )
  const key = (h: any) => `${h.utcHour}|${h.model}|${h.sessionId}`
  const sort = (xs: any[]) => [...xs].sort((a, b) => key(a).localeCompare(key(b)))
  expect(sort(got.hourly)).toEqual(sort(hourly))
  const ctx = await rawCtx(tokenId ? `AND st.user_name = '${tokenId}'` : '')
  if (tokenId) expect(got.totalTokens).toBe(ctx[0]?.ctx ?? 0)

  // Compact (heatmap) answer: per (UTC hour, model) and distinct sessions per local day.
  for (const tzOffsetMin of [-180, -330]) {
    const compact = (await q.legacyOverview(db, { range, tokenId, compact: true, tzOffsetMin })) as any
    expect([compact.sessions, compact.userMessages, compact.assistantMessages]).toEqual([
      counters.sessions,
      counters.um ?? 0,
      counters.am ?? 0,
    ])
    expect(compact.models).toEqual(got.models)
    const ch = num(
      await all(`SELECT strftime(e.timestamp::TIMESTAMP, '%Y-%m-%dT%H') AS utcHour, NULLIF(e.model, '') AS model,
        SUM(CASE WHEN e.type='user' THEN 1 ELSE 0 END) AS userMessages,
        SUM(CASE WHEN e.type='assistant' THEN 1 ELSE 0 END) AS assistantMessages,
        SUM(CASE WHEN e.type='assistant' THEN ${TOKEN_SUM} ELSE 0 END) AS tokens
        ${RAW} AND ${f} GROUP BY 1, 2`),
    )
    const ck = (h: any) => `${h.utcHour}|${h.model ?? ''}`
    const csort = (xs: any[]) =>
      [...xs].map((h) => ({ ...h, model: h.model || null })).sort((a, b) => ck(a).localeCompare(ck(b)))
    expect(csort(compact.hourly)).toEqual(csort(ch))
    const daily = num(
      await all(`SELECT strftime(e.timestamp::TIMESTAMP - INTERVAL (${tzOffsetMin}) MINUTE, '%Y-%m-%d') AS date,
        COUNT(DISTINCT e.session_id) AS sessions ${RAW} AND ${f} GROUP BY 1 ORDER BY 1`),
    )
    expect(compact.dailySessions).toEqual(daily)
  }
}

// Moscow (UTC+3) reads the per-hour level; Kolkata (UTC+5:30) the 15-minute one.
const ZONES = { moscow: ['Europe/Moscow', 180], kolkata: ['Asia/Kolkata', 330] } as const

async function checkCards(range: 'all' | '30d' | '7d', zoneKey: keyof typeof ZONES = 'moscow'): Promise<void> {
  const [zone, offset] = ZONES[zoneKey]
  const since = q.rangeStart(range)
  const f = since ? `e.timestamp >= '${since.replace(' ', 'T')}'` : 'TRUE'
  const got = await cards.cardsOverview(db, { range, tz: zone })
  const single = await cards.cardsOverview(db, { range, tzOffsetMin: -offset, subject: 'user00' })
  expect(single.user00).toEqual(got.user00)
  // The per-hour path and the 15-minute path agree card for card (heatmap split included).
  if (rollup.rollupReady()) expect(await cards.cardsOverview(db, { range, tz: zone, grain: 'bucket' })).toEqual(got)
  const allOnly = await cards.cardsOverview(db, { range, tz: zone, subject: '*' })
  expect(allOnly['*']).toEqual(got['*'])
  const local = `(e.timestamp::TIMESTAMP + INTERVAL ${offset} MINUTE)` // neither zone has DST
  const days = num(
    await all(`SELECT COALESCE(st.user_name, '') AS u, strftime(${local}, '%Y-%m-%d') AS d,
      COUNT(DISTINCT e.session_id) AS ss, SUM(CASE WHEN e.type='user' THEN 1 ELSE 0 END) AS um
      ${RAW} AND ${f} GROUP BY ALL ORDER BY 1, 2`),
  )
  const hours = num(
    await all(`SELECT COALESCE(st.user_name, '') AS u, hour(${local}) AS h, SUM(CASE WHEN e.type='user' THEN 1 ELSE 0 END) AS um
      ${RAW} AND ${f} GROUP BY ALL ORDER BY 1, 2`),
  )
  const users = [...new Set(days.map((d) => String(d.u)))].filter(Boolean)
  // Owners with all-time data but nothing in the window keep an empty card.
  const active = Object.keys(got).filter((k) => k === '*' || got[k]!.activeDays > 0)
  expect(active.sort()).toEqual(['*', ...users].sort())
  for (const u of users) {
    const ud = days.filter((d) => d.u === u)
    const c = got[u]!
    expect(c.activeDays).toBe(ud.length)
    expect(c.longestStreak).toBe(cards.longestRun(ud.map((d) => String(d.d))))
    const uh = hours.filter((h) => h.u === u && Number(h.um) > 0)
    const peak = uh.reduce<any>((b, h) => (!b || Number(h.um) > Number(b.um) ? h : b), null)
    expect(c.peakHour).toBe(peak ? Number(peak.h) : null)
    for (const day of c.days) {
      const raw = ud.find((d) => d.d === day[0])!
      expect([day[1], day[4]]).toEqual([raw.um, raw.ss])
    }
  }
  const allDays = num(await all(`SELECT COUNT(DISTINCT strftime(${local}, '%Y-%m-%d')) AS n ${RAW} AND ${f}`))[0]!
  expect(got['*']!.activeDays).toBe(allDays.n)
}

/** Every delta-maintained level equals its recompute from scratch. */
async function checkDerived(): Promise<void> {
  const l1 = `(${rollup.rollupSelect()})`
  const levels: Array<[string, string]> = [
    ['stats_user_model', rollup.userModelSelect(l1)],
    ['stats_user_hour', rollup.userHourSelect(l1)],
    ['stats_user_hour_sessions', rollup.userHourSessionsSelect(l1)],
    ['stats_session_live', rollup.sessionLiveSelect(l1)],
  ]
  for (const [table, fresh] of levels) {
    const diff = await all(
      `SELECT COUNT(*) AS n FROM ((SELECT * FROM ${table} EXCEPT ALL SELECT * FROM (${fresh}))
        UNION ALL (SELECT * FROM (${fresh}) EXCEPT ALL SELECT * FROM ${table}))`,
    )
    expect([table, diff[0]!.n]).toEqual([table, 0n])
  }
}

describe('stats rollup equals the raw-event formulas', () => {
  it('inline fallback (before the build) matches the raw oracle', async () => {
    expect(rollup.rollupReady()).toBe(false)
    await checkUsers()
    await checkOverview('all', null)
  }, 60_000)

  it('background build is resumable and matches for every range and user', async () => {
    await rollup.startRollupBuild(() => lc.getDb())
    expect(rollup.rollupReady()).toBe(true)
    await checkDerived()
    await checkUsers()
    for (const range of ['all', '30d', '7d'] as const) {
      await checkOverview(range, null)
      await checkOverview(range, 'user01')
      await checkCards(range)
      await checkCards(range, 'kolkata')
    }
  }, 120_000)

  it('incremental refresh after sweeper/proxy writes equals a full recompute', async () => {
    const before = rollup.rollupRevision()
    const folder = join(dir, 'projects', 'slug')
    mkdirSync(folder, { recursive: true })
    const sid = (await all(`SELECT session_id FROM session_tokens WHERE deleted = 0 LIMIT 1`))[0]!.session_id as string
    const ts = new Date().toISOString()
    const lines = [
      { type: 'user', uuid: 'inc-u1', timestamp: ts, message: { content: 'hello' } },
      {
        type: 'assistant',
        uuid: 'inc-a1',
        timestamp: ts,
        message: {
          id: 'inc-msg',
          model: 'claude-opus-5-5',
          content: [],
          usage: { input_tokens: 7, output_tokens: 9, cache_read_input_tokens: 120000 },
        },
      },
    ]
    writeFileSync(join(folder, `${sid}.jsonl`), lines.map((l) => JSON.stringify(l)).join('\n') + '\n')
    const sweeper = await import('../jsonl-sweeper')
    await sweeper.sweepOneByPath(join(folder, `${sid}.jsonl`))
    const { recordProxyUsage } = await import('../proxy-usage')
    await recordProxyUsage(
      {
        timestamp: ts,
        isSidechain: false,
        message: { id: 'inc-side', model: 'claude-haiku-5-5', usage: { input_tokens: 3, output_tokens: 4 } },
      } as any,
      { sessionId: sid, tokenId: 'tok-user00', userName: 'user00' },
    )
    // Distinct sessions per local day without a session axis: one session with
    // multi-hour gaps on two days, one straddling the 7d window start.
    const since = Date.parse(q.rangeStart('7d')!.replace(' ', 'T') + 'Z')
    const gapDay = Date.now() - 3 * 86_400_000
    const at = (ms: number) => new Date(ms).toISOString()
    for (const [id, times] of [
      ['gap-session', [gapDay, gapDay + 4 * 3_600_000, gapDay + 26 * 3_600_000, gapDay + 27 * 3_600_000]],
      ['edge-session', [since - 2 * 3_600_000, since + 3_600_000, since + 90 * 60_000]],
    ] as const) {
      await db.run(
        `INSERT INTO session_tokens (session_id, token_id, user_name, created_at, deleted) VALUES (?, 'tok-user03', 'user03', ?, 0)`,
        [id, ts],
      )
      const entries = times.map((t, i) => ({
        type: 'user',
        uuid: `${id}-${i}`,
        timestamp: at(t),
        message: { content: 'x' },
      }))
      writeFileSync(join(folder, `${id}.jsonl`), entries.map((l) => JSON.stringify(l)).join('\n') + '\n')
      await sweeper.sweepOneByPath(join(folder, `${id}.jsonl`))
    }
    await rollup.drainRollup(() => lc.getDb())
    expect(rollup.rollupRevision()).not.toBe(before)
    expect((await all('SELECT COUNT(*) AS n FROM stats_rollup_dirty'))[0]!.n).toBe(0n)
    const cols =
      'session_id, bucket, model, engine, long_prompt, events, user_msgs, assistant_msgs, input_tokens, output_tokens, cache_read_tokens, cache_write_tokens, last_ctx'
    const diff = await all(
      `SELECT COUNT(*) AS n FROM ((SELECT ${cols} FROM stats_rollup EXCEPT ALL SELECT ${cols} FROM (${rollup.rollupSelect()}))
        UNION ALL (SELECT ${cols} FROM (${rollup.rollupSelect()}) EXCEPT ALL SELECT ${cols} FROM stats_rollup))`,
    )
    if (diff[0]!.n !== 0n)
      console.log(
        await all(`SELECT 'table' AS src, ${cols} FROM stats_rollup EXCEPT ALL SELECT 'table', ${cols} FROM (${rollup.rollupSelect()})
          UNION ALL (SELECT 'fresh', ${cols} FROM (${rollup.rollupSelect()}) EXCEPT ALL SELECT 'fresh', ${cols} FROM stats_rollup)`),
      )
    expect(diff[0]!.n).toBe(0n)
    await checkDerived()
    await checkUsers()
    await checkOverview('7d', null)
    for (const range of ['all', '7d'] as const) {
      await checkCards(range)
      await checkCards(range, 'kolkata')
    }
  }, 120_000)

  it('soft-delete refreshes only that session at every level', async () => {
    const sid = String(
      (await all(`SELECT session_id FROM session_tokens WHERE user_name = 'user02' AND deleted = 0 LIMIT 1`))[0]!
        .session_id,
    )
    await db.run('UPDATE session_tokens SET deleted = 1 WHERE session_id = ?', [sid])
    // As the soft-delete route does.
    await rollup.markRollupSessionsDirty(db, [sid])
    await rollup.drainRollup(() => lc.getDb())
    await checkDerived()
    await checkUsers()
    await checkOverview('all', null)
    await checkCards('all')
    await checkCards('30d', 'kolkata')
  }, 60_000)
})

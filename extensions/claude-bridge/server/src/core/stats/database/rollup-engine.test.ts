import 'reflect-metadata'
import { mkdtempSync, mkdirSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { afterAll, beforeAll, expect, it, vi } from 'vitest'
import { Hono } from 'hono'
import type { DuckDBConnection } from '@duckdb/node-api'
import { OPENAI_LONG_CONTEXT_TOKENS } from '../../../shared/openai-pricing'

let dir: string
let db: DuckDBConnection
let lc: typeof import('./lifecycle')
let rollup: typeof import('./rollup')
let queries: typeof import('./stats-queries')
let accounting: typeof import('./accounting-schema')
let app: Hono
const timestamp = new Date().toISOString()

beforeAll(async () => {
  dir = mkdtempSync(join(tmpdir(), 'bridge-rollup-engine-'))
  vi.spyOn(process, 'cwd').mockReturnValue(dir)
  vi.resetModules()
  lc = await import('./lifecycle')
  rollup = await import('./rollup')
  queries = await import('./stats-queries')
  accounting = await import('./accounting-schema')
  await lc.initStatsDb()
  db = await lc.getDb()
  for (const id of ['mixed', 'codex', 'phantom'])
    await db.run(
      `INSERT INTO session_tokens (session_id, token_id, user_name, created_at, deleted) VALUES (?, 'token', 'alice', ?, 0)`,
      [id, timestamp],
    )
  const insert = async (uuid: string, session: string, type: string, engine: string | null, input = 0) =>
    db.run(
      `INSERT INTO jsonl_events (uuid, session_id, timestamp, type, model, input_tokens, output_tokens,
      cache_read_tokens, cache_write_tokens, cache_write_1h_tokens, message_id, token_id, is_subagent, engine)
    VALUES (?, ?, ?, ?, ?, ?, 0, 0, 0, 0, ?, 'token', 0, ?)`,
      [
        uuid,
        session,
        timestamp,
        type,
        type === 'assistant' ? (engine === 'codex' ? 'gpt-5.5' : 'claude-sonnet-5') : null,
        input,
        uuid,
        engine,
      ],
    )
  await insert('cu', 'mixed', 'user', null)
  await insert('ca', 'mixed', 'assistant', null, 17)
  await insert('xu', 'mixed', 'user', 'codex')
  await insert('xa', 'mixed', 'assistant', 'codex', 150000)
  await insert('xhigh', 'codex', 'assistant', 'codex', OPENAI_LONG_CONTEXT_TOKENS + 1)
  await insert('old-item', 'phantom', 'assistant', 'codex')
  await db.run(
    `INSERT INTO proxy_usage (message_id, session_id, timestamp, model, input_tokens, token_id)
    VALUES ('proxy-only', 'mixed', ?, 'claude-sonnet-5', 3, 'token')`,
    [timestamp],
  )
  app = new Hono()
  ;(await import('../../dashboard/controllers/stats')).registerStatsRoutes(app)
  ;(await import('../../dashboard/controllers/users')).registerUserRoutes(app)
})

afterAll(async () => {
  await lc.closeDb()
  vi.restoreAllMocks()
  rmSync(dir, { recursive: true, force: true })
})

async function json(url: string) {
  const response = await app.request(url)
  expect(response.status).toBe(200)
  return response.json() as Promise<any>
}

it('preserves engine in raw, canonical and proxy-only accounting branches, including indexed drain reads', async () => {
  for (const indexed of [undefined, { proxyMessageIds: ['proxy-only'] }]) {
    const rows = (
      await db.runAndReadAll(accounting.accountingEventsSelect("session_id = 'mixed'", indexed))
    ).getRowObjects()
    expect(rows.map((r) => [r.uuid, r.engine]).sort()).toEqual([
      ['ca', 'claude'],
      ['cu', 'claude'],
      ['proxy:proxy-only', 'claude'],
      ['xa', 'codex'],
      ['xu', 'codex'],
    ])
  }
})

it('keeps mixed-engine counts, filtered contexts and per-request Codex cost tiers through build and indexed replacement', async () => {
  for (const built of [false, true]) {
    if (built) await rollup.startRollupBuild(async () => db)
    const all = built
      ? await json('/api/dashboard/stats/overview?agg=hm')
      : await queries.legacyOverview(db, { range: 'all', tokenId: null, compact: true, tzOffsetMin: 0 })
    const codex = built
      ? await json('/api/dashboard/stats/overview?agg=hm&engine=codex')
      : await queries.legacyOverview(db, {
          range: 'all',
          tokenId: null,
          compact: true,
          tzOffsetMin: 0,
          engine: 'codex',
        })
    expect(all.sessions).toBe(3)
    expect(all.engines.claude.sessions).toBe(1)
    expect(all.engines.codex.sessions).toBe(3)
    expect(codex.assistantMessages).toBe(3)
    expect(codex.models.every((m: any) => m.engine === 'codex')).toBe(true)
    expect(codex.totalTokens).toBe(150000 + OPENAI_LONG_CONTEXT_TOKENS + 1)
    const costs = await queries.userCostRows(db, 'codex')
    expect(
      costs
        .filter((r) => r.input_tokens > 0)
        .map((r) => [r.long_context, r.input_tokens])
        .sort(),
    ).toEqual([
      [false, 150000],
      [true, OPENAI_LONG_CONTEXT_TOKENS + 1],
    ])
    const page = await queries.userSessionsPage(db, 'alice', { limit: 1, engine: 'codex' })
    expect(page.total).toBe(3)
    expect(page.rows[0]!.engines).toBe('codex')
    const next = await queries.userSessionsPage(db, 'alice', {
      limit: 1,
      before: `${page.rows[0]!.last_activity_at}|${page.rows[0]!.session_id}`,
      engine: 'codex',
    })
    expect(next.rows[0]!.session_id).not.toBe(page.rows[0]!.session_id)
    expect(next.rows[0]!.engines).toBe('codex')
    if (!built) {
      const pending = await json('/api/dashboard/stats/users-overview?engine=codex')
      expect(pending.building.ready).toBe(false)
      continue
    }
    const users = await json('/api/dashboard/users?engine=codex')
    expect(users[0].cost).toBe(0)
    expect(users[0].codex.apiEquivalentUsd).toBeGreaterThan(0)
    expect(Object.keys(users[0].engines)).toEqual(['codex'])
    const cards = await json('/api/dashboard/stats/users-overview?engine=codex')
    expect(cards.cards['*'].assistantMessages).toBe(3)
    expect(cards.cards.alice.engines.codex.sessions).toBe(3)
    expect(cards.cards.alice.models[0].engine).toBe('codex')
  }
  await db.run(`UPDATE jsonl_events SET input_tokens = 42 WHERE uuid = 'xa'`)
  await rollup.markRollupSessionsDirty(db, ['mixed'])
  await rollup.drainRollup(async () => db)
  const got = await queries.legacyOverview(db, {
    range: 'all',
    tokenId: null,
    compact: true,
    tzOffsetMin: 0,
    engine: 'codex',
  })
  expect(got.totalTokens).toBe(42 + OPENAI_LONG_CONTEXT_TOKENS + 1)
}, 60_000)

it('invalidates a built rollup when a full rescan only deletes a phantom and inserts no rows', async () => {
  await rollup.startRollupBuild(async () => db)
  const folder = join(dir, 'transcripts')
  mkdirSync(folder)
  const path = join(folder, 'phantom.jsonl')
  writeFileSync(
    path,
    JSON.stringify({
      uuid: 'old-item',
      sessionId: 'phantom',
      engine: 'codex',
      type: 'assistant',
      timestamp,
      message: {
        id: 'old-item',
        role: 'assistant',
        model: 'gpt-5.5',
        content: [{ type: 'text', text: 'projection, not usage' }],
      },
    }) + '\n',
  )
  const sweeper = await import('../jsonl-sweeper')
  await sweeper.sweepOneByPath(path)
  expect(
    (await db.runAndReadAll(`SELECT * FROM stats_rollup_dirty WHERE session_id = 'phantom'`)).getRowObjects(),
  ).toHaveLength(1)
  await rollup.drainRollup(async () => db)
  expect(
    (await db.runAndReadAll(`SELECT * FROM stats_rollup WHERE session_id = 'phantom'`)).getRowObjects(),
  ).toHaveLength(0)
  const codex = await queries.legacyOverview(db, {
    range: 'all',
    tokenId: null,
    compact: true,
    tzOffsetMin: 0,
    engine: 'codex',
  })
  expect(codex.assistantMessages).toBe(2)
})

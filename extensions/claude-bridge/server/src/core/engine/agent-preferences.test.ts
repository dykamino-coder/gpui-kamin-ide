import { afterAll, beforeAll, expect, it, vi } from 'vitest'
import { DuckDBInstance, type DuckDBConnection } from '@duckdb/node-api'
import { initSchema } from '../stats/database/schema'

let instance: DuckDBInstance
let db: DuckDBConnection
vi.mock('../stats/database/lifecycle', () => ({ getDb: async () => db }))

beforeAll(async () => {
  instance = await DuckDBInstance.create(':memory:')
  db = await instance.connect()
  await initSchema(db)
})
afterAll(() => {
  db.closeSync()
  instance.closeSync()
})

it('stores the main agent per token on the real schema, defaulting to Claude', async () => {
  const prefs = await import('./agent-preferences')
  expect(await prefs.getDefaultEngine('t1')).toBe('claude')
  await prefs.setDefaultEngine('t1', 'codex')
  await prefs.setDefaultEngine('t2', 'claude')
  prefs.invalidateDefaultEngineCache()
  expect(await prefs.getDefaultEngine('t1')).toBe('codex')
  expect(await prefs.getDefaultEngine('t2')).toBe('claude')
  await prefs.setDefaultEngine('t1', 'claude')
  prefs.invalidateDefaultEngineCache()
  expect(await prefs.getDefaultEngine('t1')).toBe('claude')
  // A preference does not create a streaming-settings row (its defaults would turn MITM on).
  const rows = (await db.runAndReadAll('SELECT count(*) AS n FROM user_token_settings')).getRowObjects()
  expect(Number(rows[0]!.n)).toBe(0)
})

it('has the engine column on session_tokens', async () => {
  await db.run(
    `INSERT INTO session_tokens (session_id, token_id, user_name, cwd, title, created_at, deleted, engine)
     VALUES ('c1', 't1', 'u', NULL, NULL, '2026-11-12T00:00:00Z', 0, 'codex')`,
  )
  const rows = (await db.runAndReadAll(`SELECT engine FROM session_tokens WHERE session_id = 'c1'`)).getRowObjects()
  expect(rows[0]!.engine).toBe('codex')
})

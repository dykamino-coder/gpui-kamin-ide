import { afterAll, beforeAll, expect, it, vi } from 'vitest'
import { DuckDBInstance, type DuckDBConnection } from '@duckdb/node-api'
import { initSchema } from '../stats/database/schema'
import { persistedContextTokens, ptySessionCounters } from './pty-session-counters'

const mocks = vi.hoisted(() => ({ getDb: vi.fn(), warnLog: vi.fn() }))
vi.mock('../stats/database/lifecycle', () => ({ getDb: mocks.getDb }))
vi.mock('../logging', () => ({ warnLog: mocks.warnLog, debugLog: vi.fn() }))
let instance: DuckDBInstance
let db: DuckDBConnection
const source = (id: string, contextTokens = 0) => ({
  cliConversationId: id,
  contextTokens,
  userMessages: 17,
  assistantMessages: 13,
  totalTokens: 45000,
})

beforeAll(async () => {
  instance = await DuckDBInstance.create(':memory:')
  db = await instance.connect()
  mocks.getDb.mockResolvedValue(db)
  await initSchema(db)
  await db.run(`INSERT INTO session_tokens(session_id,token_id,created_at,context_tokens) VALUES
    ('resumed', 'synthetic', '2026-11-13', 7000),
    ('persisted', 'synthetic', '2026-11-13', 8000),
    ('empty', 'synthetic', '2026-11-13', 0)`)
})
afterAll(() => {
  db?.disconnectSync()
  instance?.closeSync()
})

it('reads persisted context using the production DuckDB schema', async () => {
  const inherited = await persistedContextTokens([
    source('resumed'),
    source('persisted'),
    source('missing'),
    source('empty'),
  ])
  expect(inherited.get('resumed')).toBe(7000)
  expect(inherited.get('persisted')).toBe(8000)
  expect(inherited.has('missing')).toBe(false)
  expect(inherited.has('empty')).toBe(false)
  expect(mocks.warnLog).not.toHaveBeenCalled()
})

it('keeps live counters and prefers the live context over history', () => {
  expect(ptySessionCounters(source('resumed', 12000), new Map([['resumed', 7000]]))).toEqual({
    userMessages: 17,
    assistantMessages: 13,
    contextTokens: 12000,
    totalTokens: 45000,
  })
  expect(ptySessionCounters(source('resumed'), new Map([['resumed', 7000]]))).toMatchObject({ contextTokens: 7000 })
})

it('batches 48 resumed sessions into one database read', async () => {
  const read = vi.spyOn(db, 'runAndReadAll')
  await persistedContextTokens(Array.from({ length: 48 }, () => source('resumed')))
  expect(read).toHaveBeenCalledTimes(1)
  read.mockRestore()
  mocks.getDb.mockClear()
  await persistedContextTokens([source('resumed', 12000)])
  expect(mocks.getDb).not.toHaveBeenCalled()
})

it('logs a failed context query with its reason while retaining live totals', async () => {
  mocks.getDb.mockRejectedValueOnce(new Error('synthetic query failure'))
  const inherited = await persistedContextTokens([source('resumed')])
  expect(mocks.warnLog).toHaveBeenCalledWith(
    expect.stringContaining('query failed'),
    expect.objectContaining({ error: expect.stringContaining('synthetic query failure') }),
  )
  expect(ptySessionCounters(source('resumed'), inherited)).toMatchObject({ userMessages: 17, totalTokens: 45000 })
})

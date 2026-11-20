// Тянет logging → потоковый прокси → @peculiar/x509 → tsyringe (нужен reflect).
import 'reflect-metadata'

import { mkdtempSync, rmSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { afterAll, beforeAll, beforeEach, describe, expect, it, vi } from 'vitest'

/**
 * Dashboard "clear all": the schema dropped the legacy `counters` table, and
 * the old clear deleted `requests` first, then failed on `counters` (HTTP 500)
 * before the OTel tables were touched. Real DuckDB in a temp data directory.
 */

let dir = ''
let lc: typeof import('./lifecycle')
let crud: typeof import('./crud')

beforeAll(async () => {
  dir = mkdtempSync(join(tmpdir(), 'clear-all-db-'))
  vi.spyOn(process, 'cwd').mockReturnValue(dir)
  vi.resetModules()
  lc = await import('./lifecycle')
  crud = await import('./crud')
  await lc.initStatsDb()
})

afterAll(async () => {
  await lc.closeDb()
  vi.restoreAllMocks()
  rmSync(dir, { recursive: true, force: true })
})

async function count(table: string): Promise<number> {
  const db = await lc.getDb()
  const row = (await db.runAndReadAll(`SELECT COUNT(*) AS n FROM ${table}`)).getRowObjects()[0] as { n: unknown }
  return Number(row.n)
}

async function seed(): Promise<void> {
  const db = await lc.getDb()
  const now = new Date().toISOString()
  await db.run(
    `INSERT INTO requests (id, timestamp, endpoint, plugin_id) VALUES ('r-' || nextval('seq_otel_events_id'), ?, '/v1/messages', 'p')`,
    [now],
  )
  await db.run(`INSERT INTO otel_metrics (metric_name, value, timestamp) VALUES ('m', 1, ?)`, [now])
  await db.run(`INSERT INTO otel_events (event_name, data, timestamp) VALUES ('e', '{}', ?)`, [now])
}

describe('dashboard clear all', () => {
  beforeEach(async () => {
    await seed()
  })

  it('on the current schema (no counters table) empties requests and OTel without an error', async () => {
    expect(await count('requests')).toBeGreaterThan(0)
    await expect(crud.clearAll()).resolves.toBeUndefined()
    for (const table of crud.HISTORY_TABLES) expect(await count(table), table).toBe(0)
  })

  it('leaves tables outside the request history alone, even a legacy one still present', async () => {
    const db = await lc.getDb()
    await db.run(`CREATE TABLE IF NOT EXISTS counters (key TEXT PRIMARY KEY, value TEXT)`)
    await db.run(`INSERT INTO counters VALUES ('legacy', '1')`)
    await db.run(`INSERT INTO settings (key, value) VALUES ('clear-all-probe', 'kept') ON CONFLICT DO NOTHING`)
    await crud.clearAll()
    expect(await count('counters')).toBe(1)
    expect(await count(`settings WHERE key = 'clear-all-probe'`)).toBe(1)
    for (const table of crud.HISTORY_TABLES) expect(await count(table), table).toBe(0)
    await db.run('DROP TABLE counters')
  })

  it('a failure in any table rolls the whole clear back: nothing is partially deleted', async () => {
    const db = await lc.getDb()
    // A row that references an OTel event makes its DELETE fail mid-clear.
    await db.run(`CREATE TABLE clear_all_guard (event_id BIGINT REFERENCES otel_events(id))`)
    await db.run(`INSERT INTO clear_all_guard SELECT id FROM otel_events LIMIT 1`)
    const before = {
      requests: await count('requests'),
      metrics: await count('otel_metrics'),
      events: await count('otel_events'),
    }
    await expect(crud.clearAll()).rejects.toThrow()
    expect({
      requests: await count('requests'),
      metrics: await count('otel_metrics'),
      events: await count('otel_events'),
    }).toEqual(before)
    await db.run('DROP TABLE clear_all_guard')
    await crud.clearAll()
    for (const table of crud.HISTORY_TABLES) expect(await count(table), table).toBe(0)
  })
})

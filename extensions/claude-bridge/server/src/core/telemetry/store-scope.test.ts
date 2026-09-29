// Тянет за собой потоковый прокси → @peculiar/x509 → tsyringe, которому нужен
// полифил reflect до декораторов. Тот же импорт, что и у точки входа.
import 'reflect-metadata'

import { mkdtempSync, rmSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

import { DuckDBInstance, type DuckDBConnection } from '@duckdb/node-api'
import { afterAll, beforeAll, beforeEach, describe, expect, it, vi } from 'vitest'

/**
 * INC-2026-0016: область удаления обязана описывать то, что действительно
 * удаляется.
 *
 * База ИЗОЛИРОВАННАЯ — отдельный файл DuckDB во временном каталоге; подменён
 * только источник соединения, сами запросы идут настоящие. Боевые данные
 * карточка трогать запрещает.
 */

const holder = vi.hoisted(() => ({ conn: null as unknown as DuckDBConnection }))
vi.mock('../stats/database/lifecycle', () => ({ getDb: () => Promise.resolve(holder.conn) }))

let dir = ''

beforeAll(async () => {
  dir = mkdtempSync(join(tmpdir(), 'inc-0016-'))
  const instance = await DuckDBInstance.create(join(dir, 'test.duckdb'))
  holder.conn = await instance.connect()
  await holder.conn.run(`
    CREATE TABLE otel_events (
      id BIGINT, session_id TEXT, user_name TEXT, event_name TEXT, data TEXT, timestamp TEXT
    )`)
  await holder.conn.run(`
    CREATE TABLE otel_metrics (
      id BIGINT, session_id TEXT, user_name TEXT, metric_name TEXT, value DOUBLE,
      model TEXT, attributes TEXT, timestamp TEXT
    )`)
})

afterAll(() => {
  rmSync(dir, { recursive: true, force: true })
})

beforeEach(async () => {
  await holder.conn.run('DELETE FROM otel_events')
  await holder.conn.run('DELETE FROM otel_metrics')
  // Два пользователя, успехи и ошибки, события и метрики — набор из карточки.
  for (const [id, user, data, ts] of [
    [1, 'алиса', '{"model":"opus"}', '2026-09-01T00:00:00Z'],
    [2, 'алиса', '{"error":"boom"}', '2026-09-02T00:00:00Z'],
    [3, 'борис', '{"model":"opus"}', '2026-09-03T00:00:00Z'],
    [4, 'борис', '{"error":"boom"}', '2026-09-04T00:00:00Z'],
  ] as [number, string, string, string][]) {
    await holder.conn.run(
      'INSERT INTO otel_events (id, session_id, user_name, event_name, data, timestamp) VALUES (?, ?, ?, ?, ?, ?)',
      [id, 's', user, 'request', data, ts],
    )
  }
  for (const [id, user] of [
    [1, 'алиса'],
    [2, 'борис'],
  ] as [number, string][]) {
    await holder.conn.run(
      'INSERT INTO otel_metrics (id, session_id, user_name, metric_name, value, model, attributes, timestamp) VALUES (?, ?, ?, ?, ?, ?, ?, ?)',
      [id, 's', user, 'tokens', 1, 'opus', '{}', '2026-09-01T00:00:00Z'],
    )
  }
})

async function countOf(table: string, user: string): Promise<number> {
  const row = (
    await holder.conn.runAndReadAll(`SELECT COUNT(*) AS n FROM ${table} WHERE user_name = ?`, [user])
  ).getRowObjects()[0] as Record<string, unknown> | undefined
  return Number(row?.n ?? 0)
}

async function store(): Promise<typeof import('./store')> {
  vi.resetModules()
  await import('reflect-metadata')
  return import('./store')
}

describe('INC-2026-0016: удаление по пользователю не трогает чужие записи', () => {
  it('чужие события и метрики остаются на месте', async () => {
    const s = await store()

    const removed = await s.clearOtelDataForUser('алиса')

    expect(removed).toEqual({ events: 2, metrics: 1 })
    expect(await countOf('otel_events', 'алиса')).toBe(0)
    expect(await countOf('otel_metrics', 'алиса')).toBe(0)
    expect(await countOf('otel_events', 'борис')).toBe(2)
    expect(await countOf('otel_metrics', 'борис')).toBe(1)
  })

  it('счёт снимается ДО удаления и описывает удалённое', async () => {
    // Маршрут обязан сообщить число, а не «ok»; счёт после удаления всегда ноль.
    const s = await store()

    expect(await s.clearOtelDataForUser('борис')).toEqual({ events: 2, metrics: 1 })
    expect(await s.clearOtelDataForUser('борис')).toEqual({ events: 0, metrics: 0 })
  })
})

describe('INC-2026-0016: страницы и удаление по идентификатору', () => {
  it('смещение даёт НЕПЕРЕСЕКАЮЩИЕСЯ страницы', async () => {
    const s = await store()

    const first = await s.queryEvents({ limit: 2, offset: 0 })
    const second = await s.queryEvents({ limit: 2, offset: 2 })

    expect(first.map((e) => e.id)).toEqual([4, 3])
    expect(second.map((e) => e.id)).toEqual([2, 1])
  })

  it('сужение по пользователю отдаёт только его события', async () => {
    const s = await store()

    const rows = await s.queryEvents({ limit: 10, userName: 'алиса' })

    expect(rows.map((e) => e.userName)).toEqual(['алиса', 'алиса'])
  })

  it('удаление по идентификатору убирает РОВНО одну запись того же набора', async () => {
    // Раньше удаление ходило в устаревшую таблицу `requests`: GET отвечал 200,
    // DELETE — 404, и один идентификатор обозначал разные записи.
    const s = await store()

    expect(await s.deleteEventById(2)).toBe(true)
    expect(await s.deleteEventById(2)).toBe(false)
    expect(await countOf('otel_events', 'алиса')).toBe(1)
    expect(await countOf('otel_events', 'борис')).toBe(2)
  })
})

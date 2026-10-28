// Тянет logging → потоковый прокси → @peculiar/x509 → tsyringe (нужен reflect).
import 'reflect-metadata'

import { mkdtempSync, rmSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { afterAll, beforeAll, describe, expect, it, vi } from 'vitest'

/**
 * INC-2026-0078: CHECKPOINT / retention / compaction run only in an exclusive
 * window — after the write lock, with admission closed and every in-flight
 * call drained — and connection handles survive the compaction reopen.
 * Real DuckDB in an isolated temp data directory.
 */

let dir = ''
type Lifecycle = typeof import('./lifecycle')
type Maintenance = typeof import('./maintenance')
type WriteLock = typeof import('./write-lock')
let lc: Lifecycle
let maint: Maintenance
let lock: WriteLock

beforeAll(async () => {
  dir = mkdtempSync(join(tmpdir(), 'inc-0078-db-'))
  vi.spyOn(process, 'cwd').mockReturnValue(dir)
  vi.resetModules()
  lc = await import('./lifecycle')
  maint = await import('./maintenance')
  lock = await import('./write-lock')
  await lc.initStatsDb()
  const db = await lc.getDb()
  await db.run(`CREATE TABLE IF NOT EXISTS t_probe (id INTEGER PRIMARY KEY, v TEXT)`)
})

afterAll(async () => {
  await lc.closeDb()
  vi.restoreAllMocks()
  rmSync(dir, { recursive: true, force: true })
})

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms))

describe('exclusive DB maintenance window', () => {
  it('waits for a call already in flight before entering', async () => {
    const h = await lc.getDbByIndex(2)
    const order: string[] = []
    const slow = h.runAndReadAll(`SELECT count(*) FROM range(150000000)`).then(() => order.push('query'))
    await sleep(5)
    expect(lc.dbCallsInFlight()).toBeGreaterThan(0)
    await lc.withExclusiveDb(async () => {
      order.push('window')
    })
    await slow
    expect(order).toEqual(['query', 'window'])
  })

  it('parks calls started during the window until it ends', async () => {
    const h = await lc.getDbByIndex(5)
    const order: string[] = []
    let parked: Promise<unknown> | null = null
    await lc.withExclusiveDb(async (raw) => {
      parked = h.run(`INSERT INTO t_probe VALUES (1, 'a')`).then(() => order.push('call'))
      await sleep(50)
      order.push('window-end')
      await raw.run(`SELECT 1`)
    })
    await parked
    expect(order).toEqual(['window-end', 'call'])
  })

  it('never checkpoints inside an open write transaction of this process', async () => {
    const h = await lc.getDbByIndex(1)
    // A sweeper-shaped multi-statement transaction, mid-flight.
    const tx = lock.withStatsWrite(async () => {
      await h.run('BEGIN')
      await h.run(`INSERT INTO t_probe VALUES (10, 'x')`)
      await sleep(100)
      await h.run(`INSERT INTO t_probe VALUES (11, 'y')`)
      await h.run('COMMIT')
    })
    await sleep(10)
    await expect(lc.withExclusiveDb((raw) => raw.run('CHECKPOINT'))).resolves.toBeDefined()
    await tx
    const n = (await h.runAndReadAll(`SELECT count(*) AS n FROM t_probe WHERE id IN (10, 11)`)).getRowObjects()[0] as {
      n: bigint
    }
    expect(Number(n.n)).toBe(2)
  })

  it('keeps handles held across a compaction usable and the data intact', async () => {
    const held = await lc.getDbByIndex(3)
    await held.run(`INSERT INTO t_probe VALUES (20, 'before')`)
    const r = await maint.compactDatabase()
    expect(r.ok).toBe(true)
    await held.run(`INSERT INTO t_probe VALUES (21, 'after')`)
    const rows = (await held.runAndReadAll(`SELECT id FROM t_probe WHERE id >= 20 ORDER BY id`)).getRowObjects()
    expect(rows.map((x) => Number((x as { id: number }).id))).toEqual([20, 21])
  })

  it('abandons maintenance instead of forcing it when a call never drains', async () => {
    const h = await lc.getDbByIndex(4)
    const slow = h.runAndReadAll(`SELECT count(*) FROM range(400000000)`)
    await sleep(5)
    await expect(lc.withExclusiveDb(async () => 'ran', { drainTimeoutMs: 20 })).rejects.toThrow(/still running/)
    await slow
  })
})

describe('DuckDB resource budget', () => {
  it('derives a bounded memory limit and thread count, with overrides', async () => {
    const { duckDbBudget } = await import('./memory-budget')
    expect(duckDbBudget({}, 8 * 1024 ** 3, 8)).toEqual({ memory_limit: '1228MB', threads: '4' })
    expect(duckDbBudget({}, 1024 ** 3, 1)).toEqual({ memory_limit: '256MB', threads: '1' })
    expect(duckDbBudget({}, 64 * 1024 ** 3, 32)).toEqual({ memory_limit: '2048MB', threads: '4' })
    expect(duckDbBudget({ OCB_DUCKDB_MEMORY_LIMIT: '3GB', OCB_DUCKDB_THREADS: '2' }, 1024 ** 3, 8)).toEqual({
      memory_limit: '3GB',
      threads: '2',
    })
  })
})

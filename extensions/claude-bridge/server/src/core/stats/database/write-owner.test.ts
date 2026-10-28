// Тянет logging → потоковый прокси → @peculiar/x509 → tsyringe (нужен reflect).
import 'reflect-metadata'

import { mkdtempSync, rmSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { afterAll, afterEach, beforeAll, describe, expect, it, vi } from 'vitest'

/**
 * INC-2026-0078 F06: one owner of DuckDB writes — bounded admission, queued
 * work that can time out or be cancelled, a stuck head interrupted (never
 * bypassed), and every write statement routed through the owner whoever
 * issues it. Real DuckDB in an isolated directory.
 */

let dir = ''
let lc: typeof import('./lifecycle')
let wl: typeof import('./write-lock')

beforeAll(async () => {
  dir = mkdtempSync(join(tmpdir(), 'inc-0078-owner-'))
  vi.spyOn(process, 'cwd').mockReturnValue(dir)
  vi.resetModules()
  lc = await import('./lifecycle')
  wl = await import('./write-lock')
  await lc.initStatsDb()
  await (await lc.getDb()).run(`CREATE TABLE IF NOT EXISTS t_owner (id INTEGER PRIMARY KEY, v TEXT)`)
})

afterEach(() => {
  wl.configureStatsWriteQueue({ maxPending: 512, waitTimeoutMs: 120_000, headTimeoutMs: 120_000 })
})

afterAll(async () => {
  await lc.closeDb()
  vi.restoreAllMocks()
  rmSync(dir, { recursive: true, force: true })
})

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms))
const gate = () => {
  let open!: () => void
  const p = new Promise<void>((r) => (open = r))
  return { p, open }
}

describe('stats write owner queue', () => {
  it('refuses new work once the bounded queue is full', async () => {
    wl.configureStatsWriteQueue({ maxPending: 3 })
    const g = gate()
    const running = wl.withStatsWrite(() => g.p)
    const a = wl.withStatsWrite(async () => 1)
    const b = wl.withStatsWrite(async () => 2)
    await expect(wl.withStatsWrite(async () => 3)).rejects.toBeInstanceOf(wl.WriteQueueFullError)
    g.open()
    await expect(Promise.all([running, a, b])).resolves.toEqual([undefined, 1, 2])
  })

  it('drops queued work that waited too long or was cancelled — it never runs', async () => {
    const g = gate()
    const running = wl.withStatsWrite(() => g.p)
    let ran = 0
    const slow = wl.withStatsWrite(async () => ran++, { waitTimeoutMs: 30 })
    const ac = new AbortController()
    const cancelled = wl.withStatsWrite(async () => ran++, { signal: ac.signal })
    const cancelledResult = expect(cancelled).rejects.toThrow('client gone')
    ac.abort(new Error('client gone'))
    await expect(slow).rejects.toBeInstanceOf(wl.WriteQueueTimeoutError)
    await cancelledResult
    g.open()
    await running
    await sleep(10)
    expect(ran).toBe(0)
    expect(wl.statsWriteDepth()).toBe(0)
  })

  it('interrupts a stuck head query and only then gives the next writer its turn', async () => {
    wl.configureStatsWriteQueue({ headTimeoutMs: 200 })
    const db = await lc.getDbByIndex(1)
    const order: string[] = []
    const stuck = wl
      .withStatsWrite(async () => {
        // A long write statement on a tracked connection.
        await db.run(`CREATE TEMP TABLE big AS SELECT count(*) AS n FROM range(20000000000)`)
      })
      .then(
        () => order.push('stuck-finished'),
        () => order.push('stuck-interrupted'),
      )
    const next = wl.withStatsWrite(async () => order.push('next'))
    await Promise.all([stuck, next])
    expect(order).toEqual(['stuck-interrupted', 'next'])
  }, 30_000)

  it('runs nested owner calls directly instead of deadlocking', async () => {
    await expect(wl.withStatsWrite(() => wl.withStatsWrite(async () => 'inner'))).resolves.toBe('inner')
  })

  it('routes write statements from any caller through the single owner', async () => {
    const h1 = await lc.getDbByIndex(2)
    const h2 = await lc.getDbByIndex(3)
    let maxDepth = 0
    const probe = setInterval(() => (maxDepth = Math.max(maxDepth, wl.statsWriteDepth())), 0)
    await Promise.all(
      Array.from({ length: 40 }, (_, i) => (i % 2 ? h1 : h2).run(`INSERT INTO t_owner VALUES (?, 'x')`, [100 + i])),
    )
    clearInterval(probe)
    const n = (await h1.runAndReadAll(`SELECT count(*) AS n FROM t_owner WHERE id >= 100`)).getRowObjects()[0] as {
      n: bigint
    }
    expect(Number(n.n)).toBe(40)
    expect(maxDepth).toBeGreaterThan(1) // they queued behind one owner
    // Reads do not take a turn.
    expect(lc.isWriteSql('SELECT 1')).toBe(false)
    expect(lc.isWriteSql('  /* c */ insert into x values (1)')).toBe(true)
  })
})

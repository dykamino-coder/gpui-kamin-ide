// Size bench for the admin user-stats page (manual, not part of `npm test`).
//
//   npx tsx src/core/stats/database/rollup-bench.ts gen   <dir> <sessions> [eventsPerSession]
//   npx tsx src/core/stats/database/rollup-bench.ts bench <dir> [runs]
//
// `gen` builds <dir>/data/proxy.duckdb with the production schema and a
// synthetic corpus; `bench` opens it through the production lifecycle (same
// pool and DuckDB budget) and replays the page's request pattern.

import 'reflect-metadata'
import fs from 'fs'
import path from 'path'
import { performance } from 'perf_hooks'

async function gen(dir: string, sessions: number, eps: number): Promise<void> {
  const { DuckDBInstance } = await import('@duckdb/node-api')
  const { initSchema } = await import('./schema')
  const { generateSyntheticStats } = await import('./synthetic-stats')
  fs.mkdirSync(path.join(dir, 'data'), { recursive: true })
  const file = path.join(dir, 'data', 'proxy.duckdb')
  const inst = await DuckDBInstance.create(file, { memory_limit: '16GB', threads: '8' })
  const db = await inst.connect()
  await initSchema(db)
  // Secondary ART indexes are bulk-built after the load (much faster than
  // maintaining them row by row); initSchema below recreates them.
  const idx = (
    await db.runAndReadAll(`SELECT index_name FROM duckdb_indexes() WHERE table_name = 'jsonl_events'`)
  ).getRowObjects() as Array<{ index_name: string }>
  for (const { index_name } of idx) await db.run(`DROP INDEX IF EXISTS ${index_name}`)
  const t0 = performance.now()
  await generateSyntheticStats(db, {
    users: 34,
    sessions,
    eventsPerSession: eps,
    days: 240,
    endIso: new Date().toISOString().slice(0, 19),
    chunkSessions: 4000,
    onProgress: (n) =>
      process.stdout.write(`\r${n}/${sessions} sessions ${((performance.now() - t0) / 1000).toFixed(0)}s`),
  })
  await initSchema(db)
  await db.run('CHECKPOINT')
  const n = (await db.runAndReadAll('SELECT COUNT(*) AS n FROM jsonl_events')).getRowObjects()[0]!.n
  db.disconnectSync()
  inst.closeSync()
  const bytes = fs.statSync(file).size
  console.log(
    `\nrows=${n} size=${(bytes / 2 ** 30).toFixed(2)} GiB time=${((performance.now() - t0) / 1000).toFixed(0)}s`,
  )
}

function pct(xs: number[], p: number): number {
  const s = [...xs].sort((a, b) => a - b)
  return s.length ? s[Math.min(s.length - 1, Math.floor((p / 100) * s.length))]! : NaN
}

async function bench(dir: string, runs: number): Promise<void> {
  process.chdir(dir)
  // The endpoints kick a catch-up sweep of ~/.claude/projects: point it at the
  // empty bench dir so no real transcript is read into the synthetic DB.
  process.env.USERPROFILE = dir
  process.env.HOME = dir
  const { Hono } = await import('hono')
  const { DuckDBConnection } = await import('@duckdb/node-api')
  // Record every read statement the endpoints issue (text, params, time) so
  // each distinct one can be EXPLAIN ANALYZE'd afterwards.
  const seen = new Map<string, { params: unknown[]; ms: number[]; timeouts: number }>()
  const proto = DuckDBConnection.prototype as any
  const orig = proto.runAndReadAll
  const active = new Set<{ interrupt(): void }>()
  proto.runAndReadAll = async function (sql: string, params?: unknown[]) {
    const s = performance.now()
    active.add(this)
    let ok = false
    try {
      const r = await orig.call(this, sql, params)
      ok = true
      return r
    } finally {
      active.delete(this)
      if (/^\s*(WITH|SELECT)/i.test(sql) && !/stats_rollup_dirty|FROM settings/.test(sql)) {
        const key = sql.replace(/\s+/g, ' ').trim()
        const e = seen.get(key) ?? { params: params ?? [], ms: [], timeouts: 0 }
        if (ok) e.ms.push(performance.now() - s)
        else e.timeouts++
        seen.set(key, e)
      }
    }
  }
  const lc = await import('./lifecycle')
  const t0 = performance.now()
  await lc.initStatsDb()
  console.log(`open ${(performance.now() - t0).toFixed(0)} ms`)
  if (process.env.BENCH_BUILD) {
    const rollup = await import('./rollup')
    const b0 = performance.now()
    // An already-built rollup is reset so the build is timed from scratch.
    // BENCH_KEEP=1 keeps a built rollup (drain timings only).
    if (rollup.rollupReady() && !process.env.BENCH_KEEP) await rollup.resetStatsRollup(await lc.getDb())
    await rollup.startRollupBuild()
    console.log(`rollup build ${(performance.now() - b0).toFixed(0)} ms`)
    const db = await lc.getDb()
    const n = (await db.runAndReadAll('SELECT COUNT(*) AS n FROM stats_rollup')).getRowObjects()[0]!.n
    console.log(`rollup rows ${n}`)
    // Incremental refresh cost: one busy sweep marks a few hundred sessions.
    for (const k of [1, 32, 256]) {
      await db.run(
        `INSERT INTO stats_rollup_dirty SELECT session_id FROM session_tokens USING SAMPLE ${k} ROWS ON CONFLICT DO NOTHING`,
      )
      const d0 = performance.now()
      await rollup.drainRollup()
      console.log(`drain ${k} sessions ${(performance.now() - d0).toFixed(0)} ms`)
    }
  }
  // EXPLAIN ANALYZE of every distinct statement seen; one that cannot finish
  // within the cap gets its plain EXPLAIN plan and the timeout noted.
  const writeExplain = async (): Promise<void> => {
    const out = process.env.BENCH_EXPLAIN
    if (!out) return
    proto.runAndReadAll = orig
    const db = await lc.getDb()
    const cap = Number(process.env.BENCH_CAP_MS ?? 120_000)
    const parts: string[] = []
    for (const [sql, e] of seen) {
      const head = `-- n=${e.ms.length} p50=${pct(e.ms, 50).toFixed(1)} ms p95=${pct(e.ms, 95).toFixed(1)} ms timeouts=${e.timeouts}\n-- params ${JSON.stringify(e.params)}\n${sql}\n`
      const t = setTimeout(() => db.interrupt(), cap)
      try {
        const plan = (await db.runAndReadAll(`EXPLAIN ANALYZE ${sql}`, e.params as never[])).getRows()
        parts.push(head + plan.map((r) => r.map(String).join('\n')).join('\n'))
      } catch (err) {
        // Not explainable (e.g. a statement that is not a query) or over the cap.
        const plan = await db
          .runAndReadAll(`EXPLAIN ${sql}`, e.params as never[])
          .then((r) =>
            r
              .getRows()
              .map((x) => x.map(String).join('\n'))
              .join('\n'),
          )
          .catch(() => `(no plan: ${String(err).slice(0, 120)})`)
        parts.push(`${head}-- EXPLAIN ANALYZE failed or exceeded ${cap / 1000} s; plain plan:\n${plan}`)
      } finally {
        clearTimeout(t)
      }
    }
    fs.writeFileSync(out, parts.join('\n\n'))
    console.log(`explain: ${seen.size} distinct statements -> ${out}`)
  }
  const app = new Hono()
  ;(await import('../../dashboard/controllers/users')).registerUserRoutes(app)
  ;(await import('../../dashboard/controllers/stats')).registerStatsRoutes(app)
  if (process.env.BENCH_PAGE === 'endpoints') {
    await endpoints(app, runs, active)
    await writeExplain()
    await lc.closeDb()
    return
  }
  const users = (await (await app.request('/api/dashboard/users')).json()) as Array<{ userName: string }>
  const page: number[] = []
  const perEndpoint = new Map<string, number[]>()
  const time = async (name: string, url: string): Promise<void> => {
    const s = performance.now()
    const r = await app.request(url)
    await r.arrayBuffer()
    const ms = performance.now() - s
    if (!perEndpoint.has(name)) perEndpoint.set(name, [])
    perEndpoint.get(name)!.push(ms)
  }
  const pageUrls =
    (process.env.BENCH_PAGE ?? 'legacy') === 'legacy'
      ? [
          ['overview(all tokens)', '/api/dashboard/stats/overview?range=all'] as const,
          ...users.map(
            (u) =>
              [
                'overview(user)',
                `/api/dashboard/stats/overview?range=all&token=${encodeURIComponent(u.userName)}`,
              ] as const,
          ),
        ]
      : [['users-overview', '/api/dashboard/stats/users-overview?range=all&tz=-180'] as const]
  let rss = 0
  const rssTimer = setInterval(() => (rss = Math.max(rss, process.memoryUsage().rss)), 20)
  for (let i = 0; i < runs; i++) {
    // The overview cache would serve repeats; benchmark the cold path.
    const { clearOverviewCache } = (await import('../../dashboard/controllers/stats')) as any
    clearOverviewCache?.()
    const s = performance.now()
    await time('users', '/api/dashboard/users')
    // The browser opens ~6 HTTP/1.1 connections per origin.
    const queue = [...pageUrls]
    await Promise.all(
      Array.from({ length: 6 }, async () => {
        for (let next = queue.shift(); next; next = queue.shift()) await time(next[0], next[1])
      }),
    )
    page.push(performance.now() - s)
  }
  clearInterval(rssTimer)
  console.log(
    `users=${users.length} page p50=${pct(page, 50).toFixed(0)} p95=${pct(page, 95).toFixed(0)} ms peakRSS=${(rss / 2 ** 20).toFixed(0)} MiB`,
  )
  for (const [k, v] of perEndpoint)
    console.log(`  ${k}: n=${v.length} p50=${pct(v, 50).toFixed(0)} p95=${pct(v, 95).toFixed(0)} ms`)
  await writeExplain()
  await lc.closeDb()
}

// Per-endpoint latency like production: each request alone, 120 s cap (the
// running statement is interrupted on timeout so the next one starts clean),
// first call cold, second call warm (the endpoint's own result cache).
async function endpoints(
  app: { request(url: string): Response | Promise<Response> },
  runs: number,
  active: Set<{ interrupt(): void }>,
): Promise<void> {
  const CAP = Number(process.env.BENCH_CAP_MS ?? 120_000)
  // Synthetic users are skewed by index: user00 heaviest, user33 lightest. (The
  // old /users endpoint itself can exceed the cap, so it is not used to pick.)
  const pick = [{ userName: 'user00' }, { userName: 'user10' }, { userName: 'user33' }]
  const urls: Array<[string, string]> = [
    ['users', '/api/dashboard/users'],
    ['overview all', '/api/dashboard/stats/overview?range=all'],
    ...pick.map(
      (u, i) =>
        [
          `overview user(${['max', 'median', 'min'][i]})`,
          `/api/dashboard/stats/overview?range=all&token=${encodeURIComponent(u!.userName)}`,
        ] as [string, string],
    ),
    ['overview user(max) 7d', `/api/dashboard/stats/overview?range=7d&token=${encodeURIComponent(pick[0]!.userName)}`],
    // The KaminIDE Stats panel's call (compact heatmap answer).
    [
      'overview user(max) hm',
      `/api/dashboard/stats/overview?range=all&agg=hm&tz=-180&token=${encodeURIComponent(pick[0]!.userName)}`,
    ],
    ['users-overview page', '/api/dashboard/stats/users-overview?range=all&tz=Europe/Moscow'],
    [
      'users-overview one 30d',
      `/api/dashboard/stats/users-overview?range=30d&tz=Europe/Moscow&subject=${encodeURIComponent(pick[0]!.userName)}`,
    ],
  ]
  const timed = async (url: string): Promise<string> => {
    const s = performance.now()
    let timer: ReturnType<typeof setTimeout> | undefined
    const res = await Promise.race([
      (async () => {
        const r = await app.request(url)
        await r.arrayBuffer()
        return r.status === 404 ? 'n/a (404)' : `${(performance.now() - s).toFixed(0)} ms`
      })(),
      new Promise<string>((resolve) => {
        timer = setTimeout(() => {
          for (const c of active) c.interrupt()
          resolve(`>${CAP / 1000} s (timeout)`)
        }, CAP)
      }),
    ]).catch((e) => `error ${String(e).slice(0, 60)}`)
    clearTimeout(timer)
    // Let an interrupted statement unwind before the next request.
    await new Promise((r) => setTimeout(r, 200))
    return res
  }
  const { clearOverviewCache } = (await import('../../dashboard/controllers/stats')) as any
  const touch = ((await import('./rollup')) as any).touchRollupRevision as (() => void) | undefined
  for (const [name, url] of urls) {
    const cells: string[] = []
    for (let i = 0; i < runs; i++) {
      if (clearOverviewCache) clearOverviewCache()
      else if (i > 0) await new Promise((r) => setTimeout(r, 31_000)) // old 30 s result cache
      const cold = await timed(url)
      // A timed-out cold call cached nothing; repeating it only burns the cap.
      if (cold.includes('timeout')) {
        cells.push(`${cold} (repeat skipped)`)
        break
      }
      const warm = await timed(url)
      // Live-server case: the data changed since the cached answer (every
      // drained sweep). Served stale-while-revalidate where supported.
      let changed = ''
      if (touch) {
        touch()
        changed = ` / ${await timed(url)}`
        await new Promise((r) => setTimeout(r, 5_000)) // let the background refresh finish
      }
      cells.push(`${cold} / ${warm}${changed}`)
    }
    console.log(`${name.padEnd(24)} cold / warm${touch ? ' / changed' : ''}: ${cells.join(' | ')}`)
  }
  console.log(`peakRSS ${(process.resourceUsage().maxRSS / 1024).toFixed(0)} MiB`)
}

const [mode, dir, a, b] = process.argv.slice(2)
if (mode === 'gen') await gen(dir!, Number(a), Number(b ?? 60))
else if (mode === 'bench') await bench(dir!, Number(a ?? 3))
else console.error('usage: gen <dir> <sessions> [eventsPerSession] | bench <dir> [runs]')

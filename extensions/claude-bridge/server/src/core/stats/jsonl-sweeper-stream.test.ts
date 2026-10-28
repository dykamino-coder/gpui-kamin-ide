// Тянет logging → потоковый прокси → @peculiar/x509 → tsyringe (нужен reflect).
import 'reflect-metadata'

import { appendFileSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { afterAll, beforeAll, beforeEach, describe, expect, it, vi } from 'vitest'

/**
 * INC-2026-0078: the sweeper streams the unread range, bounds every write
 * transaction (tool_result rows included), and its byte cursor never passes a
 * record that is still being written. Real DuckDB in an isolated directory.
 */

const batches = vi.hoisted(() => ({ sizes: [] as number[] }))
vi.mock('./jsonl-batch', async (importOriginal) => {
  const actual = await importOriginal<typeof import('./jsonl-batch')>()
  return {
    ...actual,
    dedupeByUuid: (rows: unknown[][]) => {
      batches.sizes.push(rows.length)
      return actual.dedupeByUuid(rows)
    },
  }
})

let dir = ''
let sweeper: typeof import('./jsonl-sweeper')
let lc: typeof import('./database/lifecycle')

beforeAll(async () => {
  dir = mkdtempSync(join(tmpdir(), 'inc-0078-sweep-'))
  vi.spyOn(process, 'cwd').mockReturnValue(dir)
  vi.resetModules()
  lc = await import('./database/lifecycle')
  sweeper = await import('./jsonl-sweeper')
  await lc.initStatsDb()
})

afterAll(async () => {
  await lc.closeDb()
  vi.restoreAllMocks()
  rmSync(dir, { recursive: true, force: true })
})

beforeEach(() => {
  batches.sizes.length = 0
})

let n = 0
const ts = () => new Date(Date.UTC(2026, 0, 1, 0, 0, n)).toISOString()
const toolResult = (sid: string) =>
  JSON.stringify({
    type: 'user',
    uuid: `tr-${sid}-${++n}`,
    timestamp: ts(),
    message: { role: 'user', content: [{ type: 'tool_result', tool_use_id: `t${n}`, content: 'ok' }] },
  }) + '\n'
const userText = (sid: string, text: string) =>
  JSON.stringify({ type: 'user', uuid: `u-${sid}-${++n}`, timestamp: ts(), message: { role: 'user', content: text } }) +
  '\n'

function transcript(name: string, content: string): string {
  const d = join(dir, 'projects', 'slug-x')
  mkdirSync(d, { recursive: true })
  const p = join(d, `${name}.jsonl`)
  writeFileSync(p, content)
  return p
}

async function rows(sessionId: string): Promise<number> {
  const db = await lc.getDb()
  const r = (
    await db.runAndReadAll(`SELECT count(*) AS n FROM jsonl_events WHERE session_id = ?`, [sessionId])
  ).getRowObjects()[0] as {
    n: bigint
  }
  return Number(r.n)
}

describe('jsonl sweeper — streamed, bounded, durable', { timeout: 120_000 }, () => {
  it('never puts more than the batch limit of tool_result rows in one transaction', async () => {
    let body = ''
    for (let i = 0; i < 1200; i++) body += toolResult('s1')
    const p = transcript('s1', body)
    await sweeper.sweepOneByPath(p)
    expect(await rows('s1')).toBe(1200)
    expect(Math.max(...batches.sizes)).toBeLessThanOrEqual(500)
  })

  it('keeps a record written in two halves and indexes it once complete', async () => {
    const line = userText('s2', 'hello')
    const half = Math.floor(line.length / 2)
    const p = transcript('s2', userText('s2', 'first') + line.slice(0, half))
    await sweeper.sweepOneByPath(p)
    expect(await rows('s2')).toBe(1)
    appendFileSync(p, line.slice(half))
    await sweeper.sweepOneByPath(p)
    expect(await rows('s2')).toBe(2)
  })

  it('reads multi-byte text across read-chunk boundaries intact', async () => {
    let body = ''
    for (let i = 0; i < 3000; i++) body += userText('s3', `привет 😀 ${'é'.repeat(300)} ${i}`)
    const p = transcript('s3', body)
    await sweeper.sweepOneByPath(p)
    expect(await rows('s3')).toBe(3000)
  })

  it('does not re-update rows whose attribution is still unknown (F10)', async () => {
    const p = transcript('s4', userText('s4', 'one') + userText('s4', 'two'))
    await sweeper.sweepOneByPath(p)
    const db = await lc.getDb()
    const apply = (token: string | null, user: string | null) =>
      db.run(sweeper.SQL_APPLY_TOKEN_AND_SUBAGENT, [token, user, 0, null, 's4'])
    // Unknown token/user (NULL or empty): nothing to change, nothing selected.
    expect((await apply(null, null)).rowsChanged).toBe(0)
    expect((await apply('', '')).rowsChanged).toBe(0)
    // A real attribution fills the rows once…
    expect((await apply('tok-1', 'alice')).rowsChanged).toBe(2)
    // …and repeating it is a no-op.
    expect((await apply('tok-1', 'alice')).rowsChanged).toBe(0)
  })
})

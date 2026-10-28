// Тянет logging → потоковый прокси → @peculiar/x509 → tsyringe (нужен reflect).
import 'reflect-metadata'

import { mkdtempSync, readdirSync, rmSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { afterAll, beforeAll, describe, expect, it, vi } from 'vitest'

/**
 * INC-2026-0078: replay used to re-insert one request row per historical user
 * message on every resume/reattach (new id, the resuming PTY session and user).
 * The one-time cleanup keeps the original row of each (timestamp, message) and
 * nothing else. Real DuckDB in an isolated directory.
 */

let dir = ''
let lc: typeof import('./lifecycle')
let crud: typeof import('./crud')
let dd: typeof import('./requests-dedupe')

beforeAll(async () => {
  dir = mkdtempSync(join(tmpdir(), 'inc-0078-dedupe-'))
  vi.spyOn(process, 'cwd').mockReturnValue(dir)
  vi.resetModules()
  lc = await import('./lifecycle')
  crud = await import('./crud')
  dd = await import('./requests-dedupe')
  await lc.initStatsDb()
})

afterAll(async () => {
  await lc.closeDb()
  vi.restoreAllMocks()
  rmSync(dir, { recursive: true, force: true })
})

let k = 0
async function input(user: string, session: string, ts: string, text: string): Promise<void> {
  await crud.insertRequest({
    id: `r-${++k}`,
    timestamp: new Date(ts),
    endpoint: 'pty',
    method: 'INPUT',
    model: '',
    pluginId: 'pty',
    userName: user,
    durationMs: 0,
    inputTokens: 0,
    outputTokens: 0,
    cacheReadTokens: 0,
    cacheWriteTokens: 0,
    toolsUsed: [],
    status: 'success',
    statusCode: 200,
    isUserRequest: true,
    userMessage: text,
    sessionKey: session,
  } as never)
}

async function rows(): Promise<
  Array<{ user_name: string; session_key: string; timestamp: string; user_message: string }>
> {
  const db = await lc.getDb()
  return (
    await db.runAndReadAll(
      `SELECT user_name, session_key, timestamp, user_message FROM requests WHERE endpoint = 'pty' ORDER BY timestamp, rowid`,
    )
  ).getRowObjects() as never
}

describe('one-time cleanup of replay-duplicated user inputs', () => {
  it('keeps originals and genuine repeats, drops replay copies, then is a no-op', async () => {
    // Originals, live, by alice in session s1. "again" is genuinely sent twice.
    await input('alice', 's1', '2026-10-01T10:00:00.000Z', 'hello')
    await input('alice', 's1', '2026-10-01T10:05:00.000Z', 'again')
    await input('alice', 's1', '2026-10-01T10:09:00.000Z', 'again')
    // Three resumes replayed the history: copies under new PTY sessions — one
    // of them by another user who resumed the conversation.
    for (const [user, session] of [
      ['alice', 's2'],
      ['bob', 's3'],
      ['alice', 's4'],
    ]) {
      await input(user!, session!, '2026-10-01T10:00:00.000Z', 'hello')
      await input(user!, session!, '2026-10-01T10:05:00.000Z', 'again')
      await input(user!, session!, '2026-10-01T10:09:00.000Z', 'again')
    }
    expect((await rows()).length).toBe(12)

    const r = await dd.dedupeReplayedRequests({ backup: true, free: async () => Number.MAX_SAFE_INTEGER })
    expect(r).toMatchObject({ status: 'done', deleted: 9 })
    expect(readdirSync(join(dir, 'data')).some((f) => f.startsWith('proxy.pre-dedupe-'))).toBe(true)
    const left = await rows()
    expect(left.map((x) => [x.user_name, x.session_key, x.user_message])).toEqual([
      ['alice', 's1', 'hello'],
      ['alice', 's1', 'again'],
      ['alice', 's1', 'again'],
    ])

    expect(await dd.dedupeReplayedRequests()).toMatchObject({ status: 'already-done', deleted: 0 })
  })

  it('skips (and retries later) when the disk cannot hold the backup', async () => {
    const db = await lc.getDb()
    await db.run(`DELETE FROM settings WHERE key = ?`, [dd.REQUESTS_DEDUPE_FLAG])
    const r = await dd.dedupeReplayedRequests({ free: async () => 0 })
    expect(r.status).toBe('skipped-no-space')
    const flag = (
      await db.runAndReadAll(`SELECT * FROM settings WHERE key = ?`, [dd.REQUESTS_DEDUPE_FLAG])
    ).getRowObjects()
    expect(flag).toEqual([])
  })
})

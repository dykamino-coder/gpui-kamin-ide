import 'reflect-metadata'
import { DuckDBInstance, type DuckDBConnection } from '@duckdb/node-api'
import { Hono } from 'hono'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { initSchema } from './schema'
import { initSessionOwners, readSessionOwners, writeSessionToken } from './session-owners'

const state = vi.hoisted(() => ({ db: undefined as unknown as DuckDBConnection }))
vi.mock('./lifecycle', () => ({ getDb: async () => state.db }))
vi.mock('../../pty/session-manager', () => ({ getAllSessions: () => [] }))

let instance: DuckDBInstance
let db: DuckDBConnection
beforeEach(async () => {
  instance = await DuckDBInstance.create(':memory:')
  db = await instance.connect()
  state.db = db
  await initSchema(db)
})
afterEach(() => {
  vi.restoreAllMocks()
  db.disconnectSync()
  instance.closeSync()
})

const insert = (id: string, user: string | null, date: string, deleted = 0) =>
  writeSessionToken(db, id, () =>
    db.run(
      `INSERT INTO session_tokens
    (session_id, token_id, user_name, created_at, deleted) VALUES (?, 'tok', ?, ?, ?)`,
      [id, user, date, deleted],
    ),
  )

async function checkOracle() {
  const diff = (
    await db.runAndReadAll(`WITH expected AS (
    SELECT user_name, COUNT(*) AS sessions, MIN(created_at) AS first_seen, MAX(created_at) AS last_seen
    FROM session_tokens WHERE deleted = 0 AND user_name IS NOT NULL AND user_name != '' GROUP BY user_name)
    SELECT * FROM ((SELECT * FROM session_owners EXCEPT ALL SELECT * FROM expected)
      UNION ALL (SELECT * FROM expected EXCEPT ALL SELECT * FROM session_owners))`)
  ).getRowObjects()
  expect(diff).toEqual([])
  const persisted = (
    await db.runAndReadAll('SELECT user_name, first_seen, last_seen FROM session_owners ORDER BY user_name')
  ).getRowObjects()
  expect([...readSessionOwners()].sort((a, b) => a.user_name.localeCompare(b.user_name))).toEqual(persisted)
}

describe('persistent session owner roster', () => {
  it('migrates legacy live owners once and reads the build roster without scanning sessions', async () => {
    await db.run(`INSERT INTO session_tokens (session_id, token_id, user_name, created_at, deleted) VALUES
      ('a', 'tok', 'alice', '2026-01-01', 0), ('b', 'tok', 'alice', '2026-02-01', 0),
      ('c', 'tok', 'bob', '2026-03-01', 0), ('d', 'tok', 'deleted', '2026-01-01', 1),
      ('e', 'tok', '', '2026-01-01', 0), ('f', 'tok', NULL, '2026-01-01', 0)`)
    await db.run("DELETE FROM settings WHERE key = 'session_owners_v1'")
    await initSessionOwners(db)
    await checkOracle()
    const reads = vi.spyOn(db, 'runAndReadAll')
    await initSessionOwners(db)
    expect(reads.mock.calls.some(([sql]) => /FROM session_tokens/i.test(sql))).toBe(false)
    reads.mockClear()
    const { registerUserRoutes } = await import('../../dashboard/controllers/users')
    const app = new Hono()
    registerUserRoutes(app)
    for (let i = 0; i < 3; i++) {
      const res = await app.request('/api/dashboard/users')
      expect(res.status).toBe(200)
      expect(res.headers.get('X-Stats-Building')).toBe('0')
      expect(await res.json()).toMatchObject([
        { userName: 'bob', firstSeen: '2026-03-01', lastSeen: '2026-03-01', totalInputs: 0 },
        { userName: 'alice', firstSeen: '2026-01-01', lastSeen: '2026-02-01', totalInputs: 0 },
      ])
    }
    expect(reads).not.toHaveBeenCalled()
    expect(reads.mock.calls.some(([sql]) => /FROM session_tokens/i.test(sql))).toBe(false)
    await insert('new', 'new-owner', '2026-04-01')
    expect(await (await app.request('/api/dashboard/users')).json()).toMatchObject([{ userName: 'new-owner' }, {}, {}])
  })

  it('tracks insert, duplicate discovery, owner healing, soft-delete, restore and date boundaries', async () => {
    await insert('a', 'alice', '2026-01-01')
    await insert('b', 'alice', '2026-02-01')
    await insert('c', 'alice', '2026-03-01')
    await insert('unknown', '', '2025-01-01')
    await insert('deleted', 'hidden', '2025-01-01', 1)
    await checkOracle()
    await writeSessionToken(db, 'a', () =>
      db.run(`INSERT INTO session_tokens
      (session_id, token_id, user_name, created_at) VALUES ('a', 'tok', 'wrong', '2025') ON CONFLICT DO NOTHING`),
    )
    await checkOracle()
    for (const [id, sql] of [
      ['unknown', "UPDATE session_tokens SET user_name = 'bob' WHERE session_id = 'unknown'"],
      ['b', "UPDATE session_tokens SET deleted = 1 WHERE session_id = 'b'"],
      ['a', "UPDATE session_tokens SET deleted = 1 WHERE session_id = 'a'"],
      ['c', "UPDATE session_tokens SET user_name = 'bob' WHERE session_id = 'c'"],
      ['b', "UPDATE session_tokens SET deleted = 0 WHERE session_id = 'b'"],
      ['unknown', "UPDATE session_tokens SET created_at = '2026-04-01' WHERE session_id = 'unknown'"],
      ['unknown', "DELETE FROM session_tokens WHERE session_id = 'unknown'"],
    ]) {
      await writeSessionToken(db, id!, () => db.run(sql!))
      await checkOracle()
    }
  })

  it('rolls back the session and roster together on a failed write', async () => {
    await insert('a', 'alice', '2026-01-01')
    await expect(
      writeSessionToken(db, 'a', async () => {
        await db.run("UPDATE session_tokens SET user_name = 'bob' WHERE session_id = 'a'")
        throw new Error('synthetic write failure')
      }),
    ).rejects.toThrow('synthetic write failure')
    await checkOracle()
    expect((await db.runAndReadAll('SELECT user_name FROM session_owners')).getRowObjects()).toEqual([
      { user_name: 'alice' },
    ])
  })

  it('publishes only committed owners and reloads the durable snapshot on open', async () => {
    await insert('a', 'alice', '2026-01-01')
    const previous = readSessionOwners()
    const run = db.run.bind(db)
    const writes = vi.spyOn(db, 'run').mockImplementation(async (sql, ...args) => {
      if (sql === 'COMMIT') expect(readSessionOwners()).toEqual(previous)
      return run(sql, ...args)
    })
    await writeSessionToken(db, 'a', () => db.run("UPDATE session_tokens SET user_name = 'bob' WHERE session_id = 'a'"))
    writes.mockRestore()
    expect(readSessionOwners()).toEqual([{ user_name: 'bob', first_seen: '2026-01-01', last_seen: '2026-01-01' }])
    // Simulate a durable commit from the previous process before this open.
    await db.run("UPDATE session_tokens SET created_at = '2026-02-01' WHERE session_id = 'a'")
    await db.run("UPDATE session_owners SET first_seen = '2026-02-01', last_seen = '2026-02-01'")
    await initSessionOwners(db)
    expect(readSessionOwners()).toEqual([{ user_name: 'bob', first_seen: '2026-02-01', last_seen: '2026-02-01' }])
    await checkOracle()
  })

  it('leaves an interrupted backfill retryable without replacing the committed roster', async () => {
    await insert('a', 'alice', '2026-01-01')
    // As in the legacy SQLite import: schema existed before bulk session copy.
    await db.run(
      "INSERT INTO session_tokens (session_id, token_id, user_name, created_at) VALUES ('b', 'tok', 'bob', '2026-02-01')",
    )
    await db.run("DELETE FROM settings WHERE key = 'session_owners_v1'")
    const run = db.run.bind(db)
    const failure = vi.spyOn(db, 'run').mockImplementation((sql, ...args) => {
      if (/INSERT INTO session_owners/.test(sql)) return Promise.reject(new Error('synthetic backfill failure'))
      return run(sql, ...args)
    })
    await expect(initSessionOwners(db)).rejects.toThrow('synthetic backfill failure')
    expect((await db.runAndReadAll('SELECT user_name FROM session_owners')).getRowObjects()).toEqual([
      { user_name: 'alice' },
    ])
    expect((await db.runAndReadAll("SELECT * FROM settings WHERE key = 'session_owners_v1'")).getRowObjects()).toEqual(
      [],
    )
    failure.mockRestore()
    await initSessionOwners(db)
    await checkOracle()
  })
})

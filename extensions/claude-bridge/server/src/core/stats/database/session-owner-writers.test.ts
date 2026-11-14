import 'reflect-metadata'
import { DuckDBInstance, type DuckDBConnection } from '@duckdb/node-api'
import { mkdtempSync, mkdirSync, writeFileSync, rmSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { Hono } from 'hono'
import { beforeEach, afterEach, expect, it, vi } from 'vitest'
import { initSchema } from './schema'
import { writeSessionToken } from './session-owners'
import { drainRollup } from './rollup'
import { statsWriteDepth } from './write-lock'
import type { PtySession, SessionConfig } from '../../pty/types'

const state = vi.hoisted(() => ({
  db: undefined as unknown as DuckDBConnection,
  discovered: undefined as unknown as (id: string) => void,
}))
vi.mock('./lifecycle', () => ({ getDb: async () => state.db, getDbBackground: async () => state.db }))
vi.mock('../../pty/session-manager', () => ({ getAllSessions: () => [] }))
vi.mock('../../logging/lifecycle', () => ({ lifecycleLog: vi.fn() }))
vi.mock('../../pty/jsonl-watcher', () => ({
  JsonlWatcher: class {
    constructor(...args: unknown[]) {
      state.discovered = args[5] as (id: string) => void
    }
    start() {}
  },
}))

let instance: DuckDBInstance
let dir: string
beforeEach(async () => {
  instance = await DuckDBInstance.create(':memory:')
  state.db = await instance.connect()
  await initSchema(state.db)
  dir = mkdtempSync(join(tmpdir(), 'session-owner-writers-'))
})
afterEach(async () => {
  await drainRollup()
  state.db.disconnectSync()
  instance.closeSync()
  rmSync(dir, { recursive: true, force: true })
})

async function owners() {
  return (
    await state.db.runAndReadAll('SELECT user_name, sessions FROM session_owners ORDER BY user_name')
  ).getRowObjects()
}

it('maintains owners through actual transcript discovery, duplicate discovery and HTTP deletion', async () => {
  const { attachSessionJsonlWatcher } = await import('../../pty/session-jsonl-wiring')
  const session = {
    id: 'pty',
    ws: { readyState: 3 },
    tokenId: 'synthetic-token',
    userName: 'alice',
    settingsDir: dir,
  } as PtySession
  attachSessionJsonlWatcher(session, {} as SessionConfig, 0, { recordCompactLink() {} })
  const discover = async (id: string) => {
    state.discovered(id)
    await vi.waitFor(async () => {
      expect(await owners()).toEqual([{ user_name: 'alice', sessions: 1n }])
      expect(statsWriteDepth()).toBe(0)
    })
  }
  await discover('conversation')
  await discover('conversation')

  const app = new Hono()
  ;(await import('../../dashboard/controllers/stats')).registerStatsRoutes(app)
  for (let i = 0; i < 2; i++) {
    expect((await app.request('/api/dashboard/sessions/conversation', { method: 'DELETE' })).status).toBe(200)
    expect(await owners()).toEqual([])
  }
})

it('heals an unattributed session through the real watcher callback', async () => {
  await writeSessionToken(state.db, 'legacy', () =>
    state.db.run(`INSERT INTO session_tokens
    (session_id, token_id, user_name, created_at) VALUES ('legacy', '', '', '2026-01-01')`),
  )
  const { attachSessionJsonlWatcher } = await import('../../pty/session-jsonl-wiring')
  attachSessionJsonlWatcher(
    { id: 'pty', ws: { readyState: 3 }, tokenId: 'synthetic-token', userName: 'bob', settingsDir: dir } as PtySession,
    {} as SessionConfig,
    0,
    { recordCompactLink() {} },
  )
  state.discovered('legacy')
  await vi.waitFor(async () => {
    expect(await owners()).toEqual([{ user_name: 'bob', sessions: 1n }])
    expect(statsWriteDepth()).toBe(0)
  })
})

it('maintains the same roster when the JSONL sweeper discovers the session first', async () => {
  await state.db.run(`INSERT INTO api_tokens (id, name, plugin_id, token, created_at)
    VALUES ('synthetic-token', 'carol', 'fixture', 'synthetic-only', '2026-01-01')`)
  const project = join(dir, 'projects', 'bridge-synthetic-token')
  mkdirSync(project, { recursive: true })
  const file = join(project, 'swept.jsonl')
  writeFileSync(
    file,
    JSON.stringify({
      type: 'user',
      uuid: 'u',
      timestamp: '2026-01-01',
      message: { role: 'user', content: 'synthetic' },
    }) + '\n',
  )
  const { sweepOneByPath } = await import('../jsonl-sweeper')
  expect((await sweepOneByPath(file)).inserted).toBe(1)
  expect(await owners()).toEqual([{ user_name: 'carol', sessions: 1n }])
  expect((await sweepOneByPath(file)).inserted).toBe(0)
  expect(await owners()).toEqual([{ user_name: 'carol', sessions: 1n }])
})

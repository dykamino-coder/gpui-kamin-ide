// Exercise the actual teardown path: default-level diagnostics must survive
// without persisting arbitrary legacy debug/error payloads.
import 'reflect-metadata'
import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { destroySession, detachSession, sessions } from './session-core'
import type { PtySession } from './types'
import { startSessionReaper, stopSessionReaper } from './session-reaper'

vi.mock('../logging', () => ({ debugLog: vi.fn(), infoLog: vi.fn(), warnLog: vi.fn(), errorLog: vi.fn() }))

let directory: string
beforeEach(() => {
  directory = fs.mkdtempSync(path.join(process.env.BRIDGE_TEST_TMP || os.tmpdir(), 'lifecycle-proof-'))
  vi.stubEnv('BRIDGE_LIFECYCLE_LOG_DIR', directory)
  vi.useFakeTimers()
})
afterEach(() => {
  stopSessionReaper()
  sessions.clear()
  vi.useRealTimers()
  vi.unstubAllEnvs()
})
function session(id: string): PtySession {
  const value = {
    id,
    state: 'running',
    createdAt: new Date(),
    lastActivityAt: new Date(),
    pty: { kill: vi.fn() },
    jsonlWatcher: { stop: vi.fn() },
    tokenId: 'fake-secret-token',
    userName: 'private-name',
    settingsDir: '',
  } as unknown as PtySession
  sessions.set(id, value)
  return value
}
function records(): Array<Record<string, unknown>> {
  return fs
    .readFileSync(path.join(directory, 'lifecycle.jsonl'), 'utf8')
    .trim()
    .split('\n')
    .map((line) => JSON.parse(line))
}
it('persists one default-level destruction without token, user or raw session identity', () => {
  session('private-session-reference')
  destroySession('private-session-reference')
  destroySession('private-session-reference')
  const rows = records()
  expect(rows.filter((row) => row.event === 'session_destroy')).toHaveLength(1)
  expect(rows[0]).toMatchObject({ event: 'session_destroy', reason: 'unspecified' })
  expect(JSON.stringify(rows)).not.toMatch(/fake-secret-token|private-name|private-session-reference/)
})
it('distinguishes detach grace expiry from an unspecified end', () => {
  session('private-detached-session')
  detachSession('private-detached-session')
  vi.advanceTimersByTime(10 * 60 * 1000)
  expect(records()).toEqual(
    expect.arrayContaining([
      expect.objectContaining({ event: 'session_detach', graceMs: 600000 }),
      expect.objectContaining({ event: 'session_destroy', reason: 'detach_grace' }),
    ]),
  )
})
it.each([
  ['startup_timeout', 'starting', 60001, 0],
  ['idle_timeout', 'running', 1900000, 1900000],
  ['max_lifetime', 'running', 86400001, 0],
])('persists the actual %s reaper decision', (reason, state, age, idle) => {
  const value = session('reaped-session')
  value.state = state as PtySession['state']
  value.createdAt = new Date(Date.now() - Number(age))
  value.lastActivityAt = new Date(Date.now() - Number(idle))
  startSessionReaper()
  vi.advanceTimersByTime(60000)
  expect(records()).toEqual(expect.arrayContaining([expect.objectContaining({ event: 'session_destroy', reason })]))
})

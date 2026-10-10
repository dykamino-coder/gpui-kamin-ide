import 'reflect-metadata'
import { afterAll, afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import type { PtySession } from '../pty/types'

const fixture = vi.hoisted(() => {
  const fs = require('node:fs')
  const os = require('node:os')
  const path = require('node:path')
  const root: string = fs.mkdtempSync(path.join(os.tmpdir(), 'br08-telemetry-'))
  process.env.BRIDGE_SYNC_BASE = path.join(root, 'data')
  return { root, sessions: [] as PtySession[], refresh: vi.fn() }
})
vi.mock('os', async (original) => {
  const actual = await original<typeof import('os')>()
  const replacement = { ...actual, homedir: () => fixture.root }
  return { ...replacement, default: replacement }
})
vi.mock('../auth/tokens', () => ({
  resolveToken: async (value: string) =>
    value === 'synthetic-owner' || value === 'synthetic-other' ? { tokenId: value } : null,
}))
vi.mock('../pty/session-core', () => ({ getAllSessions: () => fixture.sessions }))
vi.mock('../pty/session-settings', () => ({ refreshSessionSkills: fixture.refresh }))
vi.mock('../logging', () => ({ debugLog: vi.fn(), warnLog: vi.fn() }))

import { createSyncRoutes, tokenHash } from './routes'
import { recordSkillsSnapshot } from './skills-reload-telemetry'
import {
  clearSessionInputState,
  notifySessionAttachmentChanged,
  setSessionPromptReady,
  writeCoordinatedInput,
} from '../pty/session-input-coordinator'
import fs from 'node:fs'
import path from 'node:path'

let app: ReturnType<typeof createSyncRoutes>
let sequence = 0
let owner: string
let hash: string
let echo: (() => void) | null
let writes: string[]
let session: PtySession

async function post(
  source: 'user' | 'project',
  skills?: Record<string, string>,
  projectPath = '/synthetic/private-project',
) {
  const response = await app.request(`/api/sync/${hash}/${source}`, {
    method: 'POST',
    headers: { Authorization: `Bearer ${owner}`, 'Content-Type': 'application/json' },
    body: JSON.stringify({
      ...(skills === undefined ? {} : { skills }),
      ...(source === 'project' ? { projectPath } : {}),
    }),
  })
  expect(response.status).toBe(200)
}
async function events(token = hash, bearer = owner): Promise<Array<Record<string, any>>> {
  const response = await app.request(`/api/sync/${token}/reload-diagnostics`, {
    headers: { Authorization: `Bearer ${bearer}` },
  })
  expect(response.status).toBe(200)
  return ((await response.json()) as { events: Array<Record<string, any>> }).events
}
function complete() {
  vi.advanceTimersByTime(50)
  echo?.()
  vi.advanceTimersByTime(160)
}
beforeEach(() => {
  vi.useFakeTimers()
  vi.setSystemTime(new Date('2026-10-09T12:00:00Z'))
  owner = 'synthetic-owner'
  hash = tokenHash(owner)
  // Unique session identity per test; snapshots use unique paths as well.
  writes = []
  echo = null
  session = {
    id: `private-session-${++sequence}`,
    bearerHash: hash,
    cwd: '/synthetic/private-project',
    settingsDir: '/synthetic/settings',
    state: 'running',
    detachedAt: new Date(),
    pty: {
      write: (value: string) => writes.push(value),
      onData: (cb: () => void) => {
        echo = cb
        return {
          dispose: () => {
            if (echo === cb) echo = null
          },
        }
      },
    },
  } as unknown as PtySession
  fixture.sessions = [session]
  fixture.refresh.mockReset()
  const data = path.join(fixture.root, 'data')
  if (path.dirname(data) !== fixture.root) throw new Error('Unsafe fixture path')
  fs.rmSync(data, { recursive: true, force: true })
  app = createSyncRoutes()
})
afterEach(() => {
  clearSessionInputState(session)
  vi.useRealTimers()
})
afterAll(() => {
  const root = path.resolve(fixture.root)
  if (path.basename(root).startsWith('br08-telemetry-') && root === fixture.root)
    fs.rmSync(root, { recursive: true, force: true })
})

describe('BR-08 actual sync to PTY diagnostic correlation', () => {
  it('correlates a deferred changed snapshot with reattach and actual Enter, without private data', async () => {
    const since = Date.now()
    await post('user', { 'private-skill/SKILL.md': 'synthetic confidential body' })
    setSessionPromptReady(session, true)
    vi.advanceTimersByTime(5000)
    expect(writes).toEqual([])
    session.detachedAt = null
    notifySessionAttachmentChanged(session)
    vi.advanceTimersByTime(0)
    complete()
    const trace = (await events()).filter((e) => e.at >= since)
    const queued = trace.findLast((e) => e.stage === 'queued')!
    const entered = trace.findLast((e) => e.stage === 'enter-written')!
    expect(queued).toMatchObject({ source: 'user', changed: true, blockedBy: 'detached', queuedAt: since })
    expect(entered).toMatchObject({
      token: queued.token,
      session: queued.session,
      snapshotRevision: queued.snapshotRevision,
      maintenanceRevision: queued.maintenanceRevision,
      reason: 'reattach',
      queuedAt: since,
      at: since + 5130,
    })
    expect(writes).toEqual(['\x15', '\x1b[200~/reload-skills\x1b[201~', '\r'])
    const text = JSON.stringify(trace)
    for (const privateValue of [owner, hash, session.id, session.cwd!, 'private-skill', 'confidential body'])
      expect(text).not.toContain(privateValue)
  })
  it('identifies no-op/omitted syncs without scheduling another reload', async () => {
    const snapshot = { 'same/SKILL.md': 'same' }
    await post('user', snapshot)
    const first = (await events()).at(-1)!
    await post('user', snapshot)
    const beforeOmission = await events()
    await post('user')
    const trace = await events()
    expect(trace).toEqual(beforeOmission)
    const snapshots = trace.filter((e) => e.stage === 'snapshot' && e.snapshotRevision === first.snapshotRevision)
    expect(snapshots.at(-1)).toMatchObject({ changed: false })
    expect(trace.filter((e) => e.stage === 'queued' && e.session === first.session)).toHaveLength(1)
  })
  it('preserves the exact revision submitted while a newer project sync coalesces', async () => {
    session.detachedAt = null
    setSessionPromptReady(session, true)
    await post('project', { 'one/SKILL.md': 'first' })
    vi.advanceTimersByTime(0)
    vi.advanceTimersByTime(50)
    await post('project', { 'one/SKILL.md': 'second' })
    const queued = (await events()).filter((e) => e.stage === 'queued').slice(-2)
    expect(queued[1]).toMatchObject({ source: 'project', coalesced: true })
    echo?.()
    vi.advanceTimersByTime(160)
    let entered = (await events()).filter((e) => e.stage === 'enter-written')
    expect(entered.at(-1)?.snapshotRevision).toBe(queued[0]!.snapshotRevision)
    setSessionPromptReady(session, true)
    vi.advanceTimersByTime(0)
    complete()
    entered = (await events()).filter((e) => e.stage === 'enter-written')
    expect(entered.at(-1)).toMatchObject({ snapshotRevision: queued[1]!.snapshotRevision, reason: 'prompt-ready' })
  })
  it('records raw-draft deferral and teardown without inventing a CLI acknowledgement', async () => {
    session.detachedAt = null
    setSessionPromptReady(session, true)
    writeCoordinatedInput(session, 'synthetic private draft')
    await post('user', { 'draft/SKILL.md': 'draft source' })
    expect((await events()).at(-1)).toMatchObject({ stage: 'queued', blockedBy: 'raw-input' })
    clearSessionInputState(session)
    vi.advanceTimersByTime(10000)
    expect((await events()).at(-1)).toMatchObject({ stage: 'cancelled', reason: 'teardown' })
    expect(writes).toEqual(['synthetic private draft'])
  })
  it('enforces owner authentication and emits no trace for a different project', async () => {
    await post('project', { 'excluded/SKILL.md': 'excluded' }, '/other-project')
    expect((await events()).at(-1)).toMatchObject({ stage: 'snapshot', source: 'project' })
    for (const [token, bearer, status] of [
      [hash, '', 401],
      [hash, 'synthetic-other', 403],
    ] as const) {
      const response = await app.request(`/api/sync/${token}/reload-diagnostics`, {
        headers: { Authorization: `Bearer ${bearer}` },
      })
      expect(response.status).toBe(status)
    }
    expect(await events(tokenHash('synthetic-other'), 'synthetic-other')).toEqual([])
  })
  it('bounds retained records and returns a detached response snapshot', async () => {
    fixture.sessions = []
    await post('user', { 'bounded/SKILL.md': 'initial' })
    for (let i = 0; i < 270; i++) recordSkillsSnapshot(hash, 'user', undefined, { 'bounded/SKILL.md': String(i) }, true)
    const trace = await events()
    expect(trace).toHaveLength(256)
    expect(trace.every((e) => Buffer.byteLength(JSON.stringify(e)) < 1024)).toBe(true)
    expect(JSON.stringify(trace[0])).not.toContain('bounded/SKILL.md')
    trace[0]!.snapshotRevision = 'mutated'
    expect((await events())[0]!.snapshotRevision).not.toBe('mutated')
  })
  it('distinguishes failed overlay refresh and failed PTY Enter from acknowledgement', async () => {
    fixture.refresh.mockImplementation(() => {
      throw new Error('synthetic private failure')
    })
    session.detachedAt = null
    setSessionPromptReady(session, true)
    const originalWrite = session.pty.write.bind(session.pty)
    session.pty.write = (value: string) => {
      if (value === '\r') throw new Error('synthetic exited PTY')
      originalWrite(value)
    }
    await post('user', { 'error/SKILL.md': 'private error fixture' })
    const queued = (await events()).at(-1)!
    expect(queued).toMatchObject({ stage: 'queued', overlayRefreshed: false })
    vi.advanceTimersByTime(0)
    complete()
    expect((await events()).filter((e) => e.stage === 'enter-written' && e.session === queued.session)).toEqual([])
    clearSessionInputState(session)
  })
  it('uses a stable canonical request revision within a boot, isolated by owner and source', () => {
    const a = recordSkillsSnapshot(hash, 'user', undefined, { a: 'one', b: 'two' }, true)
    const b = recordSkillsSnapshot(hash, 'user', undefined, { b: 'two', a: 'one' }, false)
    expect(a.snapshotRevision).toBe(b.snapshotRevision)
    expect(recordSkillsSnapshot(hash, 'project', '/private', { a: 'one', b: 'two' }, true).snapshotRevision).not.toBe(
      a.snapshotRevision,
    )
    expect(
      recordSkillsSnapshot(tokenHash('synthetic-other'), 'user', undefined, { a: 'one', b: 'two' }, true)
        .snapshotRevision,
    ).not.toBe(a.snapshotRevision)
    expect(recordSkillsSnapshot(hash, 'user', undefined, { a: 'one', b: 'changed' }, true).snapshotRevision).not.toBe(
      a.snapshotRevision,
    )
  })
})

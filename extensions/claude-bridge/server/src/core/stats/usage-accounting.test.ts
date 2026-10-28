import 'reflect-metadata'
import { mkdtempSync, mkdirSync, writeFileSync, rmSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { afterAll, beforeAll, describe, expect, it, vi } from 'vitest'
import { Hono } from 'hono'
import type { SynthAssistantEntry } from '../proxy/entry-synthesizer'

let dir: string
let lc: typeof import('./database/lifecycle')
let sweep: typeof import('./jsonl-sweeper')
let record: typeof import('./proxy-usage').recordProxyUsage
let app: Hono
const timestamp = '2026-10-01T12:00:00.000Z'
const owner = { sessionId: 'main', tokenId: 'alice-token', userName: 'alice' }

beforeAll(async () => {
  dir = mkdtempSync(join(tmpdir(), 'bridge-accounting-'))
  vi.spyOn(process, 'cwd').mockReturnValue(dir)
  vi.resetModules()
  lc = await import('./database/lifecycle')
  sweep = await import('./jsonl-sweeper')
  record = (await import('./proxy-usage')).recordProxyUsage
  await lc.initStatsDb()
  const db = await lc.getDb()
  for (const id of ['main', 'child']) {
    await db.run(
      `INSERT INTO session_tokens (session_id, token_id, user_name, created_at, deleted)
      VALUES (?, ?, ?, ?, 0)`,
      [id, owner.tokenId, owner.userName, timestamp],
    )
  }
  app = new Hono()
  ;(await import('../dashboard/controllers/users')).registerUserRoutes(app)
})

afterAll(async () => {
  await lc.closeDb()
  vi.restoreAllMocks()
  rmSync(dir, { recursive: true, force: true })
})

function assistant(
  id: string,
  usage = {
    input_tokens: 2,
    output_tokens: 4,
    cache_creation_input_tokens: 57871,
    cache_read_input_tokens: 0,
    cache_creation: { ephemeral_1h_input_tokens: 57871 },
  },
): SynthAssistantEntry {
  return {
    type: 'assistant',
    uuid: id,
    timestamp,
    parentUuid: null,
    isSidechain: false,
    __streaming: false,
    message: { id, role: 'assistant', model: 'claude-haiku-5-5', content: [], usage },
  }
}

async function sweepEntries(sessionId: string, entries: unknown[]): Promise<void> {
  const folder = join(dir, 'projects', 'slug')
  mkdirSync(folder, { recursive: true })
  const path = join(folder, `${sessionId}.jsonl`)
  writeFileSync(path, entries.map((e) => JSON.stringify(e)).join('\n') + '\n')
  await sweep.sweepOneByPath(path)
}

async function summary(): Promise<{ tokens: { input: number; output: number; cacheCreation: number }; cost: number }> {
  const res = await app.request('/api/dashboard/users')
  expect(res.status).toBe(200)
  return ((await res.json()) as Array<any>).find((r) => r.userName === 'alice')
}

describe('durable cost accounting across CLI/proxy/subagent paths', () => {
  it('buffers startup side-call numbers without retaining prompt content', async () => {
    const { createProxyUsageRecorder } = await import('./proxy-usage')
    const persist = vi.fn(async () => {})
    const recorder = createProxyUsageRecorder(owner, vi.fn(), persist)
    const entry = assistant('startup')
    entry.message.content.push({ type: 'text', text: 'Do not retain this text' })
    recorder.record(entry)
    expect(persist).not.toHaveBeenCalled()
    recorder.bind('discovered')
    expect(persist).toHaveBeenCalledTimes(1)
    expect(persist.mock.calls[0]).toEqual([
      {
        timestamp,
        isSidechain: false,
        message: { id: 'startup', model: entry.message.model, usage: entry.message.usage },
      },
      { ...owner, sessionId: 'discovered' },
    ])
    recorder.bind('compacted')
    expect(persist).toHaveBeenCalledTimes(1)
  })
  it('matches real CLI hourly-cache cost and converges when split JSONL arrives', async () => {
    const entry = assistant('api-main')
    await record(entry, owner)
    await record(entry, owner)
    expect((await summary()).cost).toBeCloseTo(0.0115764, 10)
    await sweepEntries('main', [
      {
        ...entry,
        uuid: 'split-1',
        message: { ...entry.message, usage: { ...entry.message.usage, output_tokens: 0, cache_creation: undefined } },
      },
      {
        ...entry,
        uuid: 'split-2',
        message: { ...entry.message, usage: { ...entry.message.usage, output_tokens: 0, cache_creation: undefined } },
      },
    ])
    const result = await summary()
    expect(result.cost).toBeCloseTo(0.0115764, 10)
    expect(result.tokens.cacheCreation).toBe(57871)
    const db = await lc.getDb()
    expect((await db.runAndReadAll('SELECT COUNT(*) AS n FROM jsonl_events')).getRowObjects()[0]?.n).toBe(2n)
  })

  it('includes subagents and hidden proxy-only side-calls exactly once', async () => {
    const child = { ...assistant('api-child'), isSidechain: true }
    await record(child, owner)
    await sweepEntries('child', [{ ...child, uuid: 'child-row' }])
    await record({ ...assistant('hidden-side-call'), isSidechain: true }, owner)
    expect((await summary()).cost).toBeCloseTo(3 * 0.0115764, 10)
    const db = await lc.getDb()
    await db.run('UPDATE session_tokens SET deleted = 1 WHERE session_id = ?', ['child'])
    expect((await summary()).cost).toBeCloseTo(2 * 0.0115764, 10)
  })
})

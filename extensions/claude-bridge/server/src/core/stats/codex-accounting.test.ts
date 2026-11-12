import 'reflect-metadata'
import { mkdtempSync, mkdirSync, writeFileSync, rmSync } from 'node:fs'
import os, { tmpdir } from 'node:os'
import { join } from 'node:path'
import { afterAll, beforeAll, describe, expect, it, vi } from 'vitest'
import { Hono } from 'hono'

// A chat that ran on both engines: Claude lines (no engine field) and codex
// projection lines (engine "codex", usage on system/api_usage lines).

let dir: string
let lc: typeof import('./database/lifecycle')
let sweep: typeof import('./jsonl-sweeper')
let rescan: typeof import('./codex-rescan')
let app: Hono
const ts = '2026-11-10T12:00:00.000Z'
const user = 'alice'

beforeAll(async () => {
  dir = mkdtempSync(join(tmpdir(), 'bridge-codex-accounting-'))
  vi.spyOn(process, 'cwd').mockReturnValue(dir)
  // sweepAll lists ~/.claude/projects.
  vi.spyOn(os, 'homedir').mockReturnValue(dir)
  vi.resetModules()
  lc = await import('./database/lifecycle')
  sweep = await import('./jsonl-sweeper')
  rescan = await import('./codex-rescan')
  await lc.initStatsDb()
  const db = await lc.getDb()
  for (const id of ['mixed', 'codex-only', 'legacy']) {
    await db.run(
      `INSERT INTO session_tokens (session_id, token_id, user_name, created_at, deleted) VALUES (?, ?, ?, ?, 0)`,
      [id, 'alice-token', user, ts],
    )
  }
  app = new Hono()
  ;(await import('../dashboard/controllers/users')).registerUserRoutes(app)
  ;(await import('../dashboard/controllers/stats')).registerStatsRoutes(app)
})

afterAll(async () => {
  await lc.closeDb()
  vi.restoreAllMocks()
  rmSync(dir, { recursive: true, force: true })
})

const claudeUser = (uuid: string, sessionId: string) => ({
  type: 'user',
  uuid,
  parentUuid: null,
  sessionId,
  timestamp: ts,
  message: { role: 'user', content: 'hi' },
})
const claudeReply = (uuid: string, sessionId: string) => ({
  type: 'assistant',
  uuid,
  parentUuid: null,
  sessionId,
  timestamp: ts,
  message: {
    id: `msg-${uuid}`,
    role: 'assistant',
    model: 'claude-haiku-5-5',
    content: [{ type: 'text', text: 'ok' }],
    usage: { input_tokens: 10, output_tokens: 5, cache_read_input_tokens: 0, cache_creation_input_tokens: 0 },
  },
})
const codexUser = (uuid: string, sessionId: string) => ({ ...claudeUser(uuid, sessionId), engine: 'codex' })
const codexItem = (uuid: string, sessionId: string, content: unknown[]) => ({
  type: 'assistant',
  uuid,
  parentUuid: null,
  sessionId,
  timestamp: ts,
  engine: 'codex',
  message: { id: uuid, role: 'assistant', model: 'gpt-5.5', content },
})
const codexUsage = (uuid: string, sessionId: string, model = 'gpt-5.5', input = 1000, cached = 3000, output = 200) => ({
  type: 'system',
  subtype: 'api_usage',
  uuid,
  parentUuid: null,
  sessionId,
  timestamp: ts,
  engine: 'codex',
  model,
  usage: {
    input_tokens: input,
    cache_read_input_tokens: cached,
    cache_creation_input_tokens: 0,
    output_tokens: output,
    reasoning_output_tokens: 50,
  },
})

function write(sessionId: string, entries: unknown[]): string {
  const folder = join(dir, '.claude', 'projects', 'slug')
  mkdirSync(folder, { recursive: true })
  const path = join(folder, `${sessionId}.jsonl`)
  writeFileSync(path, entries.map((e) => JSON.stringify(e)).join('\n') + '\n')
  return path
}

async function rows(sessionId: string): Promise<Array<Record<string, any>>> {
  const db = await lc.getDb()
  return (
    await db.runAndReadAll(
      `SELECT type, engine, message_id, model, input_tokens, output_tokens, cache_read_tokens, tool_name
       FROM jsonl_accounting_events WHERE session_id = ? ORDER BY type, message_id`,
      [sessionId],
    )
  ).getRowObjects() as Array<Record<string, any>>
}

async function json(url: string): Promise<any> {
  const res = await app.request(url)
  expect(res.status).toBe(200)
  return res.json()
}

describe('codex in Bridge accounting', () => {
  it('counts a codex usage line as one response and item lines as none', async () => {
    await sweep.sweepOneByPath(
      write('mixed', [
        claudeUser('c-u1', 'mixed'),
        claudeReply('c-a1', 'mixed'),
        codexUser('x-u1', 'mixed'),
        codexItem('x-think', 'mixed', [{ type: 'thinking', thinking: 'hmm' }]),
        codexItem('x-tool', 'mixed', [{ type: 'tool_use', id: 't1', name: 'mcp__bridge__Read', input: {} }]),
        codexItem('x-text', 'mixed', [{ type: 'text', text: 'done' }]),
        codexUsage('usage:turn1:4200', 'mixed'),
      ]),
    )
    const r = await rows('mixed')
    const assistants = r.filter((x) => x.type === 'assistant')
    expect(assistants.map((x) => [x.engine, x.message_id])).toEqual([
      ['claude', 'msg-c-a1'],
      ['codex', 'usage:turn1:4200'],
    ])
    expect(assistants[1]).toMatchObject({ model: 'gpt-5.5', input_tokens: 1000, output_tokens: 200 })
    expect(Number(assistants[1]!.cache_read_tokens)).toBe(3000)
    // The codex tool call still counts as a tool use, on the codex engine.
    expect(r.filter((x) => x.type === 'tool_use').map((x) => [x.engine, x.tool_name])).toEqual([
      ['codex', 'mcp__bridge__Read'],
    ])
    expect(
      r
        .filter((x) => x.type === 'user')
        .map((x) => x.engine)
        .sort(),
    ).toEqual(['claude', 'codex'])
  })

  it('splits the overview and the session list by engine', async () => {
    const all = await json('/api/dashboard/stats/overview?range=all')
    expect(all.engines.codex).toMatchObject({ sessions: 1, assistantMessages: 1, userMessages: 1 })
    expect(all.engines.claude).toMatchObject({ sessions: 1, assistantMessages: 1, userMessages: 1 })
    const codex = await json('/api/dashboard/stats/overview?range=all&engine=codex')
    expect(codex.engine).toBe('codex')
    expect(codex.assistantMessages).toBe(1)
    expect(codex.models.map((m: any) => m.model ?? m.name)).toEqual(['gpt-5.5'])
    const list = await json('/api/dashboard/tokens/alice/sessions')
    expect(list.sessions.find((s: any) => s.sessionId === 'mixed').engines).toEqual(['claude', 'codex'])
  })

  it('prices codex as the OpenAI API equivalent and keeps unpriced models as tokens', async () => {
    await sweep.sweepOneByPath(
      write('codex-only', [
        codexUser('y-u1', 'codex-only'),
        codexUsage('usage:t:1', 'codex-only', 'gpt-5.5', 200_000, 0, 0),
        codexUsage('usage:t:2', 'codex-only', 'codex-unreleased', 700, 0, 300),
      ]),
    )
    const users = (await json('/api/dashboard/users')) as Array<any>
    const alice = users.find((u) => u.userName === user)
    const rate = (await import('../../shared/openai-pricing')).openAiRateFor('gpt-5.5')!
    // gpt-5.5: 200K fresh input, plus the earlier response's 1000 in / 3000 cached / 200 out.
    const expected = 0.2 * rate.input + (1000 * rate.input + 3000 * rate.cachedInput! + 200 * rate.output) / 1_000_000
    expect(alice.codex.apiEquivalentUsd).toBeCloseTo(expected, 9)
    expect(alice.codex.unpricedTokens).toBe(1000)
    expect(alice.codex.unpricedModels).toEqual(['codex-unreleased'])
    expect(alice.codex.pricesAsOf).toMatch(/^\d{4}-\d{2}-\d{2}$/)
    expect(alice.engines.codex.responses).toBe(3)
    expect(alice.engines.claude.responses).toBe(1)
    // Claude cost never includes codex tokens.
    const claudeOnly = ((await json('/api/dashboard/users?engine=claude')) as Array<any>).find(
      (u) => u.userName === user,
    )
    expect(claudeOnly.codex.apiEquivalentUsd).toBe(0)
    expect(claudeOnly.tokens.input).toBe(10)
  })

  it('heals a transcript swept before engines once, idempotently', async () => {
    const db = await lc.getDb()
    // An older Bridge: codex item lines stored as zero-token Claude responses, no engine.
    const lines = [
      codexUser('z-u1', 'legacy'),
      codexItem('z-text', 'legacy', [{ type: 'text', text: 'old' }]),
      codexUsage('usage:z:9', 'legacy'),
    ]
    const path = write('legacy', lines)
    await sweep.sweepOneByPath(path)
    await db.run(`UPDATE jsonl_events SET engine = NULL WHERE session_id = 'legacy'`)
    await db.run(
      `INSERT INTO jsonl_events (uuid, session_id, type, timestamp, message_id, model, input_tokens, output_tokens)
       VALUES ('z-text', 'legacy', 'assistant', ?, 'z-text', 'gpt-5.5', 0, 0)`,
      [ts],
    )
    await db.run(`DELETE FROM settings WHERE key = ?`, [rescan.CODEX_RESCAN_FLAG])
    expect((await rows('legacy')).filter((x) => x.type === 'assistant')).toHaveLength(2)

    // The real path: sweepAll resets the cursor and reads the unchanged file again.
    await sweep.sweepAll()
    const flag = await db.runAndReadAll('SELECT 1 FROM settings WHERE key = ?', [rescan.CODEX_RESCAN_FLAG])
    expect(flag.getRowObjects()).toHaveLength(1)
    const healed = await rows('legacy')
    expect(healed.filter((x) => x.type === 'assistant').map((x) => x.message_id)).toEqual(['usage:z:9'])
    expect(healed.every((x) => x.engine === 'codex')).toBe(true)

    // Second run: flag set, no reset, nothing changes.
    expect(await rescan.rescanCodexTranscriptsOnce([path])).toEqual([])
    await sweep.sweepAll()
    expect(await rows('legacy')).toEqual(healed)

    // Only swept codex transcripts are reset: not Claude ones, not unknown files.
    await db.run('DELETE FROM settings WHERE key = ?', [rescan.CODEX_RESCAN_FLAG])
    const mixed = join(dir, '.claude', 'projects', 'slug', 'mixed.jsonl')
    expect(await rescan.rescanCodexTranscriptsOnce([mixed, path, join(dir, 'missing.jsonl')])).toEqual([path])
  })

  it('splits the per-user time series by engine', async () => {
    const { getUserTimeSeries } = await import('./database/aggregates')
    const points = await getUserTimeSeries('day', 0, null)
    const engines = new Set(points.filter((p: any) => p.userName === user || p.user === user).map((p: any) => p.engine))
    expect(engines).toEqual(new Set(['claude', 'codex']))
    const codex = await getUserTimeSeries('day', 0, 'codex')
    expect(codex.every((p: any) => p.engine === 'codex')).toBe(true)
  })
})

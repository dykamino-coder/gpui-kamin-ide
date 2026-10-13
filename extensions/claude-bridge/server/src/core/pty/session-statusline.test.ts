import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { execFileSync } from 'child_process'
import fs from 'fs'
import os from 'os'
import path from 'path'

const sent = vi.hoisted(() => [] as Array<Record<string, unknown>>)
vi.mock('./session-io', () => ({
  sendToClient: (_ws: unknown, msg: Record<string, unknown>) => {
    sent.push(msg)
    return true
  },
}))

import { currentAccountLimits, recordAccountLimits, resetAccountLimitsForTests } from './account-limits'
import {
  parseSessionStatus,
  readSessionStatus,
  resendSessionUsage,
  STATUSLINE_RESULT_NAME,
  watchSessionStatusLine,
  withAccountLimits,
  writeStatusLineScript,
} from './session-statusline'
import type { PtySession } from '../types/pty'

const FUTURE = Math.floor(Date.now() / 1000) + 3600
const WEEK = FUTURE + 5 * 86400

// Shape of a real 2.1.296 status-line payload (trimmed), incl. fields that
// must NOT leave the container (paths, session id, cost, workspace).
const PAYLOAD = {
  session_id: 'b62d0f03-424c-4838-9479-67f67002e068',
  transcript_path: '/home/bridge/.claude/projects/x/y.jsonl',
  cwd: '/home/bridge/secret-project',
  model: { id: 'claude-sonnet-4-6', display_name: 'Sonnet 4.6' },
  effort: { level: 'medium' },
  version: '2.1.296',
  cost: { total_cost_usd: 1.23 },
  context_window: { used_percentage: 12, context_window_size: 200000 },
  rate_limits: {
    five_hour: { used_percentage: 7, resets_at: FUTURE },
    seven_day: { used_percentage: 6.5, resets_at: WEEK },
  },
}

let tmp: string | null = null
let store: string
beforeEach(() => {
  store = fs.mkdtempSync(path.join(os.tmpdir(), 'bridge-limits-store-'))
  resetAccountLimitsForTests(path.join(store, 'statusline-limits.json'))
})
afterEach(() => {
  sent.length = 0
  resetAccountLimitsForTests(null)
  fs.rmSync(store, { recursive: true, force: true })
  if (tmp) fs.rmSync(tmp, { recursive: true, force: true })
  tmp = null
})

describe('parseSessionStatus', () => {
  it('maps the sanitized snapshot to the wire payload', () => {
    const usage = parseSessionStatus({
      observedAt: '2026-10-13T15:35:00.000Z',
      version: '2.1.296',
      rate_limits: PAYLOAD.rate_limits,
      context: { used_percentage: 12, size: 200000 },
      model: 'claude-sonnet-4-6',
      effort: 'medium',
    })
    expect(usage).toEqual({
      windows: [
        { id: 'five_hour', label: '5 hours', percent: 7, resetsAt: new Date(FUTURE * 1000).toISOString() },
        { id: 'seven_day', label: 'Week (all)', percent: 6.5, resetsAt: new Date(WEEK * 1000).toISOString() },
      ],
      contextPercent: 12,
      contextWindowSize: 200000,
      model: 'claude-sonnet-4-6',
      effort: 'medium',
      observedAt: '2026-10-13T15:35:00.000Z',
    })
  })

  it('tolerates absent limits (API key / before the first response) and junk', () => {
    expect(parseSessionStatus({ context: { used_percentage: null, size: 200000 } })).toMatchObject({
      windows: [],
      contextPercent: null,
      contextWindowSize: 200000,
    })
    expect(parseSessionStatus(null)).toBeNull()
    expect(parseSessionStatus([])).toBeNull()
    const junk = parseSessionStatus({
      rate_limits: {
        five_hour: { used_percentage: 140, resets_at: FUTURE },
        seven_day: { used_percentage: 5, resets_at: 1 },
      },
      context: { used_percentage: -1 },
      model: 'x y\r',
    })
    expect(junk).toMatchObject({ windows: [], contextPercent: null, model: null })
  })
})

describe('status-line script', () => {
  it('keeps only limits, context, model and effort from the CLI payload', () => {
    tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'bridge-sl-test-'))
    const setting = writeStatusLineScript(tmp)
    expect(setting.type).toBe('command')
    const script = path.join(tmp, 'bridge-statusline.cjs')
    const result = path.join(tmp, STATUSLINE_RESULT_NAME)
    const out = execFileSync(process.execPath, [script, result], { input: JSON.stringify(PAYLOAD) }).toString()
    expect(out).toBe('')
    const written = JSON.parse(fs.readFileSync(result, 'utf8'))
    expect(Object.keys(written).sort()).toEqual(['context', 'effort', 'model', 'observedAt', 'rate_limits', 'version'])
    expect(JSON.stringify(written)).not.toMatch(/secret-project|b62d0f03|total_cost/)
    expect(parseSessionStatus(written)).toMatchObject({
      contextPercent: 12,
      effort: 'medium',
      model: 'claude-sonnet-4-6',
    })
  })

  it('pushes a changed snapshot once and re-sends it on reattach', () => {
    tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'bridge-sl-test-'))
    const result = path.join(tmp, STATUSLINE_RESULT_NAME)
    const session = { id: 's1', ws: {} } as unknown as PtySession
    const previous = { own: null, key: '' }
    const snapshot = { rate_limits: PAYLOAD.rate_limits, context: { used_percentage: 12, size: 200000 } }
    fs.writeFileSync(result, JSON.stringify({ ...snapshot, observedAt: '2026-10-13T15:00:00.000Z' }))
    readSessionStatus(session, result, previous)
    fs.writeFileSync(result, JSON.stringify({ ...snapshot, observedAt: '2026-10-13T15:01:00.000Z' }))
    readSessionStatus(session, result, previous)
    expect(sent).toHaveLength(1)
    expect(sent[0]).toMatchObject({ type: 'session:usage', sessionId: 's1', usage: { contextPercent: 12 } })
    resendSessionUsage(session)
    expect(sent).toHaveLength(2)
    fs.writeFileSync(result, 'x'.repeat(5000))
    readSessionStatus(session, result, previous)
    expect(sent).toHaveLength(2)
  })
})

const WINDOWS = [
  { id: 'five_hour', label: '5 hours', percent: 7, resetsAt: new Date(FUTURE * 1000).toISOString() },
  { id: 'seven_day', label: 'Week (all)', percent: 6.5, resetsAt: new Date(WEEK * 1000).toISOString() },
]
const ctxOnly = (pct: number) => ({
  observedAt: new Date().toISOString(),
  rate_limits: {},
  context: { used_percentage: pct, size: 200000 },
})

describe('account-wide limits (CLI has no rate_limits before its first response)', () => {
  it('fills a resumed session with the last windows any CLI reported, marked by their report time', () => {
    recordAccountLimits(WINDOWS, '2026-10-14T09:00:00.000Z')
    const own = parseSessionStatus(ctxOnly(65))!
    expect(own.windows).toEqual([])
    const shown = withAccountLimits(own)
    expect(shown.windows.map((w) => [w.id, w.percent])).toEqual([
      ['five_hour', 7],
      ['seven_day', 6.5],
    ])
    expect(shown.limitsObservedAt).toBe('2026-10-14T09:00:00.000Z')
    expect(shown.contextPercent).toBe(65)
  })

  it('prefers the own fresh windows of the session', () => {
    recordAccountLimits(WINDOWS, '2026-10-14T09:00:00.000Z')
    const own = parseSessionStatus({
      ...ctxOnly(3),
      rate_limits: { five_hour: { used_percentage: 9, resets_at: FUTURE } },
    })!
    expect(withAccountLimits(own).windows.map((w) => [w.id, w.percent])).toEqual([['five_hour', 9]])
  })

  it('ignores older reports, drops reset windows and survives a server restart', () => {
    recordAccountLimits(WINDOWS, '2026-10-14T09:00:00.000Z')
    expect(recordAccountLimits([{ ...WINDOWS[0]!, percent: 1 }], '2026-10-14T08:00:00.000Z')).toBe(false)
    resetAccountLimitsForTests(path.join(store, 'statusline-limits.json')) // drop memory, keep the file
    expect(currentAccountLimits()?.windows.map((w) => w.percent)).toEqual([7, 6.5])
    const afterFiveHourReset = (FUTURE + 60) * 1000
    expect(currentAccountLimits(afterFiveHourReset)?.windows.map((w) => w.id)).toEqual(['seven_day'])
    expect(currentAccountLimits((WEEK + 60) * 1000)).toBeNull()
  })

  it('pushes the windows to a live session as soon as another session reports them', async () => {
    tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'bridge-sl-test-'))
    const resumed = { id: 'resumed', ws: {} } as unknown as PtySession
    fs.writeFileSync(path.join(tmp, STATUSLINE_RESULT_NAME), JSON.stringify(ctxOnly(65)))
    const watcher = watchSessionStatusLine(resumed, tmp)
    try {
      await vi.waitFor(() => expect(sent).toHaveLength(1))
      expect(sent[0]).toMatchObject({ sessionId: 'resumed', usage: { contextPercent: 65, windows: [] } })
      const active = { id: 'active', ws: {} } as unknown as PtySession
      const other = path.join(tmp, 'other.json')
      fs.writeFileSync(other, JSON.stringify({ ...ctxOnly(3), rate_limits: PAYLOAD.rate_limits }))
      readSessionStatus(active, other, { own: null, key: '' })
      const update = sent.find(
        (m) => m.sessionId === 'resumed' && (m.usage as { windows: unknown[] }).windows.length === 2,
      )
      expect(update).toMatchObject({ usage: { contextPercent: 65 } })
    } finally {
      watcher.stop()
    }
  })
})

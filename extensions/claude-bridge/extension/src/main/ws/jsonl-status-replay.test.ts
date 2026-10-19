import { beforeEach, describe, expect, it, vi } from 'vitest'
import { handleServerMessage, type HandlerCtx } from './handle-server-message'

let cached: any
let ctx: HandlerCtx
const send = vi.fn()
const index = { boundaries: [{ ts: '2026-01-01T01:00:00Z' }], counts: [4, 1] }
beforeEach(() => {
  cached = null
  send.mockClear()
  ctx = {
    tabId: 'parent-a', window: { webContents: { send } },
    batcher: { flushMain: vi.fn(), flushSubagent: vi.fn(), clear: vi.fn() },
    setCachedJsonlStatus: (value: unknown) => { cached = value },
    setSegmentIndex: vi.fn(), noteReplayCompleteForIdle: vi.fn(),
    clearJsonlCache: vi.fn(), resetTranscriptMirror: vi.fn(),
  } as unknown as HandlerCtx
})
const status = (data: Record<string, unknown>): void => handleServerMessage({ type: 'jsonl:status', status: 'watching', ...data } as any, ctx)
const forwarded = (): any => send.mock.calls.filter(([channel]) => channel === 'jsonl-status').at(-1)?.[2]

describe('INC-0067 replay status at the host boundary', () => {
  it('retains completed replay state across a live index update for later reveal', () => {
    status({ replayComplete: true })
    status({ segmentIndex: index })
    expect(cached.replayComplete).toBe(true)
    expect(forwarded().replayComplete).toBeUndefined()
    expect(send).toHaveBeenCalledWith('jsonl:boundaries-result', 'parent-a', index)
  })
  it('retains an unfinished replay during progress and index metadata', () => {
    status({ replayComplete: false })
    const start = cached
    status({ replayProgress: { sent: 2, total: 4 } })
    status({ segmentIndex: index })
    expect(cached).toBe(start)
    expect(cached.replayComplete).toBe(false)
    status({ replayComplete: true, segmentIndex: index })
    expect(cached.replayComplete).toBe(true)
  })
  it('normalizes a legacy bare watching start to explicit false', () => {
    status({})
    expect(forwarded().replayComplete).toBe(false)
    expect(cached.replayComplete).toBe(false)
  })
  it('normalizes and does not retain a one-shot compact event', () => {
    status({ compacted: true })
    expect(forwarded()).toMatchObject({ compacted: true, replayComplete: false })
    expect(cached.compacted).toBeUndefined()
    expect(ctx.clearJsonlCache).toHaveBeenCalledOnce()
    expect(ctx.resetTranscriptMirror).toHaveBeenCalledOnce()
  })
})

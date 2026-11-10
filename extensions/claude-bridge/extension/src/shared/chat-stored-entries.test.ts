// A reload replays only the newest REPLAY_TAIL_MAX cached rows. When those slots
// were counted over every cached row, a transcript ending in a long run of
// bookkeeping rows (cost-state, telemetry) filled the whole tail with rows the
// chat drops, and the reloaded chat came up empty.
import { describe, it, expect } from 'vitest'
import { chatReplayTail, isChatStoredEntry } from './chat-stored-entries'

const row = (type: string, pos: number) => ({ type, _pos: pos })

describe('chatReplayTail', () => {
  it('counts only rows the chat keeps toward the tail', () => {
    const cache = [row('user', 0), row('assistant', 1),
      ...Array.from({ length: 50 }, (_, i) => row(i % 2 ? 'cost-state' : 'atis-latch', 2 + i))]
    expect(chatReplayTail(cache, 10).map((e) => e.type)).toEqual(['user', 'assistant'])
  })

  it('takes the newest kept rows by file position when over the limit', () => {
    const cache = [row('user', 30), row('user', 10), row('mode', 20), row('cost-state', 40), row('assistant', 5)]
    expect(chatReplayTail(cache, 2).map((e) => e._pos)).toEqual([20, 30])
  })

  it('keeps delivery order when under the limit', () => {
    const cache = [row('user', 30), row('assistant', 10)]
    expect(chatReplayTail(cache, 10).map((e) => e._pos)).toEqual([30, 10])
  })

  it('keeps the read bookkeeping types and drops the rest', () => {
    for (const type of ['user', 'assistant', 'system', 'attachment', 'mode', 'permission-mode', 'queue-operation']) {
      expect(isChatStoredEntry({ type })).toBe(true)
    }
    for (const type of ['cost-state', 'atis-latch', 'tools-used', 'api-request', 'ai-title', 'file-history-snapshot']) {
      expect(isChatStoredEntry({ type })).toBe(false)
    }
    expect(isChatStoredEntry(null)).toBe(false)
  })
})

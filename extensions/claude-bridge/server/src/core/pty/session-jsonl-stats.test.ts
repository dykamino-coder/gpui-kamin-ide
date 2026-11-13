import { describe, expect, it } from 'vitest'
import type { JsonlEntry } from '../../shared/jsonl-types'
import { accumulateJsonlStats, resetJsonlStats } from './session-jsonl-stats'
import type { PtySession } from './types'

const codexItem = (uuid: string): JsonlEntry =>
  ({
    type: 'assistant',
    uuid,
    engine: 'codex',
    message: { id: uuid, role: 'assistant', content: [{ type: 'text', text: 'x' }] },
  }) as unknown as JsonlEntry

const codexUsage = (uuid: string, input: number, cached: number, output: number): JsonlEntry =>
  ({
    type: 'system',
    subtype: 'api_usage',
    uuid,
    engine: 'codex',
    usage: {
      input_tokens: input,
      cache_read_input_tokens: cached,
      cache_creation_input_tokens: 0,
      output_tokens: output,
    },
  }) as unknown as JsonlEntry

describe('live session counters of a mixed Claude + codex chat', () => {
  it('counts each codex usage line as one response and ignores its item lines', () => {
    const s = {} as PtySession
    resetJsonlStats(s)
    const entries: JsonlEntry[] = [
      { type: 'user', uuid: 'u1', message: { role: 'user', content: 'hi' } } as JsonlEntry,
      {
        type: 'assistant',
        uuid: 'a1',
        message: {
          id: 'msg-1',
          role: 'assistant',
          content: [],
          usage: { input_tokens: 10, output_tokens: 5, cache_read_input_tokens: 90, cache_creation_input_tokens: 0 },
        },
      } as unknown as JsonlEntry,
      {
        type: 'user',
        uuid: 'u2',
        engine: 'codex',
        message: { role: 'user', content: 'next' },
      } as unknown as JsonlEntry,
      codexItem('r1'),
      codexItem('m1'),
      codexUsage('usage:t:1', 300, 1700, 40),
      codexItem('m2'),
      codexUsage('usage:t:2', 100, 2000, 60),
    ]
    for (const e of entries) accumulateJsonlStats(s, e)
    expect(s.userMessages).toBe(2)
    expect(s.assistantMessages).toBe(3)
    // Context = the latest response's whole input; total = input + output summed.
    expect(s.contextTokens).toBe(2100)
    expect(s.totalTokens).toBe(105 + 2040 + 2160)
  })
})

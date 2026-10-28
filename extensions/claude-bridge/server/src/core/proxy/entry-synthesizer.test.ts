import { describe, expect, it } from 'vitest'
import { EntrySynthesizer } from './entry-synthesizer'
import type { AnthropicUsage } from './sse-parser'

describe('SSE usage assembly', () => {
  it('retains hourly writes when a later usage delta contains only 5m cache details', () => {
    let final: AnthropicUsage | undefined
    const synth = new EntrySynthesizer({
      ptySessionId: 'test',
      callbacks: {
        onUpdate: (e) => {
          if (!e.__streaming) final = e.message.usage
        },
      },
    })
    synth.feed([
      {
        type: 'message_start',
        message: {
          id: 'm',
          model: 'claude-haiku-5-5',
          role: 'assistant',
          usage: {
            input_tokens: 2,
            cache_creation_input_tokens: 100,
            cache_creation: { ephemeral_1h_input_tokens: 80 },
          },
        },
      },
      {
        type: 'message_delta',
        delta: { stop_reason: 'end_turn', stop_sequence: null },
        usage: { output_tokens: 4, cache_creation: { ephemeral_5m_input_tokens: 20 } },
      },
      { type: 'message_stop' },
    ])
    expect(final).toEqual({
      input_tokens: 2,
      output_tokens: 4,
      cache_creation_input_tokens: 100,
      cache_creation: { ephemeral_1h_input_tokens: 80, ephemeral_5m_input_tokens: 20 },
    })
  })
})

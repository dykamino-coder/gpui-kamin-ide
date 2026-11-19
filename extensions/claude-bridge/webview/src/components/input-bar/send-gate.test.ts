import { describe, expect, it } from 'vitest'
import { sendGate } from './send-gate'

describe('Send while the chat reconnects', () => {
  it('hands the message to the host while the chat is connecting (it is held for the chat there)', () => {
    expect(sendGate('connecting', false)).toBe('send')
    expect(sendGate('connected', false)).toBe('send')
  })

  it('keeps the text and says why when the chat is not connected', () => {
    expect(sendGate('disconnected', false)).toBe('not-connected')
    expect(sendGate('error', false)).toBe('not-connected')
  })

  it('waits while the view switches or downloads the transcript, whatever the connection', () => {
    for (const status of ['connected', 'connecting', 'disconnected']) expect(sendGate(status, true)).toBe('wait')
  })
})

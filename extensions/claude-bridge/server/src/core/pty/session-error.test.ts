import { describe, expect, it, vi } from 'vitest'
import { WebSocket as WS } from 'ws'

import { sendSessionError } from './session-error'

describe('fatal session error', () => {
  it('sends the reason before closing the session socket', () => {
    const calls: string[] = []
    const socket = {
      readyState: WS.OPEN,
      send: vi.fn((payload: string) => {
        calls.push(`send:${payload}`)
      }),
      close: vi.fn((code: number, reason: string) => {
        calls.push(`close:${code}:${reason}`)
      }),
    }

    sendSessionError(socket as Parameters<typeof sendSessionError>[0], 'Session not found', 4004, 'Session not found')

    expect(JSON.parse(calls[0]!.slice('send:'.length))).toEqual({
      type: 'session:error',
      error: 'Session not found',
      fatal: true,
    })
    expect(calls[1]).toBe('close:4004:Session not found')
  })

  it('does nothing after the socket has started closing', () => {
    const socket = { readyState: WS.CLOSING, send: vi.fn(), close: vi.fn() }
    sendSessionError(socket as Parameters<typeof sendSessionError>[0], 'late')
    expect(socket.send).not.toHaveBeenCalled()
    expect(socket.close).not.toHaveBeenCalled()
  })
})

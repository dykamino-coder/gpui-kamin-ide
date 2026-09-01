import { describe, expect, it, vi } from 'vitest'

import { toRendererConnectionState } from './connection-state'
import { handleServerMessage, type HandlerCtx } from './handle-server-message'

describe('renderer connection state', () => {
  it('maps only a server-authenticated session to input-ready connected', () => {
    expect(toRendererConnectionState({
      status: 'authenticated',
      sessionId: 'pty-1',
    }, 'authority-a', 3, 100, 7)).toEqual({
      status: 'connected',
      authority: 'authority-a',
      authorityGeneration: 3,
      authoritySequence: 100,
      revision: 7,
      sessionId: 'pty-1',
      error: undefined,
      nextRetryAt: undefined,
      retryAttempt: undefined,
    })
  })

  it('keeps an open websocket pre-authentication input-blocking', () => {
    expect(toRendererConnectionState({ status: 'connected' }, 'authority-a', 3, 100, 3).status).toBe('connecting')
  })

  it('copies retry metadata into the same atomic snapshot', () => {
    expect(toRendererConnectionState({
      status: 'connecting',
      error: 'Reconnecting',
      nextRetryAt: 1234,
      retryAttempt: 4,
    }, 'authority-b', 4, 200, 9)).toMatchObject({
      status: 'connecting',
      authority: 'authority-b',
      authorityGeneration: 4,
      authoritySequence: 200,
      revision: 9,
      error: 'Reconnecting',
      nextRetryAt: 1234,
      retryAttempt: 4,
    })
  })

  it('treats a server session error as a fatal connection failure', () => {
    const terminateSessionWithError = vi.fn()
    handleServerMessage(
      { type: 'session:error', error: 'Session not found' },
      { terminateSessionWithError } as unknown as HandlerCtx,
    )

    expect(terminateSessionWithError).toHaveBeenCalledOnce()
    expect(terminateSessionWithError).toHaveBeenCalledWith('Session not found')
  })
})

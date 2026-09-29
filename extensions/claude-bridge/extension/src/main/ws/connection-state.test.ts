import { describe, expect, it, vi } from 'vitest'

import { normalizeConnectionTransition } from '../../incident-diagnostics'
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

/** Диагностика классифицирует разрыв ровно по тому объекту, который отдал
 *  маппер: `recordBridgeOutbound` передаёт `args[1]` нормализатору без правок.
 *  Поэтому связка проверяется целиком, а не каждая функция по отдельности —
 *  по отдельности обе были зелёными и при потерянном коде (INC-2026-0027). */
function classifyThroughMapper(state: Parameters<typeof toRendererConnectionState>[0]): string {
  return normalizeConnectionTransition('tab-1', toRendererConnectionState(state, 'authority-a', 1, 1, 1)).cause
}

describe('INC-2026-0027: код закрытия доходит до диагностики', () => {
  it('обрыв без причины (1006) остаётся сетевым, а не неизвестным', () => {
    expect(classifyThroughMapper({ status: 'disconnected', closeCode: 1006 })).toBe('network')
  })

  it('штатное закрытие сервером (1000) остаётся удалённым закрытием', () => {
    expect(classifyThroughMapper({ status: 'disconnected', closeCode: 1000 })).toBe('remote-close')
  })

  it('текстовая причина классифицируется и без кода', () => {
    // Путь через `error` работал и до исправления: тест сторожит, что перенос
    // кода его не подменил собой.
    expect(classifyThroughMapper({ status: 'disconnected', error: 'Connection timed out' })).toBe('timeout')
  })

  it('намеренное отсоединение без кода остаётся неизвестным', () => {
    // `disconnect()` ставит состояние без кода: приписывать ему сетевой сбой
    // нельзя, иначе журнал наполнится ложными обрывами.
    expect(classifyThroughMapper({ status: 'disconnected' })).toBe('unknown')
  })

  it('код не подменяет собой причину живого соединения', () => {
    expect(classifyThroughMapper({ status: 'authenticated', sessionId: 'pty-1', closeCode: 1006 })).toBe('none')
  })
})

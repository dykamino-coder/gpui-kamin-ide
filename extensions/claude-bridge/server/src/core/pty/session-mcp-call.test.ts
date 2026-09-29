import { describe, expect, it, beforeEach } from 'vitest'

import { pendingMcpCalls, resendUndeliveredMcpCalls, sendMcpCall } from './session-mcp-call.js'
import type { PtySession } from './types.js'

/** Сокет, повторяющий поведение настоящего при переполнении буфера: он
 *  остаётся OPEN, но `sendToClient` отбрасывает кадр, потому что
 *  `bufferedAmount` выше 16 МиБ. Ровно этот случай и терялся. */
function socket(options: { open?: boolean; buffered?: number; throwOnSend?: boolean } = {}) {
  const frames: string[] = []
  return {
    frames,
    ws: {
      readyState: options.open === false ? 3 : 1,
      bufferedAmount: options.buffered ?? 0,
      send(payload: string) {
        if (options.throwOnSend) throw new Error('socket is racing shut')
        frames.push(payload)
      },
    },
  }
}

function session(ws: unknown): PtySession {
  return {
    id: 'session-under-test',
    ws,
    mcpCallCount: 0,
    inputCount: 0,
    mcpInitialized: true,
    mcpLastError: null,
    lastActivityAt: new Date(),
  } as unknown as PtySession
}

const OVER_CAP = 17 * 1024 * 1024

function pendingFor(sessionId: string) {
  return [...pendingMcpCalls.values()].filter((call) => call.sessionId === sessionId)
}

/** Единственный ожидаемый вызов сессии: падает с внятным текстом, если их не
 *  один, вместо `possibly undefined` у обращения по индексу. */
function onlyPending(sessionId: string) {
  const calls = pendingFor(sessionId)
  expect(calls).toHaveLength(1)
  return calls[0]!
}

describe('INC-2026-0011: состояние доставки интерактивного MCP-вызова', () => {
  beforeEach(() => {
    pendingMcpCalls.clear()
  })

  it('отброшенный по переполнению буфера вызов НЕ считается доставленным', () => {
    const live = socket({ buffered: OVER_CAP })
    const target = session(live.ws)

    void sendMcpCall(target, 'AskUserQuestion', { questions: [] })

    expect(live.frames).toHaveLength(0)
    expect(onlyPending(target.id).delivered).toBe(false)
  })

  it('успешная отправка помечается доставленной', () => {
    const live = socket()
    const target = session(live.ws)

    void sendMcpCall(target, 'AskUserQuestion', { questions: [] })

    expect(live.frames).toHaveLength(1)
    expect(onlyPending(target.id).delivered).toBe(true)
  })

  it('исключение при отправке тоже оставляет вызов недоставленным', () => {
    const live = socket({ throwOnSend: true })
    const target = session(live.ws)

    void sendMcpCall(target, 'AskUserQuestion', { questions: [] })

    expect(onlyPending(target.id).delivered).toBe(false)
  })

  it('после освобождения буфера повтор доходит и закрывает потерю', () => {
    // Полный сценарий инцидента: переполнение -> сброс -> повторное
    // подключение. До исправления повтор пропускал запрос, потому что тот
    // ошибочно числился доставленным.
    const stalled = socket({ buffered: OVER_CAP })
    const target = session(stalled.ws)
    void sendMcpCall(target, 'AskUserQuestion', { questions: [] })
    expect(stalled.frames).toHaveLength(0)

    const fresh = socket()
    const reattached = session(fresh.ws)
    Object.defineProperty(reattached, 'id', { value: target.id })

    resendUndeliveredMcpCalls(reattached)

    expect(fresh.frames).toHaveLength(1)
    expect(onlyPending(target.id).delivered).toBe(true)
  })

  it('повтор в переполненный сокет не помечает вызов доставленным', () => {
    const stalled = socket({ buffered: OVER_CAP })
    const target = session(stalled.ws)
    void sendMcpCall(target, 'AskUserQuestion', { questions: [] })

    const stillStalled = socket({ buffered: OVER_CAP })
    const reattached = session(stillStalled.ws)
    Object.defineProperty(reattached, 'id', { value: target.id })

    resendUndeliveredMcpCalls(reattached)

    expect(stillStalled.frames).toHaveLength(0)
    expect(onlyPending(target.id).delivered).toBe(false)
  })
})

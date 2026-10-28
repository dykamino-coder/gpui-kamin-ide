import { describe, expect, it, beforeEach, afterEach, vi } from 'vitest'

import {
  pendingMcpCalls,
  resendUndeliveredMcpCalls,
  sendMcpCall,
  rejectAllPending,
  rejectPendingForSession,
  handleMcpResponse,
} from './session-mcp-call.js'
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
    vi.useFakeTimers()
  })

  afterEach(() => {
    rejectAllPending('test cleanup')
    vi.useRealTimers()
  })

  it('отброшенный по переполнению буфера вызов НЕ считается доставленным', () => {
    const live = socket({ buffered: OVER_CAP })
    const target = session(live.ws)

    void sendMcpCall(target, 'AskUserQuestion', { questions: [] }).catch(() => {})

    expect(live.frames).toHaveLength(0)
    expect(onlyPending(target.id).delivered).toBe(false)
  })

  it('успешная отправка помечается доставленной', () => {
    const live = socket()
    const target = session(live.ws)

    void sendMcpCall(target, 'AskUserQuestion', { questions: [] }).catch(() => {})

    expect(live.frames).toHaveLength(1)
    expect(onlyPending(target.id).delivered).toBe(true)
  })

  it('исключение при отправке тоже оставляет вызов недоставленным', () => {
    const live = socket({ throwOnSend: true })
    const target = session(live.ws)

    void sendMcpCall(target, 'AskUserQuestion', { questions: [] }).catch(() => {})

    expect(onlyPending(target.id).delivered).toBe(false)
  })

  it('после освобождения буфера повтор доходит и закрывает потерю', () => {
    // Полный сценарий инцидента: переполнение -> сброс -> повторное
    // подключение. До исправления повтор пропускал запрос, потому что тот
    // ошибочно числился доставленным.
    const stalled = socket({ buffered: OVER_CAP })
    const target = session(stalled.ws)
    void sendMcpCall(target, 'AskUserQuestion', { questions: [] }).catch(() => {})
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
    void sendMcpCall(target, 'AskUserQuestion', { questions: [] }).catch(() => {})

    const stillStalled = socket({ buffered: OVER_CAP })
    const reattached = session(stillStalled.ws)
    Object.defineProperty(reattached, 'id', { value: target.id })

    resendUndeliveredMcpCalls(reattached)

    expect(stillStalled.frames).toHaveLength(0)
    expect(onlyPending(target.id).delivered).toBe(false)
  })
})

describe('INC-2026-0080 request backpressure', () => {
  beforeEach(() => vi.useFakeTimers())
  afterEach(() => {
    rejectAllPending('test cleanup')
    vi.useRealTimers()
  })

  it('retries after buffer drain without a reconnect, then stops after enqueue', async () => {
    const live = socket({ buffered: OVER_CAP })
    const target = session(live.ws)
    const response = sendMcpCall(target, 'Read', { file_path: 'synthetic' })
    expect(live.frames).toHaveLength(0)
    live.ws.bufferedAmount = 0
    vi.advanceTimersByTime(1000)
    expect(live.frames).toHaveLength(1)
    const id = JSON.parse(live.frames[0]!).requestId
    vi.advanceTimersByTime(3000)
    expect(live.frames).toHaveLength(1)
    handleMcpResponse(id, 'done')
    await expect(response).resolves.toBe('done')
    expect(vi.getTimerCount()).toBe(0)
  })

  it('rejects a permanently oversized request instead of waiting for a tool timeout', async () => {
    const response = sendMcpCall(session(socket().ws), 'Write', { content: 'x'.repeat(OVER_CAP) })
    await expect(response).rejects.toThrow('frame limit')
    expect(pendingMcpCalls.size).toBe(0)
    expect(vi.getTimerCount()).toBe(0)
  })
})

describe('MCP retry ownership cleanup', () => {
  beforeEach(() => vi.useFakeTimers())
  afterEach(() => {
    rejectAllPending('test cleanup')
    vi.useRealTimers()
  })

  it('clears an undelivered interactive retry when its owning session ends', async () => {
    const live = socket({ buffered: OVER_CAP })
    const target = session(live.ws)
    const response = sendMcpCall(target, 'AskUserQuestion', {})
    const failure = expect(response).rejects.toThrow('session ended')
    rejectPendingForSession(target.id, 'session ended')
    await failure
    vi.advanceTimersByTime(3000)
    expect(live.frames).toHaveLength(0)
    expect(pendingMcpCalls.size).toBe(0)
    expect(vi.getTimerCount()).toBe(0)
  })

  it('clears transport retries when the owning request deadline expires', async () => {
    const live = socket({ buffered: OVER_CAP })
    const response = sendMcpCall(session(live.ws), 'synthetic-default-deadline', {})
    const failure = expect(response).rejects.toThrow('MCP call timeout')
    await vi.advanceTimersByTimeAsync(120_000)
    await failure
    expect(live.frames).toHaveLength(0)
    expect(pendingMcpCalls.size).toBe(0)
    expect(vi.getTimerCount()).toBe(0)
  })
})

// Тянет за собой потоковый прокси → @peculiar/x509 → tsyringe, которому нужен
// полифил reflect до декораторов. Тот же импорт, что и у точки входа.
import 'reflect-metadata'

import { describe, expect, it } from 'vitest'

import { waitForDrain, type DrainableSocket } from './download-backpressure'

/**
 * INC-2026-0019: выгрузка расшифровки шла без обратного давления.
 *
 * Куски уходили подряд через сырой `ws.send` — без ожидания опустошения
 * буфера, без проверки `bufferedAmount` и без отмены при разрыве. Соединение
 * уже могло стоять выше предполагаемого предела исходящего буфера, и тогда
 * выгрузка копилась поверх обычного трафика сессии.
 *
 * Время и пауза подменяются: иначе проверка ждала бы настоящую минуту.
 */

const OPEN = 1
const CLOSED = 3

/** Сокет, чьё состояние задаёт тест, а буфер опустошается по расписанию. */
function socket(script: { buffered: number[]; readyStates?: number[] }): DrainableSocket & { reads: number } {
  const state = { reads: 0 }
  return {
    get reads() {
      return state.reads
    },
    get readyState() {
      const seq = script.readyStates
      return seq === undefined ? OPEN : (seq[Math.min(state.reads, seq.length - 1)] ?? OPEN)
    },
    get bufferedAmount() {
      const v = script.buffered[Math.min(state.reads, script.buffered.length - 1)] ?? 0
      state.reads += 1
      return v
    },
  }
}

/** Часы и пауза, которые двигает сам тест. */
function clock(): { opts: { now: () => number; sleep: (ms: number) => Promise<void> }; elapsed: () => number } {
  let t = 0
  return {
    opts: {
      now: () => t,
      sleep: (ms: number) => {
        t += ms
        return Promise.resolve()
      },
    },
    elapsed: () => t,
  }
}

const LIMIT = 4 * 1024 * 1024

describe('INC-2026-0019: выгрузка ждёт места в буфере', () => {
  it('свободный буфер пропускает кусок сразу', async () => {
    const c = clock()
    const ws = socket({ buffered: [0] })

    await expect(waitForDrain(ws, { limit: LIMIT, pollMs: 25, timeoutMs: 60_000, ...c.opts })).resolves.toBe('ready')
    expect(c.elapsed()).toBe(0)
  })

  it('переполненный буфер задерживает кусок, пока не опустеет', async () => {
    // Здесь и был дефект: кусок уходил в буфер, уже стоящий выше предела.
    const c = clock()
    const ws = socket({ buffered: [LIMIT * 4, LIMIT * 2, 0] })

    await expect(waitForDrain(ws, { limit: LIMIT, pollMs: 25, timeoutMs: 60_000, ...c.opts })).resolves.toBe('ready')
    expect(c.elapsed()).toBe(50)
  })

  it('разрыв соединения прекращает выгрузку, а не досылает в никуда', async () => {
    const c = clock()
    const ws = socket({ buffered: [LIMIT * 4, LIMIT * 4], readyStates: [OPEN, CLOSED] })

    await expect(waitForDrain(ws, { limit: LIMIT, pollMs: 25, timeoutMs: 60_000, ...c.opts })).resolves.toBe('closed')
  })

  it('буфер, который не опустел, признаётся мёртвым по сроку', async () => {
    // Молча досылать в такое соединение нельзя: файл у пользователя всё равно
    // будет неполон, а сервер продолжит копить работу.
    const c = clock()
    const ws = socket({ buffered: [LIMIT * 4] })

    await expect(waitForDrain(ws, { limit: LIMIT, pollMs: 25, timeoutMs: 100, ...c.opts })).resolves.toBe('timeout')
    expect(c.elapsed()).toBeGreaterThanOrEqual(100)
  })

  it('ожидание уступает управление между кусками', async () => {
    // Доказательство карточки: все отправки происходили ДО микрозадачи,
    // запланированной первой из них, — то есть цикл не уступал вовсе.
    const order: string[] = []
    const ws = socket({ buffered: [0] })
    const waiting = waitForDrain(ws, { limit: LIMIT, pollMs: 25, timeoutMs: 60_000 }).then(() => {
      order.push('после ожидания')
    })
    void Promise.resolve().then(() => {
      order.push('микрозадача')
    })

    await waiting

    expect(order).toEqual(['микрозадача', 'после ожидания'])
  })
})

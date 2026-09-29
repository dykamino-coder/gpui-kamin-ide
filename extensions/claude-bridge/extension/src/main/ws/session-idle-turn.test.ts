import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import { SessionIdleTracker } from './session-idle-tracker'

/**
 * BR-23: «Session finished» повторялся несколько раз за один виток.
 *
 * Сервер публикует и авторитетные состояния из хуков жизненного цикла
 * (`UserPromptSubmit`/`Stop`), и эвристические состояния из заголовка OSC.
 * В трекер они шли неразличимо, а границ витка он не знает: после каждого
 * «работает → простаивает» тост разрешался заново. Мигание OSC посреди
 * оркестрации Agent Teams успевало породить несколько уведомлений до
 * ЕДИНСТВЕННОГО `Stop` главного витка.
 *
 * Время — фальшивое: настоящий debounce в 500 мс сделал бы набор медленным и
 * плавающим.
 */
const DEBOUNCE_MS = 500
const OSC_CONFIRM_MS = 4_000

function tracker(): { fired: string[]; track: SessionIdleTracker['track']; instance: SessionIdleTracker } {
  const fired: string[] = []
  const instance = new SessionIdleTracker((rawTitle) => fired.push(rawTitle))
  // `armSettle` здесь не зовётся: окно успокоения после подключения — предмет
  // другого теста, а этот про границы витка.
  return { fired, track: instance.track.bind(instance), instance }
}

/** Простой обязан ещё и отстояться: debounce гасит короткое «нырнул и вернулся». */
function settleIdle(): void {
  vi.advanceTimersByTime(DEBOUNCE_MS + 1)
}

/** Подтверждение эвристического простоя на сервере с хуками. */
function confirmOscIdle(): void {
  vi.advanceTimersByTime(OSC_CONFIRM_MS + 1)
}

beforeEach(() => {
  vi.useFakeTimers()
  vi.setSystemTime(new Date('2026-09-29T10:00:00.000Z'))
})

afterEach(() => {
  vi.useRealTimers()
})

describe('BR-23: одно уведомление о завершении на виток', () => {
  it('мигания OSC внутри витка не дают лишних уведомлений', () => {
    const t = tracker()

    t.track('prompt', true, true) // UserPromptSubmit — виток открыт хуком
    for (let i = 0; i < 3; i++) {
      // Оркестрация Agent Teams: заголовок то гаснет, то возвращается.
      t.track('idle-blip', false, false)
      settleIdle()
      t.track('working-again', true, false)
    }
    expect(t.fired).toHaveLength(0)

    t.track('done', false, true) // Stop главного витка
    settleIdle()

    expect(t.fired).toEqual(['done'])
  })

  it('два витка подряд дают по одному уведомлению', () => {
    const t = tracker()

    t.track('p1', true, true)
    t.track('s1', false, true)
    settleIdle()
    t.track('p2', true, true)
    t.track('s2', false, true)
    settleIdle()

    expect(t.fired).toEqual(['s1', 's2'])
  })

  it('повторный Stop того же витка второго уведомления не даёт', () => {
    const t = tracker()

    t.track('p1', true, true)
    t.track('s1', false, true)
    settleIdle()
    t.track('s1-again', false, true)
    settleIdle()

    expect(t.fired).toEqual(['s1'])
  })

  it('сервер без хуков работает по-прежнему — на эвристике заголовка', () => {
    // Запасной путь обязан остаться: иначе на сервере старого образца
    // уведомление о завершении пропало бы совсем.
    const t = tracker()

    t.track('working', true, false)
    t.track('idle', false, false)
    settleIdle()

    expect(t.fired).toEqual(['idle'])
  })

  it('эвристический простой до первого хука виток закрывает сразу', () => {
    // Пока сервер не показал, что умеет хуки, единственный источник — OSC.
    const t = tracker()

    t.track('working', true, false)
    t.track('idle', false, false)
    settleIdle()

    expect(t.fired).toEqual(['idle'])
  })

  it('эвристическое завершение витка уведомляет и на сервере с хуками', () => {
    // РЕГРЕССИЯ 1.0.61: прежнее правило «раз есть хуки — закрывает только хук»
    // глушило уведомление НАСОВСЕМ там, где хук завершения витка не приходит.
    // Проверено A/B на живом приложении: на 1.0.60 тост есть, на 1.0.61 нет.
    const t = tracker()

    t.track('start', false, true) // SessionStart — хук виден, витка ещё нет
    t.track('working', true, false)
    t.track('done', false, false)
    confirmOscIdle()

    expect(t.fired).toEqual(['done'])
  })

  it('мигание, вернувшееся в работу раньше подтверждения, тоста не даёт', () => {
    // Ровно то, ради чего правило и заводилось: внутри витка заголовок гаснет
    // и возвращается за доли секунды, до порога подтверждения не доживая.
    const t = tracker()

    t.track('p', true, true)
    t.track('blip', false, false)
    vi.advanceTimersByTime(OSC_CONFIRM_MS - 500)
    t.track('back', true, false)
    confirmOscIdle()

    expect(t.fired).toEqual([])
  })

  it('закрытие вкладки снимает висящий таймер', () => {
    const t = tracker()

    t.track('working', true, false)
    t.track('idle', false, false)
    t.instance.dispose()
    settleIdle()

    expect(t.fired).toEqual([])
  })
})

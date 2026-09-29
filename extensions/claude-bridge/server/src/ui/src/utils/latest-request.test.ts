import { describe, expect, it } from 'vitest'

import { createRequestOwner } from './latest-request'

/**
 * INC-2026-0015: график публиковал КАЖДЫЙ ответ безусловно.
 *
 * Быстрое переключение периода давало ярлык «Yearly» над более старым дневным
 * ответом и погасший индикатор загрузки при ещё идущем запросе: порядок ответов
 * сети не совпадает с порядком запросов.
 */

/** Ответ, который разрешают вручную — так порядок задаётся тестом, а не сетью. */
function deferred<T>(): { promise: Promise<T>; resolve: (v: T) => void } {
  let resolve!: (v: T) => void
  const promise = new Promise<T>((r) => {
    resolve = r
  })
  return { promise, resolve }
}

describe('INC-2026-0015: публикует только последний запрос', () => {
  it('метка обогнавшего прежнего запроса больше не текущая', () => {
    const owner = createRequestOwner()

    const daily = owner.begin()
    const yearly = owner.begin()

    expect(owner.isCurrent(daily)).toBe(false)
    expect(owner.isCurrent(yearly)).toBe(true)
  })

  it('единственный запрос остаётся текущим', () => {
    const owner = createRequestOwner()

    expect(owner.isCurrent(owner.begin())).toBe(true)
  })

  it('чужая метка текущей не считается', () => {
    const owner = createRequestOwner()
    owner.begin()

    expect(owner.isCurrent(0)).toBe(false)
    expect(owner.isCurrent(99)).toBe(false)
  })

  it('ответ, пришедший ПОЗЖЕ, но запрошенный РАНЬШЕ, не публикуется', async () => {
    // Полевой порядок карточки: сначала разрешается Yearly, затем более ранний
    // Daily. Без владения запросом победил бы Daily — ярлык остался бы Yearly,
    // а данные стали бы почасовыми.
    const owner = createRequestOwner()
    const published: string[] = []
    const run = async (name: string, answer: Promise<string>): Promise<void> => {
      const token = owner.begin()
      const value = await answer
      if (!owner.isCurrent(token)) return
      published.push(`${name}:${value}`)
    }

    const daily = deferred<string>()
    const yearly = deferred<string>()
    const first = run('daily', daily.promise)
    const second = run('yearly', yearly.promise)
    yearly.resolve('месяцы')
    daily.resolve('часы')
    await Promise.all([first, second])

    expect(published).toEqual(['yearly:месяцы'])
  })

  it('индикатор загрузки гасит тоже только последний запрос', async () => {
    // Иначе ответ обогнавшего прежнего запроса объявлял бы загрузку
    // законченной, пока текущий ещё идёт.
    const owner = createRequestOwner()
    let loading = false
    const run = async (answer: Promise<string>): Promise<void> => {
      const token = owner.begin()
      loading = true
      await answer
      if (owner.isCurrent(token)) loading = false
    }

    const stale = deferred<string>()
    const fresh = deferred<string>()
    const a = run(stale.promise)
    const b = run(fresh.promise)
    stale.resolve('старый')
    await a

    expect(loading).toBe(true)

    fresh.resolve('свежий')
    await b

    expect(loading).toBe(false)
  })
})

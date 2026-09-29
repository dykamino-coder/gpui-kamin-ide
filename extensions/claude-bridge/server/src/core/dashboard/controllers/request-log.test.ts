// Тянет за собой потоковый прокси → @peculiar/x509 → tsyringe, которому нужен
// полифил reflect до декораторов. Тот же импорт, что и у точки входа.
import 'reflect-metadata'

import { describe, expect, it } from 'vitest'

import { parseDeleteFilter, parseListQuery, statusOf, toRequestLogEntry } from './request-log'

/**
 * INC-2026-0016: API истории запросов игнорировал страницы и фильтры,
 * объявлял успехом событие с ошибкой и удалял не из того набора, что читал.
 *
 * Худшее следствие — удаление с фильтром «пользователь + ошибки» стирало ВСЮ
 * историю и метрики пользователя. Молчаливо проигнорированный фильтр в запросе
 * на удаление и есть потеря данных, поэтому непосильный фильтр обязан быть
 * отвергнут, а не исполнен наполовину.
 */

describe('INC-2026-0016: разбор запроса списка', () => {
  it('смещение доезжает до выборки, а не теряется', () => {
    const parsed = parseListQuery({ limit: '10', offset: '20' })

    expect(parsed).toEqual({ ok: true, value: { limit: 10, offset: 20 } })
  })

  it('сужение по пользователю доезжает', () => {
    const parsed = parseListQuery({ userName: 'пользователь-1' })

    expect(parsed.ok && parsed.value.userName).toBe('пользователь-1')
  })

  it('фильтр по статусу отвергается, а не игнорируется молча', () => {
    const parsed = parseListQuery({ status: 'error' })

    expect(parsed.ok).toBe(false)
    expect(!parsed.ok && parsed.error).toContain('status filtering is not supported')
  })

  it('незнакомая конечная точка отвергается', () => {
    const parsed = parseListQuery({ endpoint: 'openai' })

    expect(parsed.ok).toBe(false)
    expect(!parsed.ok && parsed.error).toContain('openai')
  })

  it('единственная записываемая конечная точка принимается', () => {
    expect(parseListQuery({ endpoint: 'anthropic' }).ok).toBe(true)
  })

  it('мусор в пределах и смещении отвергается', () => {
    expect(parseListQuery({ limit: 'сто' }).ok).toBe(false)
    expect(parseListQuery({ offset: '-1' }).ok).toBe(false)
    expect(parseListQuery({ limit: '100000' }).ok).toBe(false)
  })
})

describe('INC-2026-0016: разбор фильтра удаления', () => {
  it('фильтр по статусу отвергается — иначе он стирает лишнее', () => {
    // Ровно полевой случай карточки: «пользователь + ошибки» сужало только
    // устаревшую таблицу, а затем стирало ВСЕ события и метрики пользователя.
    const parsed = parseDeleteFilter({ userName: 'пользователь-1', status: 'error' })

    expect(parsed.ok).toBe(false)
  })

  it('удаление без имени пользователя отвергается', () => {
    // Фильтр, который ничего не сужает, — это запрос «удали всё» под видом
    // выборочного удаления.
    expect(parseDeleteFilter({}).ok).toBe(false)
    expect(parseDeleteFilter({ endpoint: 'anthropic' }).ok).toBe(false)
  })

  it('удаление по одному лишь пользователю принимается', () => {
    const parsed = parseDeleteFilter({ userName: 'пользователь-1' })

    expect(parsed).toEqual({ ok: true, value: { userName: 'пользователь-1' } })
  })
})

describe('INC-2026-0016: статус берётся из события', () => {
  it('событие с ошибкой не объявляется успехом', () => {
    expect(statusOf({ data: { error: 'upstream refused' } })).toEqual({ status: 'error', statusCode: 500 })
  })

  it('код ответа из события сохраняется', () => {
    expect(statusOf({ data: { error: 'rate limited', statusCode: 429 } })).toEqual({
      status: 'error',
      statusCode: 429,
    })
  })

  it('пустая строка ошибкой не считается', () => {
    expect(statusOf({ data: { error: '' } })).toEqual({ status: 'success', statusCode: 200 })
  })

  it('список и подробность отвечают одинаково', () => {
    // Раньше это были две отдельные копии отображения, и они разошлись.
    const ev = { id: 7, data: { error: 'boom', model: 'claude-opus-5-5' }, timestamp: 'сейчас' }

    expect(toRequestLogEntry(ev)).toMatchObject({ id: '7', status: 'error', statusCode: 500, error: 'boom' })
  })
})

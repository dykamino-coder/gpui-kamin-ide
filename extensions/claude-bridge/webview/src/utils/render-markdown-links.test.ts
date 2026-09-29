import { describe, expect, it } from 'vitest'

import { renderMarkdown } from './render-markdown'

/**
 * INC-2026-0051: ссылка из чата открывалась с `%3C/strong%3E` в хвосте.
 *
 * Ссылки создавались ДО замен жирного и наклонного начертания, а те шли по
 * всему HTML — включая значения атрибутов. Закрывающий тег попадал прямо в
 * `href`, и обработчик клика читал его вместе с адресом. Плюс образец голого
 * адреса проглатывал хвостовые знаки эмфазы и уносил их в href и подпись.
 */

/** Значение `href` первой ссылки — именно его читает обработчик клика. */
function hrefOf(html: string): string | null {
  return /href="([^"]*)"/.exec(html)?.[1] ?? null
}

describe('INC-2026-0051: разметка не протекает в адрес ссылки', () => {
  it('жирный голый адрес не уносит тег в href', () => {
    const html = renderMarkdown('**https://example.com/a**')

    expect(hrefOf(html)).toBe('https://example.com/a')
    expect(html).not.toContain('/strong>"')
  })

  it('наклонный голый адрес тоже', () => {
    const html = renderMarkdown('см. *https://example.com/b* тут')

    expect(hrefOf(html)).toBe('https://example.com/b')
  })

  it('жирное начертание вокруг ссылки продолжает работать', () => {
    // Сторож: теги СКРЫВАЮТСЯ на время замен, а не разбивают строку — иначе
    // эмфаза вокруг готовой ссылки перестала бы применяться.
    const html = renderMarkdown('**https://example.com/a**')

    expect(html).toContain('<strong><a')
    expect(html).toContain('</a></strong>')
  })

  it('ссылка в разметке с обрамлением не ломается', () => {
    const html = renderMarkdown('**[метка](https://example.com/c)**')

    expect(hrefOf(html)).toBe('https://example.com/c')
    expect(html).toContain('>метка</a>')
  })

  it('точка в конце предложения остаётся в тексте, а не пропадает', () => {
    // Хвост отрезался от адреса и исчезал совсем: предложение теряло точку.
    const html = renderMarkdown('см. https://example.com/b.')

    expect(hrefOf(html)).toBe('https://example.com/b')
    expect(html).toContain('</a>.')
  })

  it('адрес без обрамления не меняется', () => {
    const html = renderMarkdown('https://example.com/plain?a=1&b=2')

    expect(hrefOf(html)).toBe('https://example.com/plain?a=1&amp;b=2')
  })
})

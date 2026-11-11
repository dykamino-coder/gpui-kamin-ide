import { describe, expect, it } from 'vitest'
import { olderPageIsStale, viewportUnfilled } from './fill-viewport'

describe('viewportUnfilled (INC-2026-0104)', () => {
  it('a window shorter than the viewport must page without a scroll gesture', () => {
    expect(viewportUnfilled(900, 900, 400)).toBe(true) // flex container: no overflow at all
  })
  it('an overflow inside the near-top zone is still unfilled: scrolling it cannot leave the bottom band', () => {
    expect(viewportUnfilled(1300, 900, 400)).toBe(true)
  })
  it('a window that overflows past the near-top zone is left to the scroll path', () => {
    expect(viewportUnfilled(1301, 900, 400)).toBe(false)
  })
  it('a panel that is not laid out gives no evidence', () => {
    expect(viewportUnfilled(0, 0, 400)).toBe(false)
  })
})

describe('olderPageIsStale', () => {
  it('drops a scroll-up page once the reader is back at the bottom', () => {
    expect(olderPageIsStale(true, false)).toBe(true)
  })
  it('keeps a page requested to fill the viewport, which is always at the bottom', () => {
    expect(olderPageIsStale(true, true)).toBe(false)
  })
  it('keeps a page for a reader who is still scrolled up', () => {
    expect(olderPageIsStale(false, false)).toBe(false)
  })
})

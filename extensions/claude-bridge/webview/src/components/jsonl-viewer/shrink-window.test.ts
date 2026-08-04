import { describe, it, expect } from 'vitest'
import { shouldShrinkWindow } from './shrink-window'

const opts = (o: Partial<Parameters<typeof shouldShrinkWindow>[0]> = {}) =>
  shouldShrinkWindow({ atBottom: true, cap: 950, initialCap: 150, loadingMore: false, ...o })

describe('render-window shrink', () => {
  it('gives the window back when the reader is at the bottom of a grown window', () => {
    expect(opts()).toBe(true)
  })

  it('never shrinks while the reader is scrolled up — that would yank them', () => {
    expect(opts({ atBottom: false })).toBe(false)
  })

  it('never shrinks while a scroll-up load is still settling', () => {
    // The load raises the cap and the layout has not landed yet; shrinking here
    // would undo the very load that is in flight.
    expect(opts({ loadingMore: true })).toBe(false)
  })

  it('does not re-render a window that was never grown', () => {
    expect(opts({ cap: 150 })).toBe(false)
  })

  it('does not act on a cap below the initial one', () => {
    expect(opts({ cap: 100 })).toBe(false)
  })

  it('shrinks after a single growth step, not only after several', () => {
    expect(opts({ cap: 550 })).toBe(true)
  })
})

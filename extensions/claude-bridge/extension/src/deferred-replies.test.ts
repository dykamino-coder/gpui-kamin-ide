import { describe, expect, it, vi } from 'vitest'
import { DeferredReplies, MAX_DEFERRED_REPLIES_PER_VIEW } from './deferred-replies'

const entry = (id: number) => ({ frame: { kind: 'invoke-reply', id }, flushed: vi.fn() })

describe('DeferredReplies', () => {
  it('returns held replies oldest first and empties the view', () => {
    const q = new DeferredReplies<object>()
    const view = {}
    q.defer(view, entry(1)); q.defer(view, entry(2))
    expect(q.take(view).map(e => e.frame.id)).toEqual([1, 2])
    expect(q.size(view)).toBe(0)
  })
  it('keeps views apart and forgets a torn-down view', () => {
    const q = new DeferredReplies<object>()
    const a = {}; const b = {}
    q.defer(a, entry(1)); q.defer(b, entry(2))
    q.forget(a)
    expect(q.size(a)).toBe(0)
    expect(q.take(b).map(e => e.frame.id)).toEqual([2])
  })
  it('puts undelivered replies back in front of newer ones', () => {
    const q = new DeferredReplies<object>()
    const view = {}
    q.defer(view, entry(3))
    q.restore(view, [entry(1), entry(2)])
    expect(q.take(view).map(e => e.frame.id)).toEqual([1, 2, 3])
  })
  it('evicts the oldest reply over the cap and hands it back', () => {
    const q = new DeferredReplies<object>()
    const view = {}
    let evicted
    for (let i = 0; i <= MAX_DEFERRED_REPLIES_PER_VIEW; i++) evicted = q.defer(view, entry(i)) ?? evicted
    expect(evicted?.frame.id).toBe(0)
    expect(q.size(view)).toBe(MAX_DEFERRED_REPLIES_PER_VIEW)
  })
})

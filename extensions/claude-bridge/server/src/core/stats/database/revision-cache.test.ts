import { describe, expect, it } from 'vitest'
import { createRevisionCache } from './revision-cache'

const later = <T>(ms: number, v: T) => new Promise<T>((r) => setTimeout(() => r(v), ms))

describe('revision cache', () => {
  it('returns the same answer for an unchanged revision without recomputing', async () => {
    const cache = createRevisionCache<number>({ max: 4, maxStaleMs: 60_000 })
    let calls = 0
    const compute = async () => ++calls
    expect((await cache.get('k', 'r1', compute)).data).toBe(1)
    expect((await cache.get('k', 'r1', compute)).data).toBe(1)
    expect(calls).toBe(1)
  })

  it('serves a fresh answer when the recompute fits the wait budget', async () => {
    const cache = createRevisionCache<string>({ max: 4, maxStaleMs: 60_000, freshWaitMs: 200 })
    await cache.get('k', 'r1', async () => 'old')
    const hit = await cache.get('k', 'r2', () => later(5, 'new'))
    expect(hit).toMatchObject({ revision: 'r2', data: 'new' })
  })

  it('serves the stale answer when the recompute is slow, then the fresh one', async () => {
    const cache = createRevisionCache<string>({ max: 4, maxStaleMs: 60_000, freshWaitMs: 10 })
    await cache.get('k', 'r1', async () => 'old')
    let calls = 0
    const slow = () => (calls++, later(60, 'new'))
    expect(await cache.get('k', 'r2', slow)).toMatchObject({ revision: 'r1', data: 'old' })
    // A second poll during the refresh shares it instead of starting another.
    expect((await cache.get('k', 'r2', slow)).data).toBe('old')
    await later(80, null)
    expect(await cache.get('k', 'r2', slow)).toMatchObject({ revision: 'r2', data: 'new' })
    expect(calls).toBe(1)
  })

  it('waits for the fresh answer once the cached one is too old', async () => {
    const cache = createRevisionCache<string>({ max: 4, maxStaleMs: 0, freshWaitMs: 1 })
    await cache.get('k', 'r1', async () => 'old')
    expect((await cache.get('k', 'r2', () => later(30, 'new'))).data).toBe('new')
  })

  it('keeps the stale answer when the background refresh fails', async () => {
    const cache = createRevisionCache<string>({ max: 4, maxStaleMs: 60_000, freshWaitMs: 50 })
    await cache.get('k', 'r1', async () => 'old')
    expect((await cache.get('k', 'r2', async () => Promise.reject(new Error('boom')))).data).toBe('old')
  })
})

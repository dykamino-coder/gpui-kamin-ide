import { describe, expect, it, vi } from 'vitest'
import { mergeLoopbackNoProxy } from '../config/no-proxy'
import { EventStream } from './event-stream'

describe('EventStream', () => {
  it('holds events until the first subscriber, then fans out live', () => {
    const s = new EventStream<number>()
    s.emit(1)
    s.emit(2)
    const a: number[] = []
    const b: number[] = []
    s.subscribe((e) => a.push(e))
    s.subscribe((e) => b.push(e))
    s.emit(3)
    expect(a).toEqual([1, 2, 3])
    expect(b).toEqual([3])
  })

  it('bounds the held backlog', () => {
    const s = new EventStream<number>(undefined, 2)
    for (let i = 0; i < 5; i++) s.emit(i)
    const got: number[] = []
    s.subscribe((e) => got.push(e))
    expect(got).toEqual([0, 1])
  })

  it('isolates listener errors and supports unsubscribe', () => {
    const onError = vi.fn()
    const s = new EventStream<string>(onError)
    const got: string[] = []
    s.subscribe(() => {
      throw new Error('boom')
    })
    const off = s.subscribe((e) => got.push(e))
    s.emit('a')
    off()
    s.emit('b')
    expect(got).toEqual(['a'])
    expect(onError).toHaveBeenCalledTimes(2)
  })

  it('close emits the last event once and drops later ones', () => {
    const s = new EventStream<string>()
    const got: string[] = []
    s.subscribe((e) => got.push(e))
    s.close('end')
    s.close('again')
    s.emit('late')
    expect(got).toEqual(['end'])
    expect(s.closed).toBe(true)
  })

  it('a subscriber arriving after close still receives what was held', () => {
    const s = new EventStream<string>()
    s.emit('started')
    s.close('exited')
    const got: string[] = []
    s.subscribe((e) => got.push(e))
    expect(got).toEqual(['started', 'exited'])
  })
})

describe('mergeLoopbackNoProxy', () => {
  it('keeps existing entries first, adds extra and loopback, dedupes', () => {
    const env: Record<string, string | undefined> = { NO_PROXY: 'corp.local, localhost', no_proxy: 'other' }
    mergeLoopbackNoProxy(env, 'bridge.internal')
    expect(env.NO_PROXY).toBe('bridge.internal,corp.local,localhost,other,127.0.0.1,::1')
    expect(env.no_proxy).toBe(env.NO_PROXY)
  })

  it('works on an empty environment', () => {
    const env: Record<string, string | undefined> = {}
    mergeLoopbackNoProxy(env)
    expect(env).toEqual({ NO_PROXY: '127.0.0.1,localhost,::1', no_proxy: '127.0.0.1,localhost,::1' })
  })
})

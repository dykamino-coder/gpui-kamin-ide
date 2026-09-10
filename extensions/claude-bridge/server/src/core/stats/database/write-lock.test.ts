import { describe, it, expect } from 'vitest'
import { withStatsWrite, statsWriteDepth } from './write-lock'

const tick = () => new Promise<void>(resolve => setTimeout(resolve, 0))

describe('withStatsWrite', () => {
  it('runs writers strictly one after another, in submission order', async () => {
    const log: string[] = []
    let release!: () => void
    const gate = new Promise<void>(resolve => { release = resolve })

    const first = withStatsWrite(async () => {
      log.push('first:start')
      await gate
      log.push('first:end')
    })
    const second = withStatsWrite(async () => {
      log.push('second:start')
      log.push('second:end')
    })

    await tick()
    // The second writer must not start while the first still holds the turn.
    expect(log).toEqual(['first:start'])
    expect(statsWriteDepth()).toBe(2)

    release()
    await Promise.all([first, second])
    expect(log).toEqual(['first:start', 'first:end', 'second:start', 'second:end'])
    expect(statsWriteDepth()).toBe(0)
  })

  it('returns the writer result and lets the next writer run after a failure', async () => {
    const failed = withStatsWrite(async () => { throw new Error('commit failed') })
    const next = withStatsWrite(async () => 42)

    await expect(failed).rejects.toThrow('commit failed')
    await expect(next).resolves.toBe(42)
    expect(statsWriteDepth()).toBe(0)
  })
})

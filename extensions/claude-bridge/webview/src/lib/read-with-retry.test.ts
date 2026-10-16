import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { ReadTimeoutError, readWithRetry } from './read-with-retry'

describe('readWithRetry', () => {
  beforeEach(() => { vi.useFakeTimers() })
  afterEach(() => { vi.useRealTimers() })

  it('asks once when the first reply arrives', async () => {
    const read = vi.fn(async () => 'ok')
    await expect(readWithRetry('config', read, [10, 20], 30)).resolves.toBe('ok')
    await vi.advanceTimersByTimeAsync(100)
    expect(read).toHaveBeenCalledTimes(1)
  })

  it('asks again after a lost reply and takes the first answer', async () => {
    const replies: Array<(v: string) => void> = []
    const read = vi.fn(() => new Promise<string>(resolve => { replies.push(resolve) }))
    const result = readWithRetry('config', read, [10, 20], 30)
    await vi.advanceTimersByTimeAsync(10)
    expect(read).toHaveBeenCalledTimes(2)
    replies[1]!('second')
    await expect(result).resolves.toBe('second')
    // A late reply of the first attempt and the cancelled timers change nothing.
    replies[0]!('late')
    await vi.advanceTimersByTimeAsync(100)
    expect(read).toHaveBeenCalledTimes(2)
  })

  it('does not re-ask a read the host answered with an error', async () => {
    const read = vi.fn(async () => { throw new Error('host said no') })
    await expect(readWithRetry('config', read, [10], 30)).rejects.toThrow('host said no')
    await vi.advanceTimersByTimeAsync(100)
    expect(read).toHaveBeenCalledTimes(1)
  })

  it('gives up with a timeout error once every attempt stayed unanswered', async () => {
    const read = vi.fn(() => new Promise<string>(() => { /* never answered */ }))
    const result = readWithRetry('config', read, [10, 20], 30)
    const assertion = expect(result).rejects.toBeInstanceOf(ReadTimeoutError)
    await vi.advanceTimersByTimeAsync(10 + 20 + 30)
    await assertion
    expect(read).toHaveBeenCalledTimes(3)
  })
})

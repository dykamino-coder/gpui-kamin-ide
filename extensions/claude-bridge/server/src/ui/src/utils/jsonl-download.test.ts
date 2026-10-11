import { afterEach, describe, expect, it, vi } from 'vitest'
import { createJsonlDownloader } from './jsonl-download'
afterEach(() => {
  vi.clearAllTimers()
  vi.useRealTimers()
})
describe('Dashboard native JSONL download handoff', () => {
  it('locks repeated clicks, posts Bearer only for grant, never buffers file and launches exact URL', async () => {
    vi.useFakeTimers()
    const busy = vi.fn()
    const launch = vi.fn(() => vi.fn())
    const request = vi.fn(
      async () => new Response(JSON.stringify({ url: '/api/dashboard/sessions/id/jsonl-download' })),
    )
    const downloader = createJsonlDownloader(busy, request as typeof fetch, launch)
    const first = downloader.start('id', 'synthetic')
    expect(await downloader.start('id', 'synthetic')).toBe(false)
    expect(await first).toBe(true)
    expect(request).toHaveBeenCalledTimes(1)
    expect(request.mock.calls[0]?.[0]).toBe('/api/dashboard/sessions/id/jsonl-download')
    expect(launch).toHaveBeenCalledWith('/api/dashboard/sessions/id/jsonl-download')
    vi.advanceTimersByTime(9999)
    expect(await downloader.start('id', 'synthetic')).toBe(false)
    vi.advanceTimersByTime(1)
    expect(await downloader.start('id', 'synthetic')).toBe(true)
    downloader.dispose()
  })
  it('resets after failure and cancels a pending grant on unmount', async () => {
    const busy = vi.fn()
    const request = vi.fn(async () => new Response('', { status: 403 }))
    const downloader = createJsonlDownloader(busy, request as typeof fetch, vi.fn())
    await expect(downloader.start('id', null)).rejects.toThrow('HTTP 403')
    expect(busy).toHaveBeenLastCalledWith(false)
    let pendingSignal: AbortSignal | undefined
    const launch = vi.fn()
    const pending = createJsonlDownloader(
      busy,
      ((_url, options) => {
        pendingSignal = options?.signal as AbortSignal
        return new Promise((_, reject) =>
          pendingSignal!.addEventListener('abort', () => reject(new Error('cancelled'))),
        )
      }) as typeof fetch,
      launch,
    )
    const started = pending.start('id', 'synthetic')
    pending.dispose()
    expect(await started).toBe(false)
    expect(pendingSignal!.aborted).toBe(true)
    expect(launch).not.toHaveBeenCalled()
  })
  it('refuses arbitrary response destinations', async () => {
    const downloader = createJsonlDownloader(
      vi.fn(),
      vi.fn(async () => new Response(JSON.stringify({ url: 'https://invalid.example/download' }))) as typeof fetch,
      vi.fn(),
    )
    await expect(downloader.start('id', null)).rejects.toThrow('Invalid download destination')
    downloader.dispose()
  })
})

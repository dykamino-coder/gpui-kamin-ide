// @vitest-environment jsdom
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { render } from 'preact'
import { act } from 'preact/test-utils'
import { JsonlTimestamp, formatRelativeStamp } from './JsonlTimestamp'

beforeEach(() => {
  vi.useFakeTimers()
  vi.setSystemTime(new Date('2026-11-20T10:00:00Z'))
})
afterEach(() => vi.useRealTimers())

it('labels an age relative to the given clock', () => {
  const now = Date.parse('2026-11-20T10:00:00Z')
  expect(formatRelativeStamp('2026-11-20T09:59:30Z', now).short).toBe('just now')
  expect(formatRelativeStamp('2026-11-20T09:57:00Z', now).short).toBe('3m ago')
  expect(formatRelativeStamp('2026-11-20T08:00:00Z', now).short).toBe('2h ago')
  expect(formatRelativeStamp('not a time', now).short).toBe('')
})

it('ages a label that its row never renders again', async () => {
  const host = document.createElement('div')
  await act(() => render(<JsonlTimestamp timestamp="2026-11-20T10:00:00Z" />, host))
  expect(host.textContent).toBe('just now')
  // Two hours pass with no new render of the row (an answer nobody touched).
  await act(() => { vi.advanceTimersByTime(2 * 3600_000) })
  expect(host.textContent).toBe('2h ago')
  render(null, host)
})

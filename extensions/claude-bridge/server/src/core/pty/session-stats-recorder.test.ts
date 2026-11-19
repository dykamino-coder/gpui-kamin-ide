import { afterEach, expect, it, vi } from 'vitest'
import { WriteQueueFullError, droppedStatsWrites } from '../stats/database/write-lock'

vi.mock('../stats/database/crud', () => ({
  insertRequest: vi.fn(() => Promise.reject(new WriteQueueFullError(512))),
}))

const { handleJsonlUserEntry } = await import('./session-stats-recorder')

const unhandled: unknown[] = []
const onUnhandled = (reason: unknown): void => {
  unhandled.push(reason)
}
process.on('unhandledRejection', onUnhandled)
afterEach(() => {
  process.off('unhandledRejection', onUnhandled)
})

it('counts a statistics row dropped by a full write queue instead of leaving the rejection unhandled', async () => {
  const before = droppedStatsWrites()
  const session = { id: 's1', userName: 'synthetic', inputCount: 0, mcpCallCount: 0 } as never
  handleJsonlUserEntry(session, { type: 'user', uuid: 'u1', message: { role: 'user', content: 'hello' } } as never)
  await new Promise((r) => setTimeout(r, 20))
  expect(droppedStatsWrites()).toBe(before + 1)
  expect(unhandled).toEqual([])
  expect((session as { inputCount: number }).inputCount).toBe(1)
})

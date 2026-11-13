import { afterEach, expect, it, vi } from 'vitest'
import { engineFilter } from './engine'

const getUsers = vi.hoisted(() => vi.fn())
vi.mock('../services/api-client', () => ({ api: { getUsers } }))

function deferred<T>() {
  let resolve!: (value: T) => void
  const promise = new Promise<T>((r) => {
    resolve = r
  })
  return { promise, resolve }
}

afterEach(() => {
  vi.unstubAllGlobals()
  vi.useRealTimers()
})

it('discards pending roster/cards responses across engine switches and resets after remount', async () => {
  vi.useFakeTimers()
  engineFilter.value = 'all'
  const oldUsers = deferred<any[]>()
  const oldCards = deferred<Response>()
  const fetchCards = vi
    .fn()
    .mockReturnValueOnce(oldCards.promise)
    .mockImplementation(
      async () => new Response(JSON.stringify({ revision: 'codex-rev', cards: { '*': { sessions: 2 } } })),
    )
  vi.stubGlobal('fetch', fetchCards)
  getUsers.mockReturnValueOnce(oldUsers.promise).mockResolvedValue([{ userName: 'codex-user' }])
  const stats = await import('./users-stats')
  let stop = stats.subscribeUserStats({ cards: true })
  engineFilter.value = 'codex'
  await vi.waitFor(() => expect(stats.users.value[0]?.userName).toBe('codex-user'))
  await vi.waitFor(() => expect(stats.cards.value?.['*'].sessions).toBe(2))
  expect(fetchCards.mock.calls[1][0]).toContain('&engine=codex')
  oldUsers.resolve([{ userName: 'stale-all' }])
  oldCards.resolve(new Response(JSON.stringify({ revision: 'all-rev', cards: { '*': { sessions: 99 } } })))
  await vi.waitFor(() => expect(stats.usersLoaded.value).toBe(true))
  await Promise.resolve()
  expect(stats.users.value[0]?.userName).toBe('codex-user')
  expect(stats.cards.value?.['*'].sessions).toBe(2)
  stop()

  engineFilter.value = 'claude'
  const pendingUsers = deferred<any[]>()
  getUsers.mockReturnValue(pendingUsers.promise)
  stop = stats.subscribeUserStats()
  expect(stats.users.value).toEqual([])
  expect(stats.usersLoaded.value).toBe(false)
  expect(stats.cards.value).toBeNull()
  expect(getUsers).toHaveBeenLastCalledWith('claude')
  pendingUsers.resolve([])
  await Promise.resolve()
  stop()
})

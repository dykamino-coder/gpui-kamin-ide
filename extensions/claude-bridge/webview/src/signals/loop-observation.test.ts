import { beforeEach, expect, it, vi } from 'vitest'
import type { TabInfo } from '../../shared/types'
import type { JsonlEntryData } from '../types/jsonl'

beforeEach(() => {
  vi.resetModules()
})
const stamp = (ms: number) => new Date(1700000000000 + ms).toISOString()
const tab = (id: string, authority = 'server-a'): TabInfo =>
  ({
    id,
    cwd: '/project',
    conversationId: `conversation-${id}`,
    connectionAuthority: authority,
  }) as TabInfo
const prompt = (id: string, ms = 0): JsonlEntryData => ({
  type: 'user',
  uuid: id,
  timestamp: stamp(ms),
  message: { role: 'user', content: 'Work' },
})
const completed = (): JsonlEntryData[] =>
  Array.from({ length: 5 }, (_, i) => [
    prompt(`u${i}`, i * 20000),
    {
      type: 'system',
      uuid: `end${i}`,
      timestamp: stamp(i * 20000 + 10000),
      subtype: 'turn_duration',
      durationMs: 10000,
    },
  ]).flat()

it('shares project history between conversations without crossing connection authorities', async () => {
  const { tabs } = await import('./tabs')
  const { appendJsonlEntries } = await import('./jsonl')
  const { loopObservations } = await import('./loop-observation')
  tabs.value = [tab('live'), tab('history'), tab('foreign', 'server-b')]
  appendJsonlEntries('live', [prompt('current')])
  appendJsonlEntries('history', completed())
  appendJsonlEntries('foreign', [prompt('foreign')])
  expect(loopObservations.value.get('live')?.baseline).toEqual({ samples: 5, durationMs: 10000, toolCalls: 0 })
  expect(loopObservations.value.get('foreign')?.baseline).toBeNull()
  expect(loopObservations.value.get('live')?.current?.key).toBe('current')
})

it('reuses observations across streaming text flushes and rescans committed structure', async () => {
  const { tabs } = await import('./tabs')
  const { appendJsonlEntries, jsonlEntriesByTab } = await import('./jsonl')
  const { loopObservations } = await import('./loop-observation')
  tabs.value = [tab('live')]
  appendJsonlEntries('live', [prompt('current')])
  const before = loopObservations.value
  // Same reference refresh used by the streaming renderer: no structural change.
  jsonlEntriesByTab.value = new Map(jsonlEntriesByTab.value)
  expect(loopObservations.value).toBe(before)
  appendJsonlEntries('live', [
    {
      type: 'assistant',
      uuid: 'tool',
      timestamp: stamp(1000),
      message: { role: 'assistant', content: [{ type: 'tool_use', id: 't', name: 'Read', input: { path: 'a' } }] },
    },
  ])
  expect(loopObservations.value).not.toBe(before)
  expect(loopObservations.value.get('live')?.current?.toolCalls).toBe(1)
})

it('clears stale live observations when a tab is reset or closed', async () => {
  const { tabs } = await import('./tabs')
  const { appendJsonlEntries, clearJsonlEntries } = await import('./jsonl')
  const { loopObservations } = await import('./loop-observation')
  tabs.value = [tab('live')]
  appendJsonlEntries('live', [prompt('old')])
  expect(loopObservations.value.get('live')?.current?.key).toBe('old')
  clearJsonlEntries('live')
  expect(loopObservations.value.get('live')?.current).toBeNull()
  tabs.value = [{ ...tab('live'), conversationId: 'new-conversation' }]
  appendJsonlEntries('live', [prompt('new')])
  expect(loopObservations.value.get('live')?.current?.key).toBe('new')
  tabs.value = []
  expect(loopObservations.value.size).toBe(0)
})

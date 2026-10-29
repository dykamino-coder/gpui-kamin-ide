import { afterEach, expect, it } from 'vitest'
import { appendJsonlEntries, applyStreamingEntry, clearJsonlEntries, jsonlEntriesByTab, replaceWindowWithSegment, setArchivedView } from './jsonl'
import { orderEntries } from './order-entries'
import type { JsonlEntryData } from '../types/jsonl'

const row = (uuid: string, _pos: number, timestamp?: string) => ({ type: 'assistant', uuid, _pos, timestamp, message: { role: 'assistant', content: [{ type: 'text', text: uuid }] } }) as JsonlEntryData
const ids = () => jsonlEntriesByTab.value.get('archive-loading')?.map((r) => r.uuid)
afterEach(() => clearJsonlEntries('archive-loading'))

it('keeps the visible archive stable during replay and restores Current immediately with live arrivals', () => {
  appendJsonlEntries('archive-loading', [row('recent', 100)])
  setArchivedView('archive-loading', 'old-boundary')
  replaceWindowWithSegment('archive-loading', [row('old-final-answer', 50)])
  appendJsonlEntries('archive-loading', [row('recent', 100), row('live', 200)])
  applyStreamingEntry('archive-loading', { ...row('stream-future', 300), __streaming: true, message: { id: 'future', role: 'assistant', content: [{ type: 'text', text: 'future' }] } } as JsonlEntryData)
  expect(ids()).toEqual(['old-final-answer'])
  setArchivedView('archive-loading', null)
  expect(ids()).toEqual(['recent', 'live', 'stream-future'])
})

it('uses physical file order across clock skew, missing timestamps and replay generations', () => {
  const final = row('final-before-idle-compact', 20, '2026-10-29T14:56:30Z')
  const task = row('task-completed', 10)
  const current = row('after-boundary', 30, '2026-10-29T14:56:00Z')
  expect(orderEntries([current, final, task]).map((r) => r.uuid)).toEqual(['task-completed', 'final-before-idle-compact', 'after-boundary'])
})

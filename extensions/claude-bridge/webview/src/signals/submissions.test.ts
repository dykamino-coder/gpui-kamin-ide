import { beforeEach, describe, expect, it } from 'vitest'
import { jsonlEntriesByTab } from './jsonl'
import { noteSubmissionRows, pendingSubmissionRows, receiveSubmission, submissions } from './submissions'
import type { JsonlEntryData } from '../types/jsonl'
import { clearLocalQueue, localQueue, reconcileQueueWithEntries, sweepStaleQueue } from './queue'

beforeEach(() => { clearLocalQueue('tab'); submissions.value = new Map(); jsonlEntriesByTab.value = new Map() })
describe('submission transcript identity', () => {
  it('clears a composer warning when a later receipt no longer carries it', () => {
    receiveSubmission('tab', { id: 'id', text: 'sent', state: 'queued', warning: 'Draft exists' })
    receiveSubmission('tab', { id: 'id', state: 'native' })
    expect(submissions.value.get('tab')![0].warning).toBeUndefined()
  })
  it('keeps identical queued prose separate and collapses only the acknowledged UUID', () => {
    for (const id of ['one', 'two', 'three']) receiveSubmission('tab', { id, text: 'same', state: 'queued' })
    const row = { type: 'attachment', uuid: 'stored-two' } as JsonlEntryData
    receiveSubmission('tab', { id: 'two', state: 'read', uuid: row.uuid })
    const items = submissions.value.get('tab')!
    expect(pendingSubmissionRows([row], items).map(row => row.submissionId)).toEqual(['one', 'three'])
    expect(pendingSubmissionRows([{ ...row, uuid: 'unrelated' }], items)).toHaveLength(3)
  })
  it('does not resurrect an acknowledged row when the transcript window is trimmed', () => {
    receiveSubmission('tab', { id: 'id', text: 'sent', state: 'read', uuid: 'stored' })
    noteSubmissionRows('tab', [{ uuid: 'stored' } as JsonlEntryData])
    expect(pendingSubmissionRows([], submissions.value.get('tab')!)).toEqual([])
  })
  it('handles JSONL arriving before the receipt and removes cancelled rows', () => {
    jsonlEntriesByTab.value = new Map([['tab', [{ uuid: 'stored' } as JsonlEntryData]]])
    receiveSubmission('tab', { id: 'id', text: 'sent', state: 'read', uuid: 'stored' })
    receiveSubmission('tab', { id: 'cancel', text: 'never sent', state: 'cancelled' })
    expect(pendingSubmissionRows([], submissions.value.get('tab')!)).toEqual([])
  })

  it('replays a fallback identity once and never resurrects it after JSONL acceptance', () => {
    const receipt = { id: 'fallback', text: 'queued message', state: 'fallback' as const }
    receiveSubmission('tab', receipt)
    receiveSubmission('tab', receipt)
    expect(localQueue.value.get('tab')?.map(item => item.id)).toEqual(['fallback'])
    expect(pendingSubmissionRows([], submissions.value.get('tab')!)).toEqual([])
    jsonlEntriesByTab.value = new Map([['tab', [{ type: 'user', uuid: 'accepted', _ord: 1,
      message: { role: 'user', content: 'queued message' } }]]])
    reconcileQueueWithEntries('tab')
    expect(localQueue.value.has('tab')).toBe(false)
    receiveSubmission('tab', receipt)
    expect(localQueue.value.has('tab')).toBe(false)
  })

  it('restores the host cursor and leaves acceptance to the host', () => {
    jsonlEntriesByTab.value = new Map([['tab', [{ type: 'user', uuid: 'accepted', _ord: 11,
      message: { role: 'user', content: 'queued message' } }]]])
    receiveSubmission('tab', { id: 'fallback', text: 'queued message', state: 'fallback',
      fallbackQueue: { afterOrd: 10, enqueuedAt: 123 } })
    expect(localQueue.value.get('tab')?.[0]).toMatchObject({ id: 'fallback', afterOrd: 10, enqueuedAt: 123, hostOwned: true })
    receiveSubmission('tab', { id: 'fallback', state: 'fallback', fallbackQueue: { afterOrd: 10, enqueuedAt: 123, consumed: true } })
    expect(localQueue.value.has('tab')).toBe(false)
  })

  it('does not reuse a consumed host row for the next identical pending message', () => {
    jsonlEntriesByTab.value = new Map([['tab', [{ type: 'user', uuid: 'accepted', _ord: 11,
      message: { role: 'user', content: 'same queued message' } }]]])
    receiveSubmission('tab', { id: 'first', text: 'same queued message', state: 'fallback',
      fallbackQueue: { afterOrd: 10, enqueuedAt: 123, consumed: true } })
    receiveSubmission('tab', { id: 'second', text: 'same queued message', state: 'fallback',
      fallbackQueue: { afterOrd: 10, enqueuedAt: 124 } })
    reconcileQueueWithEntries('tab')
    sweepStaleQueue()
    expect(localQueue.value.get('tab')?.map(item => item.id)).toEqual(['second'])
  })

  it('honors host consumption even when the accepted row is outside the replay window', () => {
    const receipt = { id: 'fallback', text: 'queued message', state: 'fallback' as const,
      fallbackQueue: { afterOrd: 10, enqueuedAt: 123 } }
    receiveSubmission('tab', receipt)
    expect(localQueue.value.get('tab')?.[0].enqueuedAt).toBe(123)
    receiveSubmission('tab', { ...receipt, fallbackQueue: { ...receipt.fallbackQueue, consumed: true } })
    expect(localQueue.value.has('tab')).toBe(false)
    // New page with no prior receipt or transcript state.
    submissions.value = new Map()
    receiveSubmission('tab', { ...receipt, fallbackQueue: { ...receipt.fallbackQueue, consumed: true } })
    expect(localQueue.value.has('tab')).toBe(false)
  })
})

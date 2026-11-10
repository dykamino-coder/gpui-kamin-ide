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

  it('honors host commitment on a new page without the matching transcript row', () => {
    receiveSubmission('tab', { id: 'id', text: 'sent', state: 'read', uuid: 'stored', committed: true })
    expect(pendingSubmissionRows([], submissions.value.get('tab')!)).toEqual([])
    receiveSubmission('tab', { id: 'id', state: 'native', committed: false })
    expect(pendingSubmissionRows([], submissions.value.get('tab')!)).toEqual([])
    receiveSubmission('tab', { id: 'another', text: 'sent', state: 'read', uuid: 'different' })
    expect(pendingSubmissionRows([], submissions.value.get('tab')!).map(row => row.submissionId)).toEqual(['another'])
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

  it('retains host display dismissal across late receipts and a fresh page without cancelling delivery', () => {
    const receipt = { id: 'fallback', text: 'restore in composer', state: 'fallback' as const,
      fallbackQueue: { afterOrd: 10, enqueuedAt: 123 } }
    receiveSubmission('tab', receipt)
    expect(localQueue.value.has('tab')).toBe(true)
    receiveSubmission('tab', { ...receipt, fallbackQueue: { ...receipt.fallbackQueue, dismissed: true } })
    expect(localQueue.value.has('tab')).toBe(false)
    receiveSubmission('tab', receipt)
    expect(submissions.value.get('tab')![0].fallbackQueue?.dismissed).toBe(true)
    expect(submissions.value.get('tab')![0].state).toBe('fallback')
    submissions.value = new Map()
    receiveSubmission('tab', { ...receipt, fallbackQueue: { ...receipt.fallbackQueue, dismissed: true } })
    receiveSubmission('tab', receipt)
    expect(localQueue.value.has('tab')).toBe(false)
    expect(pendingSubmissionRows([], submissions.value.get('tab')!)).toEqual([])
  })
})

describe('stale pending receipts settle from the transcript', () => {
  const user = (uuid: string, text: string, extra: Partial<JsonlEntryData> = {}): JsonlEntryData =>
    ({ type: 'user', uuid, message: { role: 'user', content: text }, ...extra }) as JsonlEntryData
  const pending = () => pendingSubmissionRows(jsonlEntriesByTab.value.get('tab') ?? [], submissions.value.get('tab') ?? [])
    .map(row => row.submissionId)
  const append = (...rows: JsonlEntryData[]) => {
    jsonlEntriesByTab.value = new Map([['tab', [...(jsonlEntriesByTab.value.get('tab') ?? []), ...rows]]])
    noteSubmissionRows('tab', rows)
  }
  const t0 = Date.parse('2026-11-10T10:00:00Z')

  it('drops an unconfirmed bubble once the CLI wrote the same prompt after it', () => {
    receiveSubmission('tab', { id: 'u', text: 'do not touch the role', state: 'uncertain', createdAt: t0 })
    expect(pending()).toEqual(['u'])
    append(user('row', 'do not touch the role', { timestamp: new Date(t0 + 500).toISOString() }))
    expect(pending()).toEqual([])
  })

  it('turns a queued receipt the restarted server forgot into a dismissible one, then settles it', () => {
    receiveSubmission('tab', { id: 'lost', text: 'queued before restart', state: 'queued', cancelable: true, createdAt: t0 })
    receiveSubmission('tab', { id: 'kept', text: 'still tracked', state: 'queued', cancelable: true, createdAt: t0 })
    receiveSubmission('tab', { id: '', known: ['kept'] })
    const [lost, kept] = submissions.value.get('tab')!
    expect(lost).toMatchObject({ state: 'uncertain', cancelable: false })
    expect(lost.warning).toMatch(/no longer tracks/)
    expect(kept).toMatchObject({ state: 'queued', cancelable: true })
    append(user('row', 'queued before restart', { timestamp: new Date(t0 + 60_000).toISOString() }))
    expect(pending()).toEqual(['kept'])
  })

  it('never lets an older identical prompt confirm a new receipt', () => {
    jsonlEntriesByTab.value = new Map([['tab', [user('old', 'again', { timestamp: new Date(t0 - 60_000).toISOString() })]]])
    receiveSubmission('tab', { id: 'new', text: 'again', state: 'uncertain', createdAt: t0 })
    noteSubmissionRows('tab', [])
    expect(pending()).toEqual(['new'])
  })

  it('without server time trusts only positioned rows past its watermark', () => {
    jsonlEntriesByTab.value = new Map([['tab', [user('old', 'again', { _ord: 5 })]]])
    receiveSubmission('tab', { id: 'new', text: 'again', state: 'uncertain' })
    append(user('live', 'again'))
    expect(pending()).toEqual(['new'])
    append(user('replayed', 'again', { _ord: 6 }))
    expect(pending()).toEqual([])
  })

  it('confirms identical prompts one row each', () => {
    for (const id of ['a', 'b']) receiveSubmission('tab', { id, text: 'same', state: 'uncertain', createdAt: t0 })
    append(user('r1', 'same', { timestamp: new Date(t0 + 1).toISOString() }))
    expect(pending()).toEqual(['b'])
    append(user('r2', 'same', { timestamp: new Date(t0 + 2).toISOString() }))
    expect(pending()).toEqual([])
  })
})

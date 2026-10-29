import { beforeEach, describe, expect, it } from 'vitest'
import { jsonlEntriesByTab } from './jsonl'
import { noteSubmissionRows, pendingSubmissionRows, receiveSubmission, submissions } from './submissions'
import type { JsonlEntryData } from '../types/jsonl'

beforeEach(() => { submissions.value = new Map(); jsonlEntriesByTab.value = new Map() })
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
})

// INC-2026-0031: a queued message or an unconfirmed receipt stayed "pending"
// after the CLI had consumed it, when the Bridge server restarted in between.
// The watermark was the server's `_ord`, which each server process numbers
// anew (jsonl-watcher.ts: `_ord = base + byte offset`, then the counter moves
// past the file), so the same transcript comes back numbered lower.
import { beforeEach, describe, expect, it } from 'vitest'
import { clearLocalQueue, enqueueLocal, localQueue, reconcileQueueWithEntries } from './queue'
import { jsonlEntriesByTab } from './jsonl'
import { noteSubmissionRows, receiveSubmission, submissions } from './submissions'
import type { JsonlEntryData } from '../types/jsonl'

const TAB = 'restart'
const SIZE = 10_000 // transcript bytes
const ts = new Date(Date.now() + 1000).toISOString()
/** A server row at byte `offset`, numbered by a process whose counter starts at `base`. */
const user = (base: number, offset: number, text: string): JsonlEntryData =>
  ({ type: 'user', uuid: `u${offset}`, _pos: offset, _ord: base + offset, timestamp: ts, message: { content: text } }) as unknown as JsonlEntryData
const oldServer = SIZE + 1 // after one replay
const newServer = 0 // first replay of a restarted server
const set = (rows: JsonlEntryData[]): void => { jsonlEntriesByTab.value = new Map([[TAB, rows]]) }

beforeEach(() => { clearLocalQueue(TAB); submissions.value = new Map(); jsonlEntriesByTab.value = new Map() })

describe('pending items across a Bridge server restart', () => {
  it('a queued message consumed after the restart leaves the queue', () => {
    set([user(oldServer, 9_000, 'earlier message already answered')])
    enqueueLocal(TAB, 'queued while the CLI was busy, please continue')
    set([user(newServer, 9_000, 'earlier message already answered'), user(newServer, 9_500, 'queued while the CLI was busy, please continue')])
    reconcileQueueWithEntries(TAB)
    expect(localQueue.value.get(TAB)?.length ?? 0).toBe(0)
  })

  it('the earlier row still cannot consume the queued message after the restart', () => {
    set([user(oldServer, 9_000, 'the same words twice in a row')])
    enqueueLocal(TAB, 'the same words twice in a row')
    set([user(newServer, 9_000, 'the same words twice in a row')])
    reconcileQueueWithEntries(TAB)
    expect(localQueue.value.get(TAB)?.length).toBe(1)
  })

  it('an unconfirmed receipt is settled by the row the restarted server reports', () => {
    set([user(oldServer, 9_000, 'earlier message already answered')])
    receiveSubmission(TAB, { id: 's1', text: 'sent just before the server restarted', state: 'queued' })
    // The restarted server no longer tracks the id: the receipt turns uncertain.
    receiveSubmission(TAB, { id: '', known: [] })
    expect(submissions.value.get(TAB)?.[0]?.state).toBe('uncertain')
    const rows = [user(newServer, 9_000, 'earlier message already answered'), user(newServer, 9_500, 'sent just before the server restarted')]
    set(rows)
    noteSubmissionRows(TAB, rows)
    expect(submissions.value.get(TAB)?.[0]?.committed).toBe(true)
  })

  it('a host-owned fallback item keeps the host watermark', () => {
    set([user(oldServer, 9_000, 'earlier message already answered')])
    receiveSubmission(TAB, { id: 'f1', text: '/compact keep the plan', state: 'fallback',
      fallbackQueue: { afterOrd: oldServer + 9_000, afterPos: 9_000, enqueuedAt: Date.now() } })
    expect(localQueue.value.get(TAB)?.[0]).toMatchObject({ afterPos: 9_000, hostOwned: true })
  })
})

// Receipts correlate optimistic user rows by ID and stored UUID. A receipt the
// server can no longer settle (restart, lost read receipt) falls back to the shared
// transcript matcher, bounded by a position watermark and the server's time.
import { signal } from '@preact/signals'
import type { SubmissionUpdate } from '../../shared/types'
import { LOST_RECEIPT_WARNING, lostAfterSnapshot, matchReceiptsToTranscript, mergeFallbackQueue, queueRowIdentity, queueWatermark, unresolvedReceipt } from '../../shared/submission-queue'
import type { JsonlEntryData } from '../types/jsonl'
import { enqueueLocal, reconcileQueueWithEntries, removeLocalQueueItem } from './queue'
import { jsonlEntriesByTab } from './jsonl'

export interface Submission extends SubmissionUpdate {
  text: string
  timestamp: string
  /** Store position watermark when this id first appeared: older rows never confirm it. */
  afterOrd?: number
  /** Identity of the transcript row that confirmed it (claims the row). */
  matchedRow?: string
}
export const modCapabilities = signal(new Map<string, boolean>())
export const submissions = signal(new Map<string, Submission[]>())

export function receiveSubmission(tabId: string, data: SubmissionUpdate): void {
  if (typeof data.capable === 'boolean') {
    modCapabilities.value = new Map(modCapabilities.value).set(tabId, data.capable)
    return
  }
  if (Array.isArray(data.known)) { applyKnownSnapshot(tabId, new Set(data.known)); return }
  if (!data.id) return
  const current = submissions.value.get(tabId) ?? []
  const existing = current.find(item => item.id === data.id)
  const item: Submission = { text: '', timestamp: new Date().toISOString(), ...existing, ...data, warning: data.warning,
    afterOrd: existing?.afterOrd ?? queueWatermark(jsonlEntriesByTab.peek().get(tabId) ?? []) }
  if (existing?.committed) item.committed = true
  item.fallbackQueue = mergeFallbackQueue(existing?.fallbackQueue, data.fallbackQueue)
  if (item.state === 'fallback') {
    if (item.fallbackQueue?.consumed || item.fallbackQueue?.dismissed) removeLocalQueueItem(tabId, item.id)
    else if (item.text && existing?.state !== 'fallback') {
      enqueueLocal(tabId, item.text, { id: item.id, ...item.fallbackQueue, hostOwned: !!item.fallbackQueue })
      // Старый host без метаданных использует desktop matcher. Browser host
      // сам принимает решение, даже когда принявшая строка уже вне replay window.
      reconcileQueueWithEntries(tabId)
    }
  }
  if (item.uuid && jsonlEntriesByTab.peek().get(tabId)?.some(row => row.uuid === item.uuid)) item.committed = true
  const next = existing ? current.map(old => old.id === item.id ? item : old) : [...current, item]
  // Receipt retention is bounded independently of the transcript window.
  submissions.value = new Map(submissions.value).set(tabId, settleFromTranscript(tabId, next.slice(-256)))
}

export function optimisticSubmission(tabId: string, text: string): string {
  const id = crypto.randomUUID()
  receiveSubmission(tabId, { id, text, state: 'queued', cancelable: true })
  return id
}

export function pendingSubmissionRows(entries: JsonlEntryData[], items: Submission[]): JsonlEntryData[] {
  const uuids = new Set(entries.map(entry => entry.uuid))
  return items.filter(item => item.state !== 'cancelled' && item.state !== 'fallback' && !item.committed && (!item.uuid || !uuids.has(item.uuid)))
    .map(item => ({ type: 'user', uuid: item.uuid ?? item.id, timestamp: item.timestamp,
      message: { role: 'user', content: item.text }, submissionId: item.id }))
}

export function noteSubmissionRows(tabId: string, entries: JsonlEntryData[]): void {
  const current = submissions.peek().get(tabId)
  if (!current) return
  const uuids = new Set(entries.map(row => row.uuid))
  const byUuid = current.some(item => !item.committed && item.uuid && uuids.has(item.uuid))
    ? current.map(item => item.uuid && uuids.has(item.uuid) ? { ...item, committed: true } : item)
    : current
  const next = settleFromTranscript(tabId, byUuid)
  if (next !== current) submissions.value = new Map(submissions.peek()).set(tabId, next)
}

/** Commit receipts the transcript already answers (see submission-queue.ts). */
function settleFromTranscript(tabId: string, list: Submission[]): Submission[] {
  if (!list.some(unresolvedReceipt)) return list
  const entries = jsonlEntriesByTab.peek().get(tabId) ?? []
  if (entries.length === 0) return list
  const consumed = new Set<string>()
  for (const item of list) {
    if (item.uuid) consumed.add(`uuid:${item.uuid}`)
    if (item.matchedRow) consumed.add(item.matchedRow)
  }
  const matches = matchReceiptsToTranscript(list.map(item => ({ ...item, afterOrd: item.afterOrd ?? -1 })), entries, consumed)
  if (matches.length === 0) return list
  const rowById = new Map(matches.map(({ id, entry }) => [id, queueRowIdentity(entry)]))
  return list.map(item => rowById.has(item.id) ? { ...item, committed: true, matchedRow: rowById.get(item.id) } : item)
}

/** Reattach snapshot: pending receipts the server no longer tracks will never
 *  get another update. Show them as unconfirmed (with Dismiss) instead of a
 *  Queued bubble whose Cancel reaches nothing, then let the transcript settle them. */
function applyKnownSnapshot(tabId: string, known: ReadonlySet<string>): void {
  const current = submissions.peek().get(tabId)
  if (!current?.some(item => lostAfterSnapshot(item, known))) return
  const next = current.map(item => lostAfterSnapshot(item, known)
    ? { ...item, state: 'uncertain' as const, cancelable: false, warning: LOST_RECEIPT_WARNING }
    : item)
  submissions.value = new Map(submissions.peek()).set(tabId, settleFromTranscript(tabId, next))
}

export function forgetSubmissions(tabId: string): void {
  const next = new Map(submissions.peek()); next.delete(tabId); submissions.value = next
  const capabilities = new Map(modCapabilities.peek()); capabilities.delete(tabId); modCapabilities.value = capabilities
}

export function dismissUncertain(tabId: string, id: string): void {
  const current = submissions.peek().get(tabId) ?? []
  submissions.value = new Map(submissions.peek()).set(tabId,
    current.filter(item => item.id !== id || item.state !== 'uncertain'))
}

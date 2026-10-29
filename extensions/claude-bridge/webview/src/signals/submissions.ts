// Receipts correlate optimistic user rows by ID and stored UUID, never prose.
import { signal } from '@preact/signals'
import type { SubmissionUpdate } from '../../shared/types'
import type { JsonlEntryData } from '../types/jsonl'
import { enqueueLocal } from './queue'
import { jsonlEntriesByTab } from './jsonl'

export interface Submission extends SubmissionUpdate { text: string; timestamp: string; committed?: boolean }
export const modCapabilities = signal(new Map<string, boolean>())
export const submissions = signal(new Map<string, Submission[]>())

export function receiveSubmission(tabId: string, data: SubmissionUpdate): void {
  if (typeof data.capable === 'boolean') {
    modCapabilities.value = new Map(modCapabilities.value).set(tabId, data.capable)
    return
  }
  if (!data.id) return
  const current = submissions.value.get(tabId) ?? []
  const existing = current.find(item => item.id === data.id)
  if (data.state === 'fallback') {
    if (existing) enqueueLocal(tabId, existing.text)
    submissions.value = new Map(submissions.value).set(tabId, current.filter(item => item.id !== data.id))
    return
  }
  const item: Submission = { text: '', timestamp: new Date().toISOString(), ...existing, ...data, warning: data.warning }
  if (item.uuid && jsonlEntriesByTab.peek().get(tabId)?.some(row => row.uuid === item.uuid)) item.committed = true
  const next = existing ? current.map(old => old.id === item.id ? item : old) : [...current, item]
  // Receipt retention is bounded independently of the transcript window.
  submissions.value = new Map(submissions.value).set(tabId, next.slice(-256))
}

export function optimisticSubmission(tabId: string, text: string): string {
  const id = crypto.randomUUID()
  receiveSubmission(tabId, { id, text, state: 'queued', cancelable: true })
  return id
}

export function pendingSubmissionRows(entries: JsonlEntryData[], items: Submission[]): JsonlEntryData[] {
  const uuids = new Set(entries.map(entry => entry.uuid))
  return items.filter(item => item.state !== 'cancelled' && !item.committed && (!item.uuid || !uuids.has(item.uuid)))
    .map(item => ({ type: 'user', uuid: item.uuid ?? item.id, timestamp: item.timestamp,
      message: { role: 'user', content: item.text }, submissionId: item.id }))
}

export function noteSubmissionRows(tabId: string, entries: JsonlEntryData[]): void {
  const current = submissions.peek().get(tabId)
  if (!current) return
  const uuids = new Set(entries.map(row => row.uuid))
  if (!current.some(item => !item.committed && item.uuid && uuids.has(item.uuid))) return
  submissions.value = new Map(submissions.peek()).set(tabId,
    current.map(item => item.uuid && uuids.has(item.uuid) ? { ...item, committed: true } : item))
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

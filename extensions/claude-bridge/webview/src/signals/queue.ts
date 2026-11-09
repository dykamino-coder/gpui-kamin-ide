import { signal, computed } from '@preact/signals'
import { activeTabId } from './tabs'
import { jsonlEntriesByTab } from './jsonl'
import { matchSubmissionQueue, queueRowIdentity, queueWatermark, type LocalQueueItem } from '../../shared/submission-queue'

// Claude CLI writes queue-operation entries in JSONL but *without* `content`
// in recent versions (2.1.116+). We can't reconstruct the queue text from
// that stream, so we track it locally: push on send while CLI is busy,
// pop when a matching `user` entry appears in JSONL (CLI accepted the
// message). The queue-operation stream is only a fallback for counts.

export type { LocalQueueItem } from '../../shared/submission-queue'
const consumedRows = new Map<string, Set<string>>()
export const localQueue = signal<Map<string, LocalQueueItem[]>>(new Map())

export function enqueueLocal(tabId: string, content: string, identity?: Partial<LocalQueueItem>): void {
  const next = new Map(localQueue.value)
  const list = [...(next.get(tabId) ?? [])]
  const id = identity?.id ?? `${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`
  if (list.some(item => item.id === id)) return
  list.push({ id, content, enqueuedAt: identity?.enqueuedAt ?? Date.now(),
    afterOrd: identity?.afterOrd ?? queueWatermark(jsonlEntriesByTab.value.get(tabId) ?? []),
    ...(identity?.hostOwned ? { hostOwned: true } : {}) })
  next.set(tabId, list)
  localQueue.value = next
}

export function removeLocalQueueItem(tabId: string, id: string): void {
  const list = localQueue.peek().get(tabId)
  if (!list?.some(item => item.id === id)) return
  const next = new Map(localQueue.peek())
  const kept = list.filter(item => item.id !== id)
  if (kept.length) next.set(tabId, kept)
  else next.delete(tabId)
  localQueue.value = next
}

export function clearLocalQueue(tabId: string): void {
  consumedRows.delete(tabId)
  const next = new Map(localQueue.value)
  next.delete(tabId)
  localQueue.value = next
}

// Стабильные UUID/_ord не дают одной строке принять два одинаковых prompt
// при отдельных JSONL batches. Matcher также используется headless host.
export function reconcileQueueWithEntries(tabId: string): void {
  const list = localQueue.value.get(tabId)
  if (!list?.length) return
  const used = consumedRows.get(tabId) ?? new Set<string>()
  const matches = matchSubmissionQueue(list.filter(item => !item.hostOwned), jsonlEntriesByTab.value.get(tabId) ?? [], used)
  if (!matches.length) return
  consumedRows.set(tabId, used)
  for (const { item, entry } of matches) {
    const key = queueRowIdentity(entry)
    if (key) used.add(key)
    removeLocalQueueItem(tabId, item.id)
  }
}

// TTL sweep: anything older than 10 minutes without a matching JSONL entry
// is likely stale (user aborted, CLI crashed, etc.) — drop it.
const QUEUE_TTL_MS = 10 * 60 * 1000
export function sweepStaleQueue(): void {
  const now = Date.now()
  let changed = false
  const next = new Map(localQueue.value)
  for (const [tabId, list] of next) {
    const fresh = list.filter(i => i.hostOwned || now - i.enqueuedAt < QUEUE_TTL_MS)
    if (fresh.length === list.length) continue
    changed = true
    if (fresh.length === 0) next.delete(tabId)
    else next.set(tabId, fresh)
  }
  if (changed) localQueue.value = next
}

export const activeLocalQueue = computed<LocalQueueItem[]>(() => {
  const id = activeTabId.value
  if (!id) return []
  return localQueue.value.get(id) ?? []
})

import { signal } from '@preact/signals'
import type { RequestLogEntry } from '../types'

export const captureMode = signal(false)
export const capturedLog = signal<RequestLogEntry[]>([])
export const selectedRequest = signal<RequestLogEntry | null>(null)

/** Accumulated error entries (newest first) */
export const errorLog = signal<RequestLogEntry[]>([])

export function addRequest(entry: RequestLogEntry) {
  // Always prepend to the log (dedup by id)
  if (!capturedLog.value.some(r => r.id === entry.id)) {
    capturedLog.value = [entry, ...capturedLog.value].slice(0, 100)
  }
  if (captureMode.value) {
    captureMode.value = false
  }
}

export function updateRequest(id: string, updates: Partial<RequestLogEntry>) {
  capturedLog.value = capturedLog.value.map(r =>
    r.id === id ? { ...r, ...updates } : r
  )
}

/** Add a completed error entry to the error log (deduplicates by id) */
export function addError(entry: RequestLogEntry) {
  const existing = errorLog.value
  if (existing.some(e => e.id === entry.id)) return
  errorLog.value = [entry, ...existing]
}

/** Seed error log from initial request log (on init) */
export function seedErrors(entries: RequestLogEntry[]) {
  const errors = entries.filter(e => e.status === 'error')
  if (errors.length === 0) return
  const existing = new Set(errorLog.value.map(e => e.id))
  const newErrors = errors.filter(e => !existing.has(e.id))
  if (newErrors.length > 0) {
    errorLog.value = [...newErrors, ...errorLog.value]
      .sort((a, b) => new Date(b.timestamp).getTime() - new Date(a.timestamp).getTime())
  }
}

export function clearErrors() {
  errorLog.value = []
}

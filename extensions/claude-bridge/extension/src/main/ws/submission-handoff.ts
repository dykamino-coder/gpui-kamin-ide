import type { SubmissionUpdate } from '../../shared/types'

/** Tab-owned outbox survives manual PTY replacement, unlike server receipts. */
export class SubmissionHandoff {
  private pending = new Map<string, SubmissionUpdate & { text: string }>()
  private ready = new Set<string>()
  private inFlight = new Set<string>()

  constructor(private emit: (update: SubmissionUpdate) => void) {}

  remember(id: string, text: string): void {
    if (!this.pending.has(id)) this.pending.set(id, { id, text, state: 'queued' })
  }

  receive(update: SubmissionUpdate): void {
    if (!update.id) return
    this.inFlight.delete(update.id)
    if (['read', 'cancelled', 'fallback'].includes(update.state ?? '')) {
      this.pending.delete(update.id)
      this.ready.delete(update.id)
      return
    }
    const previous = this.pending.get(update.id)
    if (previous || update.text) this.pending.set(update.id, {
      ...previous, ...update, text: previous?.text ?? update.text!, retryable: false,
    })
  }

  end(transferable: readonly string[], confirmed: boolean): void {
    const safe = new Set(transferable)
    for (const item of this.pending.values()) {
      // A repeated Disconnect while offline must retain the first handoff.
      if (this.ready.has(item.id) || item.retryable) continue
      if (safe.has(item.id)) {
        item.state = 'queued'
        item.cancelable = true
        item.warning = 'Waiting to reconnect before delivering this message.'
        this.ready.add(item.id)
      } else {
        item.state = 'uncertain'
        item.retryable = true
        item.cancelable = true
        item.warning = confirmed
          ? 'The previous CLI session ended before delivery was confirmed. Check the transcript, then retry or cancel.'
          : 'The previous server did not confirm the handoff. Check the transcript, then retry or cancel.'
      }
      this.emit({ ...item })
    }
  }

  /** A snapshot may race the new session's first receipts. */
  tracked(): string[] { return [...this.ready, ...this.inFlight] }

  replay(): void {
    for (const item of this.pending.values()) this.emit({ ...item })
  }

  resume(send: (text: string, id: string) => void): void {
    for (const id of this.ready) {
      this.ready.delete(id)
      const item = this.pending.get(id)
      if (!item) continue
      this.inFlight.add(id)
      item.warning = undefined
      this.emit({ ...item })
      send(item.text, id)
    }
  }

  action(id: string, action: string, connected: boolean, send: (text: string, id: string) => void): boolean {
    const item = this.pending.get(id)
    if (!item || (!this.ready.has(id) && !item.retryable)) return false
    if (action === 'cancel') {
      this.ready.delete(id)
      this.pending.delete(id)
      this.emit({ ...item, state: 'cancelled', retryable: false, cancelable: false, warning: undefined })
    } else if (action === 'push' && connected) {
      item.state = 'queued'
      item.retryable = false
      item.warning = undefined
      this.ready.delete(id)
      this.inFlight.add(id)
      this.emit({ ...item })
      send(item.text, id)
    } else if (action === 'push') {
      item.warning = 'Reconnect this tab before retrying delivery.'
      this.emit({ ...item })
    }
    return true
  }
}

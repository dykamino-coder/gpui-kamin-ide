import type { ClientFrame, HostFrame } from './protocol.js'
import { createHash } from 'node:crypto'
import type { SubmissionStorage } from './submission-state.js'
import { LOST_RECEIPT_WARNING, lostAfterSnapshot, matchReceiptsToTranscript, matchSubmissionQueue, mergeFallbackQueue, queueRowIdentity, queueWatermark, type QueueEntry, type SubmissionUpdate } from '../../webview/shared/submission-queue.js'

const TAB_EVENTS = new Set(['session:activity', 'session:usage', 'session:title', 'connection-state-changed'])
const GLOBAL_EVENTS = new Set(['tree-update', 'mcp-servers-changed', 'mcp-loading', 'mcp-ready', 'config-ready'])
const QUESTIONS = new Set(['permission-request', 'elicitation-request', 'ask-user-question', 'mcp-elicitation-request'])

/** Host-owned presentation metadata. Transcripts and execution stay in the
 * shared ConnectionManager; this index never implements an agent handler. */
export class WorkspacePresentation {
  private latest = new Map<string, HostFrame>()
  private questions = new Map<string, HostFrame>()
  private submissions = new Map<string, HostFrame>()
  private accepted = new Map<string, { digest: string; bytes: number }>()
  private acceptedBytes = 0
  private consumedQueueRows = new Map<string, Set<string>>()
  private storageFailed = false

  constructor(private publish: (channel: string, ...args: unknown[]) => void,
    private entries: (tabId: string) => readonly QueueEntry[] = () => [],
    private storage?: SubmissionStorage) {}

  restore(tabIds: Set<string>): void {
    const state = this.storage?.load()
    for (const item of state?.submissions ?? []) {
      if (!tabIds.has(item.tabId)) continue
      const receipt = { ...item.receipt, cancelable: false }
      if (!receipt.committed && receipt.state !== 'cancelled' && !receipt.fallbackQueue?.consumed) {
        receipt.state = 'uncertain'
        receipt.warning = 'Workspace host restarted; delivery is uncertain. This message will not be sent again automatically.'
      }
      const key = `${item.tabId}:${receipt.id}`
      this.accepted.set(key, { digest: item.digest, bytes: item.bytes })
      this.acceptedBytes += item.bytes
      this.submissions.set(key, { kind: 'event', channel: 'session:submission', args: [item.tabId, receipt] })
    }
    for (const [tabId, rows] of state?.consumedRows ?? []) if (tabIds.has(tabId)) this.consumedQueueRows.set(tabId, new Set(rows))
  }

  private persist(strict = false): void {
    if (!this.storage) return
    try {
      this.storage.save({ submissions: [...this.accepted].map(([key, identity]) => {
        const [tabId, receipt] = this.submissions.get(key)!.args as [string, SubmissionUpdate & { text: string }]
        return { tabId, ...identity, receipt }
      }), consumedRows: [...this.consumedQueueRows].map(([tab, rows]) => [tab, [...rows]]) })
    } catch {
      if (!this.storageFailed) console.warn('Could not persist workspace submission state; new submissions blocked')
      this.storageFailed = true
    }
    if (strict && this.storageFailed) throw new Error('Submission persistence unavailable; restore storage and restart the workspace host')
  }

  observe(frame: HostFrame): void {
    if (frame.kind !== 'event' || typeof frame.channel !== 'string' || !Array.isArray(frame.args)) return
    const { channel, args } = frame
    if (GLOBAL_EVENTS.has(channel)) this.latest.set(channel, frame)
    if (TAB_EVENTS.has(channel)) this.latest.set(`${channel}:${args[0]}`, frame)
    if (QUESTIONS.has(channel)) {
      const request = channel === 'mcp-elicitation-request' ? args[0] : args[1]
      if (request && typeof request.requestId === 'string') this.questions.set(request.requestId, frame)
    }
    if (channel === 'session:submission' && args[1]?.id) {
      const key = `${args[0]}:${args[1].id}`
      const previous = this.submissions.get(key)?.args as unknown[] | undefined
      const receipt = { ...(previous?.[1] as SubmissionUpdate), ...args[1] }
      if ((previous?.[1] as SubmissionUpdate)?.committed) receipt.committed = true
      receipt.fallbackQueue = mergeFallbackQueue((previous?.[1] as SubmissionUpdate)?.fallbackQueue, args[1].fallbackQueue)
      this.submissions.set(key, { ...frame, args: [args[0], receipt] })
      if (!receipt.committed) this.reconcileCommitted(args[0], this.entries(args[0]))
      if (args[1].state === 'fallback') this.reconcileFallback(args[0], this.entries(args[0]))
      if (!receipt.committed) this.reconcileTranscript(args[0], this.entries(args[0]))
      this.persist()
    }
    if (channel === 'session:submission' && Array.isArray(args[1]?.known)) this.applyKnownSnapshot(args[0], new Set(args[1].known))
    if (channel === 'jsonl-entries' && Array.isArray(args[1])) {
      this.reconcileCommitted(args[0], args[1])
      this.reconcileFallback(args[0], args[1])
      if (this.reconcileTranscript(args[0], args[1])) this.persist()
    }
    if (channel === 'session:submission' && typeof args[1]?.capable === 'boolean') this.latest.set(`submission-capability:${args[0]}`, frame)
    if (channel === 'mcp-activity' && args[1]?.requestId) {
      const key = `mcp:${args[0]}:${args[1].requestId}`
      if (args[1].status === 'pending') this.latest.set(key, frame)
      else {
        this.latest.delete(key)
        if (['completed', 'denied'].includes(args[1].status)) this.resolve(args[1].requestId)
      }
    }
    if (channel === 'tab:closed') this.forgetTab(args[0])
    if (channel === 'mcp-elicitation-resolved') this.resolve(args[0])
  }

  private reconcileCommitted(tabId: string, entries: readonly QueueEntry[]): void {
    const uuids = new Set(entries.map(entry => entry.uuid).filter(Boolean))
    if (!uuids.size) return
    for (const frame of this.submissions.values()) {
      const [id, receipt] = frame.args as [string, SubmissionUpdate]
      if (id !== tabId || !receipt.uuid || receipt.committed || !uuids.has(receipt.uuid)) continue
      // Commit before broadcasting: the process-owned observer sees this event
      // recursively, and replicas may receive it before the original receipt.
      receipt.committed = true
      this.publish('session:submission', tabId, receipt)
    }
  }

  private pendingFor(tabId: string): SubmissionUpdate[] {
    return [...this.submissions.values()].map(frame => frame.args as [string, SubmissionUpdate])
      .filter(([id]) => id === tabId).map(([, receipt]) => receipt)
  }

  /** Receipts the server can no longer settle (restart, lost read receipt,
   *  host restore) are committed by the prompt the CLI wrote after them. */
  private reconcileTranscript(tabId: string, entries: readonly QueueEntry[]): boolean {
    const receipts = this.pendingFor(tabId)
    if (!entries.length || !receipts.some(receipt => !receipt.committed && !receipt.uuid)) return false
    const used = this.consumedQueueRows.get(tabId) ?? new Set<string>()
    // Rows a stored UUID already claims stay out of the match, but are not persisted.
    const claimed = new Set(used)
    for (const receipt of receipts) if (receipt.uuid) claimed.add(`uuid:${receipt.uuid}`)
    const matches = matchReceiptsToTranscript(receipts.map(receipt => ({ ...receipt,
      afterOrd: receipt.fallbackQueue?.afterOrd ?? -1 })), entries, claimed)
    if (!matches.length) return false
    this.consumedQueueRows.set(tabId, used)
    for (const { id, entry } of matches) {
      const identity = queueRowIdentity(entry)
      if (identity) used.add(identity)
      const receipt = receipts.find(receipt => receipt.id === id)!
      receipt.committed = true // before publish: the observer sees it recursively
      this.publish('session:submission', tabId, receipt)
    }
    return true
  }

  /** Reattach snapshot: pending receipts the server process does not track
   *  will never be updated again; stop offering Cancel for them. */
  private applyKnownSnapshot(tabId: string, known: ReadonlySet<string>): void {
    const lost = this.pendingFor(tabId).filter(receipt => lostAfterSnapshot(receipt, known))
    for (const receipt of lost) {
      Object.assign(receipt, { state: 'uncertain', cancelable: false, warning: LOST_RECEIPT_WARNING })
      this.publish('session:submission', tabId, receipt)
    }
    if (this.reconcileTranscript(tabId, this.entries(tabId)) || lost.length) this.persist()
  }
  /** Called once at host ingress. Never re-send accepted submissions on attach. */
  accept(frame: ClientFrame): boolean {
    if (frame.kind === 'invoke' && frame.channel === 'workspace:queue-dismiss') {
      const [tabId, ids] = frame.args
      if (typeof tabId !== 'string' || !Array.isArray(ids) || ids.length > 256 ||
          ids.some(id => typeof id !== 'string' || !id || id.length > 1024)) throw new Error('Expected queue item IDs')
      this.dismissQueue(tabId, ids)
      return true
    }
    if (frame.kind !== 'send') return true
    const { channel, args } = frame
    if (channel === 'session:interrupt' && typeof args[0] === 'string' && Array.isArray(args[1])) {
      this.dismissQueue(args[0], args[1].filter((id: unknown) => typeof id === 'string'))
    }
    if (channel === 'pty-submit-text' && typeof args[0] === 'string' && typeof args[1] === 'string' && typeof args[2] === 'string') {
      if (this.storageFailed) throw new Error('Submission persistence unavailable; restore storage and restart the workspace host')
      if (!args[2] || args[2].length > 1024) throw new Error('Expected a submission identity')
      const key = `${args[0]}:${args[2]}`
      const digest = createHash('sha256').update(args[1]).digest('hex')
      const prior = this.accepted.get(key)
      if (prior !== undefined) {
        if (prior.digest !== digest) throw new Error('Submission identity already used for different text')
        return false
      }
      // Never evict an identity and then silently execute a duplicate. Bound
      // process memory by rejecting new identities at the limit instead.
      const bytes = Buffer.byteLength(args[1])
      if (this.accepted.size >= 10000 || this.acceptedBytes + bytes > 32 * 1024 * 1024) throw new Error('Submission limit reached; end a session before sending more')
      this.accepted.set(key, { digest, bytes })
      this.acceptedBytes += bytes
      const receipt: SubmissionUpdate = { id: args[2], text: args[1], state: 'queued', cancelable: true,
        fallbackQueue: { afterOrd: queueWatermark(this.entries(args[0])), enqueuedAt: Date.now() } }
      this.submissions.set(key, { kind: 'event', channel: 'session:submission', args: [args[0], receipt] })
      this.persist(true) // The duplicate barrier must reach disk BEFORE handler dispatch.
      this.publish('session:submission', args[0], receipt)
    }
    if (['permission-response', 'elicitation-response', 'ask-user-response'].includes(channel)) return this.resolve(args[0])
    if (['mcp-respond', 'mcp-deny'].includes(channel)) this.resolve(args[0])
    if (channel.startsWith('mcp-elicitation-response:')) return this.resolve(channel.slice('mcp-elicitation-response:'.length))
    return true
  }

  private dismissQueue(tabId: string, ids: string[]): void {
    for (const id of new Set(ids)) {
      const receipt = (this.submissions.get(`${tabId}:${id}`)?.args as [string, SubmissionUpdate] | undefined)?.[1]
      if (!receipt?.fallbackQueue || receipt.fallbackQueue.consumed || receipt.fallbackQueue.dismissed) continue
      // Это сброс отображаемой очереди, не подтверждение отмены доставки CLI.
      receipt.fallbackQueue = { ...receipt.fallbackQueue, dismissed: true }
      this.publish('session:submission', tabId, receipt)
    }
  }

  private reconcileFallback(tabId: string, entries: readonly QueueEntry[]): void {
    const pending = [...this.submissions.values()].map(frame => frame.args as [string, SubmissionUpdate])
      .filter(([id, receipt]) => id === tabId && receipt.state === 'fallback' && receipt.fallbackQueue && !receipt.fallbackQueue.consumed)
      .map(([, receipt]) => receipt)
    const used = this.consumedQueueRows.get(tabId) ?? new Set<string>()
    const matches = matchSubmissionQueue(pending.map(receipt => ({ id: receipt.id, content: receipt.text ?? '',
      afterOrd: receipt.fallbackQueue!.afterOrd, enqueuedAt: receipt.fallbackQueue!.enqueuedAt })), entries, used)
    if (!matches.length) return
    this.consumedQueueRows.set(tabId, used)
    for (const { item, entry } of matches) {
      const identity = queueRowIdentity(entry)
      if (identity) used.add(identity)
      const receipt = pending.find(receipt => receipt.id === item.id)!
      // Публикация проходит через observer повторно; committed marker ставится
      // до неё, чтобы sync и рекурсивный broadcast не воскресили legacy queue.
      receipt.fallbackQueue = { ...receipt.fallbackQueue!, consumed: true }
      this.publish('session:submission', tabId, receipt)
    }
  }

  private resolve(id: unknown): boolean {
    if (typeof id !== 'string' || !this.questions.delete(id)) return false
    this.publish('workspace:question-resolved', id)
    return true
  }

  private forgetTab(tabId: unknown): void {
    this.consumedQueueRows.delete(String(tabId))
    for (const [key, frame] of this.latest) if ((frame.args as unknown[])?.[0] === tabId && !GLOBAL_EVENTS.has(String(frame.channel))) this.latest.delete(key)
    for (const [id, frame] of this.questions) if ((frame.args as unknown[])?.[0] === tabId) this.resolve(id)
    for (const [key, frame] of this.submissions) if ((frame.args as unknown[])?.[0] === tabId) this.submissions.delete(key)
    for (const [key, value] of this.accepted) if (key.startsWith(`${tabId}:`)) {
      this.acceptedBytes -= value.bytes
      this.accepted.delete(key)
    }
    this.persist()
  }

  forgetSessions(): void {
    for (const id of [...this.questions.keys()]) this.resolve(id)
    for (const [key, frame] of this.latest) if (!GLOBAL_EVENTS.has(String(frame.channel))) this.latest.delete(key)
    this.submissions.clear()
    this.accepted.clear()
    this.acceptedBytes = 0
    this.consumedQueueRows.clear()
    this.persist()
  }

  snapshot(): HostFrame[] {
    return [...this.latest.values(), ...this.submissions.values(), ...this.questions.values()]
  }
}

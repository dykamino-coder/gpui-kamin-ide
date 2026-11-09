import type { ClientFrame, HostFrame } from './protocol.js'
import { createHash } from 'node:crypto'
import { matchSubmissionQueue, queueRowIdentity, queueWatermark, type QueueEntry, type SubmissionUpdate } from '../../webview/shared/submission-queue.js'

const TAB_EVENTS = new Set(['session:activity', 'session:usage', 'session:title', 'connection-state'])
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

  constructor(private publish: (channel: string, ...args: unknown[]) => void,
    private entries: (tabId: string) => readonly QueueEntry[] = () => []) {}

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
      this.submissions.set(key, { ...frame, args: [args[0], { ...(previous?.[1] as object), ...args[1] }] })
      if (args[1].state === 'fallback') this.reconcileFallback(args[0], this.entries(args[0]))
    }
    if (channel === 'jsonl-entries' && Array.isArray(args[1])) this.reconcileFallback(args[0], args[1])
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

  /** Called once at host ingress. Never re-send accepted submissions on attach. */
  accept(frame: ClientFrame): boolean {
    if (frame.kind !== 'send') return true
    const { channel, args } = frame
    if (channel === 'pty-submit-text' && typeof args[0] === 'string' && typeof args[1] === 'string' && typeof args[2] === 'string') {
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
      this.publish('session:submission', args[0], { id: args[2], text: args[1], state: 'queued', cancelable: true,
        fallbackQueue: { afterOrd: queueWatermark(this.entries(args[0])), enqueuedAt: Date.now() } })
    }
    if (['permission-response', 'elicitation-response', 'ask-user-response'].includes(channel)) return this.resolve(args[0])
    if (['mcp-respond', 'mcp-deny'].includes(channel)) this.resolve(args[0])
    if (channel.startsWith('mcp-elicitation-response:')) return this.resolve(channel.slice('mcp-elicitation-response:'.length))
    return true
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
  }

  forgetSessions(): void {
    for (const id of [...this.questions.keys()]) this.resolve(id)
    for (const [key, frame] of this.latest) if (!GLOBAL_EVENTS.has(String(frame.channel))) this.latest.delete(key)
    this.submissions.clear()
    this.accepted.clear()
    this.acceptedBytes = 0
    this.consumedQueueRows.clear()
  }

  snapshot(): HostFrame[] {
    return [...this.latest.values(), ...this.submissions.values(), ...this.questions.values()]
  }
}

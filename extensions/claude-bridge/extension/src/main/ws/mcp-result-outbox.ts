// Retain serialized MCP outcomes until the owning server session acknowledges
// settlement. Retry responses, never tools; bound memory and disconnected lifetime.
import { mcpDiagnostic } from './mcp-diagnostics'
import type { ClientMessage } from '../../shared/mcp-protocol'

export type McpOutcome = Extract<ClientMessage, { type: 'mcp:response' | 'mcp:denied' }>
type Slot = { taskOwner?: string; sessionId: string; frame: string; bytes: number; lastSentAt?: number; expires: number; promise: Promise<McpOutcome>; resolve: (outcome: McpOutcome) => void; reject: (error: Error) => void }
export const MCP_RESULT_LIMITS = { count: 64, bytes: 16 * 1024 * 1024, frameBytes: 4 * 1024 * 1024, lifetimeMs: 120_000, retryMs: 1000 }

export class McpResultOutbox {
  private slots = new Map<string, Slot>()
  private bytes = 0
  private sessionId: string | null = null
  private taskOwner: string | undefined
  private timer: ReturnType<typeof setInterval> | null = null
  constructor(private sendFrame: (frame: string) => boolean) {}

  deliver(sessionId: string, outcome: McpOutcome): Promise<McpOutcome> {
    const existing = this.slots.get(outcome.requestId)
    if (existing) return existing.taskOwner === outcome.taskOwner && (existing.sessionId === sessionId || !!outcome.taskOwner) ? existing.promise : Promise.reject(new Error('MCP result session mismatch'))
    if (outcome.taskOwner && this.sessionId && outcome.taskOwner !== this.taskOwner) return Promise.reject(new Error('MCP result task owner mismatch'))
    if (outcome.taskOwner && outcome.taskOwner === this.taskOwner && this.sessionId) sessionId = this.sessionId
    if (this.slots.size >= MCP_RESULT_LIMITS.count) return Promise.reject(new Error('MCP result delivery count capacity exceeded'))
    let frame: string
    // Reserve room for one compact failure per slot even when payload storage
    // is saturated. A finished tool must still answer its server request.
    const payloadBudget = MCP_RESULT_LIMITS.bytes - MCP_RESULT_LIMITS.count * 1024
    const failure = (reason: string): McpOutcome => ({
      type: 'mcp:response', requestId: outcome.requestId, taskOwner: outcome.taskOwner,
      result: { isError: true, content: [{ type: 'text', text: `Error: ${reason}. The tool may already have completed; inspect its effects before retrying. For reads, request a smaller range or narrower query.` }] },
    })
    try { frame = JSON.stringify({ ...outcome, sessionId }) } catch {
      outcome = failure('Tool result cannot be serialized')
      frame = JSON.stringify({ ...outcome, sessionId })
    }
    if (Buffer.byteLength(frame) > MCP_RESULT_LIMITS.frameBytes || this.bytes + Buffer.byteLength(frame) > payloadBudget) {
      outcome = failure('Tool result exceeds the bounded delivery byte budget')
      frame = JSON.stringify({ ...outcome, sessionId })
    }
    const bytes = Buffer.byteLength(frame)
    if (bytes > MCP_RESULT_LIMITS.frameBytes || this.bytes + bytes > MCP_RESULT_LIMITS.bytes) {
      return Promise.reject(new Error('MCP result delivery capacity exceeded'))
    }
    let resolve!: (outcome: McpOutcome) => void
    let reject!: (error: Error) => void
    const promise = new Promise<McpOutcome>((ok, fail) => { resolve = ok; reject = fail })
    this.slots.set(outcome.requestId, { taskOwner: outcome.taskOwner, sessionId, frame, bytes, expires: Date.now() + (outcome.taskOwner ? 86_400_000 : MCP_RESULT_LIMITS.lifetimeMs), promise, resolve, reject })
    this.bytes += bytes
    mcpDiagnostic('result-queued', { requestId: outcome.requestId, sessionId, bytes, queuedBytes: this.bytes, queuedCount: this.slots.size })
    this.timer ??= setInterval(() => this.flush(), MCP_RESULT_LIMITS.retryMs)
    this.timer.unref?.()
    this.flush()
    return promise
  }

  attach(sessionId: string, taskOwner?: string): void {
    this.taskOwner = taskOwner
    this.sessionId = sessionId
    mcpDiagnostic('transport-attached', { sessionId, queuedCount: this.slots.size, queuedBytes: this.bytes })
    for (const [id, slot] of this.slots) {
      if (slot.taskOwner && slot.taskOwner === taskOwner) {
        const frame = JSON.stringify({ ...JSON.parse(slot.frame), sessionId })
        const bytes = Buffer.byteLength(frame)
        if (bytes > MCP_RESULT_LIMITS.frameBytes || this.bytes - slot.bytes + bytes > MCP_RESULT_LIMITS.bytes) {
          this.finish(id, new Error('MCP result reattach capacity exceeded'))
          continue
        }
        this.bytes += bytes - slot.bytes
        Object.assign(slot, { sessionId, frame, bytes, lastSentAt: undefined })
      } else if (slot.taskOwner || slot.sessionId !== sessionId) this.finish(id, new Error('MCP result belongs to an ended server session'))
      else slot.lastSentAt = undefined
    }
    this.flush()
  }

  detach(): void {
    mcpDiagnostic('transport-detached', { sessionId: this.sessionId ?? undefined, queuedCount: this.slots.size, queuedBytes: this.bytes })
    this.sessionId = null
  }

  acknowledge(sessionId: string, requestId: string, accepted: boolean): void {
    const slot = this.slots.get(requestId)
    if (!slot || sessionId !== this.sessionId || sessionId !== slot.sessionId) return
    this.finish(requestId, accepted ? undefined : new Error('Server no longer accepts this MCP result'))
  }

  dispose(): void {
    this.detach()
    for (const id of this.slots.keys()) this.finish(id, new Error('MCP result delivery cancelled: session ended'))
  }

  private flush(): void {
    for (const [id, slot] of this.slots) {
      if (Date.now() >= slot.expires) { this.finish(id, new Error('MCP result acknowledgement timed out')); continue }
      if (this.sessionId !== slot.sessionId || (slot.taskOwner !== undefined && slot.taskOwner !== this.taskOwner)) continue
      if (slot.lastSentAt !== undefined && Date.now() - slot.lastSentAt < MCP_RESULT_LIMITS.retryMs) continue
      try {
        const enqueued = this.sendFrame(slot.frame)
        mcpDiagnostic(enqueued ? 'result-enqueued' : 'result-backpressured', { requestId: id, sessionId: slot.sessionId, bytes: slot.bytes })
        if (enqueued) slot.lastSentAt = Date.now()
      } catch { /* retain across close/send races */ }
    }
  }

  private finish(id: string, error?: Error): void {
    const slot = this.slots.get(id)
    if (!slot) return
    mcpDiagnostic(error ? 'result-delivery-failed' : 'result-acknowledged', { requestId: id, sessionId: slot.sessionId })
    this.slots.delete(id)
    this.bytes -= slot.bytes
    if (this.slots.size === 0 && this.timer) { clearInterval(this.timer); this.timer = null }
    if (error) slot.reject(error)
    else slot.resolve(JSON.parse(slot.frame) as McpOutcome)
  }
}

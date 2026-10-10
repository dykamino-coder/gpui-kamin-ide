// Retain serialized MCP outcomes until the owning server session acknowledges
// settlement. Retry responses, never tools; bound memory and disconnected lifetime.
import type { ClientMessage } from '../../shared/mcp-protocol'

type Outcome = Extract<ClientMessage, { type: 'mcp:response' | 'mcp:denied' }>
type Slot = { sessionId: string; frame: string; bytes: number; expires: number; promise: Promise<void>; resolve: () => void; reject: (error: Error) => void }
export const MCP_RESULT_LIMITS = { count: 64, bytes: 16 * 1024 * 1024, frameBytes: 4 * 1024 * 1024, lifetimeMs: 120_000, retryMs: 1000 }

export class McpResultOutbox {
  private slots = new Map<string, Slot>()
  private bytes = 0
  private sessionId: string | null = null
  private timer: ReturnType<typeof setInterval> | null = null
  constructor(private sendFrame: (frame: string) => boolean) {}

  deliver(sessionId: string, outcome: Outcome): Promise<void> {
    const existing = this.slots.get(outcome.requestId)
    if (existing) return existing.sessionId === sessionId ? existing.promise : Promise.reject(new Error('MCP result session mismatch'))
    let frame: string
    try { frame = JSON.stringify({ ...outcome, sessionId }) } catch { return Promise.reject(new Error('MCP result cannot be serialized')) }
    const bytes = Buffer.byteLength(frame)
    if (bytes > MCP_RESULT_LIMITS.frameBytes || this.bytes + bytes > MCP_RESULT_LIMITS.bytes || this.slots.size >= MCP_RESULT_LIMITS.count) {
      return Promise.reject(new Error('MCP result delivery capacity exceeded'))
    }
    let resolve!: () => void
    let reject!: (error: Error) => void
    const promise = new Promise<void>((ok, fail) => { resolve = ok; reject = fail })
    this.slots.set(outcome.requestId, { sessionId, frame, bytes, expires: Date.now() + MCP_RESULT_LIMITS.lifetimeMs, promise, resolve, reject })
    this.bytes += bytes
    this.timer ??= setInterval(() => this.flush(), MCP_RESULT_LIMITS.retryMs)
    this.timer.unref?.()
    this.flush()
    return promise
  }

  attach(sessionId: string): void {
    this.sessionId = sessionId
    for (const [id, slot] of this.slots) {
      if (slot.sessionId !== sessionId) this.finish(id, new Error('MCP result belongs to an ended server session'))
    }
    this.flush()
  }

  detach(): void { this.sessionId = null }

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
      if (this.sessionId !== slot.sessionId) continue
      try { this.sendFrame(slot.frame) } catch { /* retain across close/send races */ }
    }
  }

  private finish(id: string, error?: Error): void {
    const slot = this.slots.get(id)
    if (!slot) return
    this.slots.delete(id)
    this.bytes -= slot.bytes
    if (this.slots.size === 0 && this.timer) { clearInterval(this.timer); this.timer = null }
    if (error) slot.reject(error)
    else slot.resolve()
  }
}

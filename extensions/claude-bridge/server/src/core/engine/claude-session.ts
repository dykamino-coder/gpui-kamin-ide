// ============================================================================
// Claude CLI behind the engine-neutral AgentSession contract.
//
// A thin view over an existing PTY session: every verb delegates to the same
// session-core / claude-turn functions the session WebSocket calls, so the
// Claude path keeps its behaviour. Chat content still travels as native CLI
// JSONL (normalizedTranscript: false); `events` carries lifecycle only.
// Text sent during a turn is queued by the CLI (nativeSteer: false).
// ============================================================================

import type { AgentEvent } from '../../shared/agent-events'
import type { DestroyReason } from '../logging/lifecycle'
import type { AgentCapabilities, AgentSession, SetModelOutcome } from './agent-session'
import { EventStream } from './event-stream'

export interface ClaudeSessionOps {
  conversationId(sessionId: string): string | null
  /** False when the session no longer exists. */
  exists(sessionId: string): boolean
  submitText(sessionId: string, text: string, submissionId?: string): void
  interrupt(sessionId: string): void
  /** In-place `/model` switch; false means a restart is required. */
  switchModel(sessionId: string, model: string): boolean
  destroy(sessionId: string, reason: DestroyReason): void
  /** Subscribes to the destruction of `sessionId`; returns an unsubscribe. */
  onDestroyed(sessionId: string, listener: (exitCode: number | null) => void): () => void
}

export const CLAUDE_CAPABILITIES: AgentCapabilities = Object.freeze({
  nativeSteer: false,
  liveModelSwitch: true,
  normalizedTranscript: false,
})

const DESTROY_REASONS: ReadonlySet<string> = new Set<DestroyReason>(['explicit_end', 'dashboard_kill', 'unspecified'])

export class ClaudeAgentSession implements AgentSession {
  readonly engine = 'claude' as const
  readonly capabilities = CLAUDE_CAPABILITIES
  readonly events = new EventStream<AgentEvent>()
  private readonly offDestroyed: () => void
  private stopReason: string | undefined

  constructor(
    readonly id: string,
    private readonly ops: ClaudeSessionOps,
    private readonly now: () => number = Date.now,
  ) {
    this.offDestroyed = ops.onDestroyed(id, (exitCode) => this.exited(exitCode))
  }

  conversationId(): string | null {
    return this.ops.conversationId(this.id)
  }

  private live(): void {
    if (this.events.closed || !this.ops.exists(this.id)) throw new Error(`Claude session ${this.id} is not running`)
  }

  async send(text: string, submissionId?: string): Promise<void> {
    this.live()
    this.ops.submitText(this.id, text, submissionId)
  }

  /** The CLI has no steer channel: input during a turn is queued by it. */
  steer(text: string, submissionId?: string): Promise<void> {
    return this.send(text, submissionId)
  }

  async interrupt(): Promise<void> {
    if (this.events.closed) return
    this.ops.interrupt(this.id)
  }

  async setModel(model: string): Promise<SetModelOutcome> {
    this.live()
    return this.ops.switchModel(this.id, model) ? 'applied' : 'restart-required'
  }

  async stop(reason = 'explicit_end'): Promise<void> {
    if (this.events.closed) return
    this.stopReason = reason
    if (this.ops.exists(this.id)) {
      this.ops.destroy(this.id, (DESTROY_REASONS.has(reason) ? reason : 'unspecified') as DestroyReason)
    }
    this.exited(null)
  }

  private exited(exitCode: number | null): void {
    if (this.events.closed) return
    this.offDestroyed()
    this.events.close({
      kind: 'session.exited',
      threadId: this.conversationId() ?? this.id,
      at: this.now(),
      code: exitCode,
      signal: null,
      ...(this.stopReason ? { reason: this.stopReason } : {}),
    })
  }
}

// ============================================================================
// Engine-neutral agent session contract.
//
// Bridge drives Claude CLI (PTY + JSONL) and codex app-server (JSON-RPC) through
// the same verbs. What differs is surfaced as capability flags and outcomes,
// never emulated: e.g. Claude has no separate steer channel (text typed during
// a turn is queued by the CLI), and its chat content travels as native JSONL,
// so its normalized stream carries lifecycle events only.
// ============================================================================

import type { AgentEngineId, AgentEvent } from '../../shared/agent-events'
import type { EventStream } from './event-stream'

export type { AgentEngineId, AgentEvent }

export interface AgentCapabilities {
  /** `steer` reaches the running turn instead of queueing a new one. */
  nativeSteer: boolean
  /** Model can change without restarting the engine process. */
  liveModelSwitch: boolean
  /** Chat content is published on `events` (else it travels as Claude JSONL). */
  normalizedTranscript: boolean
}

/**
 * - `applied`: the running engine now uses the model;
 * - `next-turn`: accepted, takes effect when the next turn starts;
 * - `restart-required`: the caller must respawn/resume with the model.
 */
export type SetModelOutcome = 'applied' | 'next-turn' | 'restart-required'

export interface AgentSession {
  readonly engine: AgentEngineId
  /** Bridge session id (Claude) or Bridge-assigned id of the codex session. */
  readonly id: string
  readonly capabilities: AgentCapabilities
  readonly events: EventStream<AgentEvent>
  /** Engine conversation/thread id once known (resume key). */
  conversationId(): string | null
  /** Submit a user message; starts a turn, or steers/queues if one is running. */
  send(text: string, submissionId?: string): Promise<void>
  /** Add input to the running turn; with no running turn behaves like `send`. */
  steer(text: string, submissionId?: string): Promise<void>
  /** Stop the running turn; a no-op when idle. */
  interrupt(): Promise<void>
  setModel(model: string): Promise<SetModelOutcome>
  /** Ends the session and its engine process. Idempotent. */
  stop(reason?: string): Promise<void>
}

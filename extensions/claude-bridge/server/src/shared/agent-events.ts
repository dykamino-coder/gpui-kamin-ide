// ============================================================================
// Normalized agent event model — engine-neutral chat stream.
//
// Claude sessions keep their native JSONL transcript (jsonl-types.ts); a
// non-Claude engine is projected into THIS model instead of forging Claude
// JSONL, so the webview can render either without one pretending to be the
// other. Events are append-only and keyed by stable item ids: a `*.delta`
// event may be followed by the final event for the same id, which replaces
// the accumulated deltas.
// ============================================================================

export type AgentEngineId = 'claude' | 'codex'

export interface AgentTokenUsage {
  totalTokens: number
  inputTokens: number
  cachedInputTokens: number
  outputTokens: number
  reasoningOutputTokens: number
}

export interface AgentToolRef {
  /** `mcp` = Bridge MCP tool; `dynamic` = client-registered tool; `builtin` = engine-native tool. */
  source: 'mcp' | 'dynamic' | 'builtin'
  server?: string
  name: string
}

export type AgentToolOutput =
  { type: 'text'; text: string } | { type: 'image'; url: string } | { type: 'json'; value: unknown }

export interface AgentErrorInfo {
  message: string
  details?: string
}

interface Base {
  /** Engine thread/conversation id. */
  threadId: string
  /** Milliseconds since epoch when the engine reported it, else when Bridge saw it. */
  at: number
}

export type AgentEvent = Base &
  (
    | { kind: 'session.started'; engine: AgentEngineId; model: string | null; resumed: boolean }
    | { kind: 'turn.started'; turnId: string }
    | {
        kind: 'turn.completed'
        turnId: string
        status: 'completed' | 'interrupted' | 'failed'
        error?: AgentErrorInfo
        durationMs?: number
      }
    | { kind: 'message.user'; id: string; turnId: string; text: string }
    | { kind: 'message.assistant.delta'; id: string; turnId: string; delta: string }
    | { kind: 'message.assistant'; id: string; turnId: string; text: string }
    | {
        kind: 'reasoning.delta'
        id: string
        turnId: string
        channel: 'summary' | 'content'
        index: number
        delta: string
      }
    | { kind: 'reasoning'; id: string; turnId: string; summary: string[]; content: string[] }
    | { kind: 'tool.started'; id: string; turnId: string; tool: AgentToolRef; input: unknown }
    | {
        kind: 'tool.completed'
        id: string
        turnId: string
        tool: AgentToolRef
        input: unknown
        status: 'completed' | 'failed' | 'declined'
        output: AgentToolOutput[]
        error?: string
        durationMs?: number
      }
    | { kind: 'context.compaction'; turnId: string; id?: string }
    | {
        kind: 'usage'
        turnId: string
        total: AgentTokenUsage
        last: AgentTokenUsage
        contextWindow: number | null
      }
    | { kind: 'model.rerouted'; turnId: string; from: string; to: string; reason: string }
    | { kind: 'notice'; level: 'warning' | 'error'; message: string; turnId?: string; willRetry?: boolean }
    /** An engine item Bridge does not render yet; surfaced instead of silently dropped. */
    | { kind: 'item.unsupported'; id?: string; turnId: string; itemType: string }
    | { kind: 'session.exited'; code: number | null; signal: string | null; reason?: string }
  )

export type AgentEventKind = AgentEvent['kind']

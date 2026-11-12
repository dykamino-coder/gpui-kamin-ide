// Engine ids Bridge can run as the main agent. The per-user default and the
// per-chat choice (fixed for the chat's lifetime) are wired in the next stage.
import type { AgentEngineId } from '../../shared/agent-events'

export const AGENT_ENGINES: readonly AgentEngineId[] = ['claude', 'codex']
export const DEFAULT_AGENT_ENGINE: AgentEngineId = 'claude'

export function isAgentEngineId(value: unknown): value is AgentEngineId {
  return typeof value === 'string' && (AGENT_ENGINES as readonly string[]).includes(value)
}

/** Unknown or missing values fall back to Claude, the existing behaviour. */
export function agentEngineOrDefault(value: unknown): AgentEngineId {
  return isAgentEngineId(value) ? value : DEFAULT_AGENT_ENGINE
}

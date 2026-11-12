// Which agent a KaminIDE session's chat runs on.
//
// The engine of a chat is fixed when the chat is created and is stored in the
// session's `metadata.bridge.engine`, next to its conversationId. A resumed chat
// keeps the engine of its transcript (the server decides; the stored value is
// only a hint). A brand-new session takes, in order: the engine written on it,
// the engine of a pending "open a GPT chat" request, the user's main agent.
import { isAgentEngine, type AgentEngine } from "./shared/types"

export interface BridgeMeta {
  conversationId?: string | null
  cwd?: string | null
  engine?: AgentEngine
  /** Model the chat was opened with (applied once on its first start). */
  model?: string
}

export interface AgentChoice {
  engine: AgentEngine
  model?: string
}

export function readBridgeMeta(metadata: Record<string, unknown> | undefined): BridgeMeta | null {
  const raw = metadata?.bridge
  if (!raw || typeof raw !== "object") return null
  const meta = raw as Record<string, unknown>
  return {
    conversationId: typeof meta.conversationId === "string" ? meta.conversationId : null,
    cwd: typeof meta.cwd === "string" ? meta.cwd : null,
    ...(isAgentEngine(meta.engine) ? { engine: meta.engine } : {}),
    ...(typeof meta.model === "string" && meta.model ? { model: meta.model } : {}),
  }
}

/** Agent for a session that has no conversation yet. */
export function agentForNewChat(meta: BridgeMeta | null, pending: AgentChoice | null, defaultEngine: AgentEngine): AgentChoice {
  if (meta?.engine) return { engine: meta.engine, ...(meta.model ? { model: meta.model } : {}) }
  if (pending) return pending
  return { engine: defaultEngine }
}

/** Metadata to write when the chat's conversation id becomes known. The
 *  `bridge` key is replaced as a whole, so the engine has to travel along. */
export function bridgeMetaFor(conversationId: string | null, cwd: string | null, engine: AgentEngine | undefined): BridgeMeta {
  return { conversationId, cwd, ...(engine ? { engine } : {}) }
}

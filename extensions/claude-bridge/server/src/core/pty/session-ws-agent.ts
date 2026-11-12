// Engine-level WS requests that are not bound to one chat: the user's main
// agent and the GPT model list for the selector. Replies are non-fatal — the
// socket and any session on it stay as they are.
import type { WebSocket as WS } from 'ws'
import { resolveToken } from '../auth/tokens'
import { classifyCodexError } from '../codex/codex-errors'
import { codexRuntime, type CodexRuntime } from '../codex/codex-runtime'
import { getDefaultEngine, setDefaultEngine } from '../engine/agent-preferences'
import { isAgentEngineId } from '../engine/engines'
import { sendToClient } from './session-io'

export type AgentWsMessage =
  { type: 'agent:set-default-engine'; engine: unknown } | { type: 'codex:models'; token?: string }

export interface AgentWsDeps {
  runtime?: () => CodexRuntime
  resolveToken?: (token: string) => Promise<{ tokenId: string } | null>
  setDefaultEngine?: (tokenId: string, engine: 'claude' | 'codex') => Promise<void>
}

export async function handleAgentWsMessage(
  ws: WS,
  msg: AgentWsMessage,
  authenticatedTokenId: string | null,
  deps: AgentWsDeps = {},
): Promise<void> {
  if (msg.type === 'agent:set-default-engine') {
    if (!authenticatedTokenId) {
      sendToClient(ws, { type: 'agent:preferences', error: 'Not authenticated — send session:create first' })
      return
    }
    if (!isAgentEngineId(msg.engine)) {
      sendToClient(ws, { type: 'agent:preferences', error: 'Unknown engine' })
      return
    }
    await (deps.setDefaultEngine ?? setDefaultEngine)(authenticatedTokenId, msg.engine)
    sendToClient(ws, { type: 'agent:preferences', defaultEngine: msg.engine })
    return
  }

  let tokenId = authenticatedTokenId
  if (!tokenId && typeof msg.token === 'string') {
    tokenId = (await (deps.resolveToken ?? resolveToken)(msg.token))?.tokenId ?? null
  }
  if (!tokenId) {
    sendToClient(ws, { type: 'codex:models', models: [], error: { kind: 'other', message: 'Invalid token' } })
    return
  }
  const runtime = (deps.runtime ?? codexRuntime)()
  // No codex on this server: the client shows the ChatGPT group as unavailable.
  const availability = await runtime.availability()
  if (!availability.available) {
    sendToClient(ws, { type: 'codex:models', models: [], unavailable: availability.reason })
    return
  }
  try {
    const [models, account] = await Promise.all([runtime.models(), runtime.accountStatus()])
    sendToClient(ws, { type: 'codex:models', models, account })
  } catch (error) {
    sendToClient(ws, { type: 'codex:models', models: [], error: classifyCodexError((error as Error).message) })
  }
}

/** Tells the client the user's main agent, off the startup path (no await there). */
export function sendAgentPreferences(ws: WS, tokenId: string): void {
  void getDefaultEngine(tokenId)
    .then((defaultEngine) => sendToClient(ws, { type: 'agent:preferences', defaultEngine }))
    .catch(() => undefined)
}

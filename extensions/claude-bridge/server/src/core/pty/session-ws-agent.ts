// Engine-level WS requests that are not bound to one chat: the user's main
// agent and the GPT model list for the selector. Replies are non-fatal — the
// socket and any session on it stay as they are.
//
// `agent:preferences` carries the main agent and which engines this server is
// signed in to (`auth`); it is pushed after session:created, after every GPT
// list request and to every open socket when a sign-in changes.
import type { WebSocket as WS } from 'ws'
import { resolveToken } from '../auth/tokens'
import { classifyCodexError } from '../codex/codex-errors'
import { codexRuntime, type CodexRuntime } from '../codex/codex-runtime'
import { getDefaultEngine, setDefaultEngine } from '../engine/agent-preferences'
import { engineAuthState, mainAgentRefusal, type ClaudeAuthMonitor, type EngineAuthState } from '../engine/engine-auth'
import { isAgentEngineId } from '../engine/engines'
import { sendToClient } from './session-io'

export type AgentWsMessage =
  { type: 'agent:set-default-engine'; engine: unknown } | { type: 'codex:models'; token?: string }

export interface AgentWsDeps {
  runtime?: () => CodexRuntime
  claude?: ClaudeAuthMonitor
  resolveToken?: (token: string) => Promise<{ tokenId: string } | null>
  getDefaultEngine?: (tokenId: string) => Promise<'claude' | 'codex'>
  setDefaultEngine?: (tokenId: string, engine: 'claude' | 'codex') => Promise<void>
}

function authDeps(deps: AgentWsDeps) {
  return { runtime: (deps.runtime ?? codexRuntime)(), ...(deps.claude ? { claude: deps.claude } : {}) }
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
    // The main agent is never an engine this server cannot run.
    const refusal = await mainAgentRefusal(msg.engine, authDeps(deps))
    if (refusal) {
      await sendAgentPreferences(ws, authenticatedTokenId, deps, { error: refusal })
      return
    }
    await (deps.setDefaultEngine ?? setDefaultEngine)(authenticatedTokenId, msg.engine)
    await sendAgentPreferences(ws, authenticatedTokenId, deps)
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
  } else {
    try {
      const [models, account] = await Promise.all([runtime.models(), runtime.accountStatus()])
      sendToClient(ws, { type: 'codex:models', models, account })
    } catch (error) {
      sendToClient(ws, { type: 'codex:models', models: [], error: classifyCodexError((error as Error).message) })
    }
  }
  // The selector just opened: a fresh sign-in state for both engines.
  await sendAgentPreferences(ws, tokenId, deps, {}, true)
}

/** Main agent + sign-in state of both engines. `probe` asks codex (starting
 *  its app-server); without it only an already known ChatGPT state is sent. */
export async function agentPreferencesPayload(
  tokenId: string,
  deps: AgentWsDeps = {},
  probe = false,
): Promise<{ defaultEngine: 'claude' | 'codex'; auth: EngineAuthState }> {
  const [defaultEngine, auth] = await Promise.all([
    (deps.getDefaultEngine ?? getDefaultEngine)(tokenId),
    engineAuthState(authDeps(deps), probe),
  ])
  return { defaultEngine, auth }
}

/** Tells the client the user's main agent and the engines' sign-in state. */
export async function sendAgentPreferences(
  ws: WS,
  tokenId: string,
  deps: AgentWsDeps = {},
  extra: { error?: string } = {},
  probe = false,
): Promise<void> {
  try {
    const payload = await agentPreferencesPayload(tokenId, deps, probe)
    sendToClient(ws, { type: 'agent:preferences', ...payload, ...extra })
  } catch {
    /* the next push or selector request repeats it */
  }
}

/** A sign-in changed: every open socket gets the new state. */
export async function broadcastAgentPreferences(
  sockets: ReadonlyMap<string, ReadonlySet<WS>>,
  deps: AgentWsDeps = {},
): Promise<void> {
  // Probe codex only once it has an app-server (knownAccount); a Claude-only
  // server never starts one for this.
  const auth = await engineAuthState(authDeps(deps), false)
  await Promise.all(
    [...sockets].map(async ([tokenId, set]) => {
      const defaultEngine = await (deps.getDefaultEngine ?? getDefaultEngine)(tokenId).catch(() => 'claude' as const)
      for (const ws of set) sendToClient(ws, { type: 'agent:preferences', defaultEngine, auth })
    }),
  )
}

// Disconnect logic + transport-agnostic JSON-RPC dispatcher — extracted from
// `manager.ts` (Sprint 2 / Stage C, C3). Pure functions over the per-server
// state struct; the manager class delegates here so connection teardown
// stays isolated from lifecycle / OAuth / message-routing concerns.

import type { McpServerState, TransportContext } from './transports/context'
import { wsJsonRpc } from './transports/ws'
import { sseJsonRpcRequest } from './transports/sse-legacy'
import { httpJsonRpcAuth } from './transports/http'
import { stdioJsonRpc } from './transports/stdio'

/** Tear down a server's transport + clear all in-flight state. Idempotent —
 *  safe to call when the server is already disconnected (state mutation is
 *  the same in both cases). Resolves all pending RPC promises with a
 *  "Server disconnected" error so callers don't hang forever. */
export function teardownServer(state: McpServerState | undefined): void {
  if (!state) return

  if (state.process) {
    state.process.kill()
    state.process = undefined
  }
  if (state.ws) {
    try { state.ws.close() } catch { /* already closed */ }
    state.ws = undefined
  }
  if (state.sseAbort) {
    try { state.sseAbort.abort() } catch { /* already aborted */ }
    state.sseAbort = undefined
    state.sseMessageEndpoint = undefined
  }

  for (const pending of state.pendingRequests.values()) {
    if (pending.timer) clearTimeout(pending.timer)
    pending.reject(new Error('Server disconnected'))
  }
  state.pendingRequests.clear()
  state.status = 'disconnected'
  state.tools = []
  state.toolSchemas.clear()
  state.resources = []
  state.resourceTemplates = []
  state.prompts = []
  state.capabilities = {}
  state.inputBuffer = ''
}

/** Transport-agnostic JSON-RPC request dispatcher. Routes the call to the
 *  matching transport (stdio / http / sse / ws) based on the server's type.
 *  Bumps `state.nextRequestId` so concurrent calls each get a unique id. */
export async function dispatchRpcRequest(
  ctx: TransportContext,
  id: string,
  method: string,
  params: Record<string, unknown>,
): Promise<unknown> {
  const state = ctx.servers.get(id)!
  const body = { jsonrpc: '2.0', id: state.nextRequestId++, method, params }
  if (state.config.type === 'ws') return wsJsonRpc(ctx, id, body)
  if (state.config.type === 'sse') return sseJsonRpcRequest(ctx, id, body)
  if (state.config.type === 'http') {
    const res = await httpJsonRpcAuth(ctx, id, body)
    if (res.error) throw new Error(res.error.message)
    return res.result
  }
  return stdioJsonRpc(ctx, id, method, params)
}

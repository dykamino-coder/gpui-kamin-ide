// Deadlines for JSON-RPC requests we send to external MCP servers, shared by
// every transport that matches replies through `state.pendingRequests`.
//
// Service methods (tools/list, resources/read, ...) answer from the server's
// own state and keep a short deadline. A tool call may wait for a person: a
// server can show its own picker, or ask through `elicitation/create`, which
// Bridge keeps open for MCP_ELICITATION_TIMEOUT_MS. A tool-call deadline at or below that
// window turned a slow but valid answer into a timeout while the person was
// still choosing. Like the MCP TypeScript SDK (`resetTimeoutOnProgress` +
// `maxTotalTimeout`), a tool call asks for progress and every
// `notifications/progress` for it restarts the idle clock; the total cap still
// bounds a server that reports progress forever.

import type { McpPendingRequest, McpServerState } from './context'

/** How long an `elicitation/create` dialog waits for the person. */
export const MCP_ELICITATION_TIMEOUT_MS = 10 * 60_000
/** Service requests: initialize-time listing, resources, prompts, refetches. */
export const MCP_REQUEST_TIMEOUT_MS = 120_000
/** Silence allowed during a tool call. Longer than the 10-minute elicitation
 *  window, so a person's answer (or the dialog's own dismissal) arrives first. */
export const MCP_TOOL_CALL_IDLE_TIMEOUT_MS = 15 * 60_000
/** Absolute cap of one tool call, regardless of progress notifications. The
 *  Bridge server waits slightly longer so this host's specific error wins. */
export const MCP_TOOL_CALL_MAX_TIMEOUT_MS = 30 * 60_000

export interface McpRequestDeadline {
  idleMs: number
  maxMs: number
}

export function mcpRequestDeadline(method: string): McpRequestDeadline {
  return method === 'tools/call'
    ? { idleMs: MCP_TOOL_CALL_IDLE_TIMEOUT_MS, maxMs: MCP_TOOL_CALL_MAX_TIMEOUT_MS }
    : { idleMs: MCP_REQUEST_TIMEOUT_MS, maxMs: MCP_REQUEST_TIMEOUT_MS }
}

/** `params._meta.progressToken` for a request: the SDK convention reuses the
 *  JSON-RPC id, which is already unique per server connection. */
export function withProgressToken(
  method: string,
  requestId: number,
  params: Record<string, unknown>,
): Record<string, unknown> {
  const { idleMs, maxMs } = mcpRequestDeadline(method)
  if (idleMs >= maxMs) return params
  const meta = params._meta && typeof params._meta === 'object' ? params._meta as Record<string, unknown> : {}
  return { ...params, _meta: { ...meta, progressToken: requestId } }
}

export interface AwaitMcpReplyOptions {
  method: string
  /** Owning call's cancellation (user interrupt or manager shutdown). */
  signal?: AbortSignal
  /** Tells the server the request is abandoned (`notifications/cancelled`). */
  cancel?: (requestId: number, reason: string) => void
}

function duration(ms: number): string {
  return ms % 60_000 === 0 ? `${ms / 60_000} min` : `${ms / 1000}s`
}

/**
 * Registers `requestId` in `state.pendingRequests` and resolves with whatever
 * the transport's reply router passes to `resolve`. Register before sending:
 * a reply cannot overtake a synchronous write, but the entry must exist when
 * it arrives. The entry removes itself on every outcome, so routers and
 * `teardownServer` may settle it directly.
 */
export function awaitMcpReply<T>(
  state: McpServerState,
  requestId: number,
  options: AwaitMcpReplyOptions,
): Promise<T> {
  const { method, signal, cancel } = options
  const { idleMs, maxMs } = mcpRequestDeadline(method)
  return new Promise<T>((resolve, reject) => {
    if (signal?.aborted) {
      reject(new Error('MCP request cancelled'))
      return
    }
    const startedAt = Date.now()
    let done = false
    // Reply routers may delete the entry before calling it; settle anyway.
    const settle = (): boolean => {
      if (done) return false
      done = true
      if (state.pendingRequests.get(requestId) === entry) state.pendingRequests.delete(requestId)
      if (entry.timer) clearTimeout(entry.timer)
      signal?.removeEventListener('abort', onAbort)
      return true
    }
    const abandon = (reason: string): void => {
      if (!settle()) return
      reject(new Error(reason))
      cancel?.(requestId, reason)
    }
    const arm = (): void => {
      if (entry.timer) clearTimeout(entry.timer)
      const remaining = maxMs - (Date.now() - startedAt)
      const capped = remaining <= idleMs
      entry.timer = setTimeout(() => abandon(capped
        ? `${method} timeout (${duration(maxMs)} total)`
        : `${method} timeout (no reply or progress for ${duration(idleMs)})`), Math.max(0, Math.min(idleMs, remaining)))
    }
    const onAbort = (): void => abandon('MCP request cancelled')
    const entry: McpPendingRequest = {
      resolve: (value) => { if (settle()) resolve(value as T) },
      reject: (error) => { if (settle()) reject(error) },
      progressToken: idleMs < maxMs ? requestId : undefined,
      onProgress: idleMs < maxMs ? arm : undefined,
    }
    state.pendingRequests.set(requestId, entry)
    arm()
    signal?.addEventListener('abort', onAbort, { once: true })
  })
}

/** The server is waiting on the person (elicitation asked or answered): its
 *  running tool calls are alive, so their idle clocks restart. */
export function noteMcpActivity(state: McpServerState): void {
  for (const pending of [...state.pendingRequests.values()]) pending.onProgress?.()
}

/** Routes `notifications/progress` to the request that asked for it. */
export function noteMcpProgress(state: McpServerState, progressToken: unknown): void {
  if (typeof progressToken !== 'number') return
  const pending = state.pendingRequests.get(progressToken)
  if (pending?.progressToken === progressToken) pending.onProgress?.()
}

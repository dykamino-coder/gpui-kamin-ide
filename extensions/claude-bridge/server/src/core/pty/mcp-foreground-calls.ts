// Tool calls the CLI is still waiting on, by session and JSON-RPC request id.
// A user interrupt ends the turn inside the CLI but says nothing to the client
// host, so the command it started kept running to its natural end
// (INC-2026-0096). This registry is what turns "the turn was stopped" into a
// cancellation of exactly those calls.
import type { PtySession } from './types'
import { interruptPendingForSession } from './session-mcp-call'

type RpcId = string | number
interface ForegroundCall {
  sessionId: string
  rpcId: RpcId | undefined
  cancel: () => void
}

const calls = new Set<ForegroundCall>()

/** Registers a call for the time the CLI awaits its answer; returns the release. */
export function trackForegroundCall(sessionId: string, rpcId: RpcId | undefined, cancel: () => void): () => void {
  const call: ForegroundCall = { sessionId, rpcId, cancel }
  calls.add(call)
  return () => {
    calls.delete(call)
  }
}

/** Cancels the awaited calls of a session, or only the one the CLI named in
 *  `notifications/cancelled`. Returns how many were cancelled. */
export function cancelForegroundCalls(sessionId: string, rpcId?: RpcId): number {
  let cancelled = 0
  for (const call of [...calls]) {
    if (call.sessionId !== sessionId || (rpcId !== undefined && call.rpcId !== rpcId)) continue
    calls.delete(call)
    cancelled++
    try {
      call.cancel()
    } catch {
      /* one failed cancel must not keep the others running */
    }
  }
  return cancelled
}

/** User pressed Stop: nothing consumes the results of this session's running
 *  tool calls any more, so stop them on the client host. Explicit background
 *  tasks are not awaited by a request and keep running, as in the native CLI. */
export function interruptSessionToolCalls(session: PtySession): number {
  return cancelForegroundCalls(session.id) + interruptPendingForSession(session)
}

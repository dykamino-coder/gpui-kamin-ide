// MCP-call routing extracted from session-core.ts (Sprint 5 / Stage E1).
// Server queues an MCP call, ships it over the WS to the client host, awaits the
// response (or denial). Pending calls + their timers live in this module.

import { randomUUID } from 'crypto'
import type { PtySession, PendingMcpCall } from './types'
import { eventBus } from '../events/bus'
import { debugLog, warnLog } from '../logging'
import { sendToClient } from './session-io'

/** Default MCP call timeout — most tools finish well under this. Heavy
 *  fs/shell tools (Grep/Glob/Bash/Read/etc.) hit HEAVY_TOOL_TIMEOUT_MS
 *  instead because on big repos (Rider solutions, node_modules) even a
 *  plain `rg` can genuinely take a minute+, and Bash legitimately runs
 *  `npm install` / `podman build` for many minutes. */
const MCP_CALL_TIMEOUT_MS = 120_000
const HEAVY_TOOL_TIMEOUT_MS = 1_800_000 // 30 minutes
const HEAVY_TOOLS = new Set([
  'Grep', 'Glob', 'Read', 'Bash', 'PowerShell', 'Write', 'Edit',
  'NotebookEdit', 'WebFetch', 'WebSearch', 'Monitor',
  // Worktree / LSP — cold-start + indexing on big repos legitimately
  // exceed the 2-min default.
  'EnterWorktree', 'ExitWorktree',
  'LspDiagnostics', 'LspHover', 'LspDefinition', 'LspReferences',
])
const INTERACTIVE_TOOLS = new Set(['AskUserQuestion', 'ExitPlanMode', 'EnterPlanMode'])

/** Pending MCP calls waiting for a client response. Key = requestId,
 *  shared across all sessions. */
export const pendingMcpCalls = new Map<string, PendingMcpCall>()

/** True iff the given session has any tool call awaiting a client
 *  response — used by the reaper to avoid killing sessions mid-Bash. */
export function hasInflightMcpCall(sessionId: string): boolean {
  for (const [, pending] of pendingMcpCalls) {
    if (pending.sessionId === sessionId) return true
  }
  return false
}

export function sendMcpCall(
  session: PtySession,
  toolName: string,
  input: Record<string, unknown>,
): Promise<unknown> {
  const requestId = randomUUID()
  session.mcpCallCount++
  session.lastActivityAt = new Date()

  eventBus.emit('session:updated', {
    sessionId: session.id,
    mcpCallCount: session.mcpCallCount,
    inputCount: session.inputCount,
    mcpInitialized: session.mcpInitialized,
    mcpLastError: session.mcpLastError,
    lastActivityAt: session.lastActivityAt.toISOString(),
  })

  return new Promise((resolve, reject) => {
    // Interactive tools (AskUserQuestion, ExitPlanMode) wait indefinitely —
    // user may think forever. Heavy fs/shell tools get 10 min because on
    // big repos they legitimately can run that long. Everything else: 2 min.
    const isInteractive = INTERACTIVE_TOOLS.has(toolName)
    const timeoutMs = HEAVY_TOOLS.has(toolName) ? HEAVY_TOOL_TIMEOUT_MS : MCP_CALL_TIMEOUT_MS
    const timer = isInteractive
      ? null
      : setTimeout(() => {
          pendingMcpCalls.delete(requestId)
          reject(new Error(`MCP call timeout (${timeoutMs}ms): ${toolName}`))
        }, timeoutMs)

    const delivered = session.ws.readyState === 1 /* WS.OPEN */
    pendingMcpCalls.set(requestId, {
      resolve, reject, timer, toolName, input, delivered,
      createdAt: Date.now(), sessionId: session.id,
    })

    sendToClient(session.ws, {
      type: 'mcp:call',
      requestId,
      toolName,
      input,
    })

    debugLog('MCP call sent to client host', { sessionId: session.id, requestId, toolName, delivered })
  })
}

/**
 * Re-send calls that never reached a client (issued while the session was
 * detached) to the freshly reattached WS. Delivered calls are left alone —
 * the surviving client may still be executing them and will answer on the
 * new socket.
 */
export function resendUndeliveredMcpCalls(session: PtySession): void {
  for (const [requestId, pending] of pendingMcpCalls) {
    if (pending.sessionId !== session.id || pending.delivered) continue
    if (session.ws.readyState !== 1 /* WS.OPEN */) return
    pending.delivered = true
    sendToClient(session.ws, {
      type: 'mcp:call',
      requestId,
      toolName: pending.toolName,
      input: pending.input,
    })
    debugLog('MCP call re-sent after reattach', { sessionId: session.id, requestId, toolName: pending.toolName })
  }
}

export function handleMcpResponse(requestId: string, result: unknown): void {
  const pending = pendingMcpCalls.get(requestId)
  if (!pending) {
    warnLog('MCP response for unknown requestId', { requestId })
    return
  }
  if (pending.timer) clearTimeout(pending.timer)
  pendingMcpCalls.delete(requestId)
  const durationMs = Date.now() - pending.createdAt
  debugLog('MCP response received', { requestId, toolName: pending.toolName, durationMs })
  pending.resolve(result)
}

export function handleMcpDenied(requestId: string, reason: string): void {
  const pending = pendingMcpCalls.get(requestId)
  if (!pending) return
  if (pending.timer) clearTimeout(pending.timer)
  pendingMcpCalls.delete(requestId)
  debugLog('MCP call denied', { requestId, toolName: pending.toolName, reason })
  pending.reject(new Error(`Tool denied by user: ${reason}`))
}

/** Reject all pending MCP calls — used on graceful shutdown. */
export function rejectAllPending(reason: string): void {
  for (const [, pending] of pendingMcpCalls) {
    if (pending.timer) clearTimeout(pending.timer)
    pending.reject(new Error(reason))
  }
  pendingMcpCalls.clear()
}

/** Reject + clear the pending MCP calls belonging to ONE session. Called when a
 *  session is destroyed (tab close / reap) so its in-flight calls don't leak —
 *  especially INTERACTIVE tools (AskUserQuestion/ExitPlanMode), whose promise has
 *  no timeout and would otherwise keep its SSE stream + heartbeat interval alive
 *  forever. */
export function rejectPendingForSession(sessionId: string, reason: string): void {
  for (const [id, pending] of pendingMcpCalls) {
    if (pending.sessionId !== sessionId) continue
    if (pending.timer) clearTimeout(pending.timer)
    pendingMcpCalls.delete(id)
    pending.reject(new Error(reason))
  }
}

// MCP-call + hook-execute + permission-check helpers extracted from
// connection-manager.ts (Sprint 5 / Stage E1). Pure handlers — caller is
// responsible for owning the pendingMcpCalls map, the WS send channel,
// and the renderer IPC channel.

import type { BrowserWindow } from '@kaminide/host-compat'
import type { PermissionDecision } from '../../shared/types'
import { mcpDiagnostic } from './mcp-diagnostics'
import type { McpOutcome } from './mcp-result-outbox'
import type { ClientMessage } from '../../shared/mcp-protocol'
import type { PermissionManager } from '../mcp/permission-manager'
import type { PermissionMode } from './connection-manager'
import { resolveWorkingDirectory } from '../utils/working-directory'

/** Working directory of the tab for client-side work: its folder when that
 *  exists on this host, otherwise the home directory. */
export function tabWorkingDirectory(configured: string | undefined): string {
  return resolveWorkingDirectory(configured)
}

export interface PendingMcpExecution {
  toolName: string
  input: Record<string, unknown>
  taskOwner?: string
  controller?: AbortController
  output?: string
}

export type PendingPermissionDecision = PermissionDecision | 'cancelled' | 'disconnected'
type PermissionOutcome = { allowed: true } | { allowed: false; reason: string }

export function cancelMcpCall(ctx: McpCallCtx, requestId: string, taskOwner?: string): void {
  const call = ctx.pendingMcpCalls.get(requestId)
  if (!call || call.taskOwner !== taskOwner) return
  call.controller?.abort()
  ctx.pendingPermissions.get(requestId)?.('cancelled')
}

export interface McpCallCtx {
  window: BrowserWindow
  tabId: string
  send: (msg: ClientMessage) => void
  reportDeliveryFailure: () => void
  deliverResult: (outcome: McpOutcome) => Promise<McpOutcome>
  pendingMcpCalls: Map<string, PendingMcpExecution>
  pendingPermissions: Map<string, (decision: PendingPermissionDecision) => void>
  workingDirectory?: () => string | undefined
  commandOwner?: () => string | undefined
  permissionMode: () => PermissionMode
  permissionManager: PermissionManager
  denyMcp: (requestId: string, reason: string) => void
}

/** Preserve who denied a call; lifecycle cancellation is not a user response. */
export function checkToolPermission(
  ctx: McpCallCtx,
  requestId: string,
  toolName: string,
  input: Record<string, unknown>,
): PermissionOutcome | Promise<PermissionOutcome> {
  // Internal resource/prompt relays are continuations of an MCP protocol
  // request, not model-invokable shell tools. They are never advertised in
  // tools/list and must not surface a misleading permission dialog.
  if (toolName.startsWith('__bridge_mcp_')) return { allowed: true }
  const mode = ctx.permissionMode()
  if (mode === 'bypassPermissions') return { allowed: true }

  const category = ctx.permissionManager.getToolCategory(toolName)

  if (mode === 'acceptEdits') {
    if (category !== 'shell' && category !== 'git') return { allowed: true }
    // Shell/git → ask user
  } else {
    const decision = ctx.permissionManager.checkPermission(toolName, input)
    if (decision === 'allow') return { allowed: true }
    if (decision === 'deny') return { allowed: false, reason: 'Permission denied by policy' }
    // decision === 'ask' → ask user
  }

  return new Promise<PermissionOutcome>((resolve) => {
    const settle = (outcome: PermissionOutcome): boolean => {
      // Replies saved by an old renderer or teardown cannot grant a session rule.
      if (ctx.pendingPermissions.get(requestId) !== respond) return false
      ctx.pendingPermissions.delete(requestId)
      resolve(outcome)
      try {
        ctx.window.webContents.send('permission-resolved', ctx.tabId, { requestId })
      } catch {
        // A detached view receives the reconciled workspace snapshot on attach.
      }
      return true
    }
    const respond = (decision: PendingPermissionDecision): void => {
      const reason = decision === 'deny' ? 'User denied permission'
        : decision === 'cancelled' ? 'Permission request cancelled'
        : decision === 'disconnected' ? 'Permission request cancelled: connection disconnected'
        : undefined
      if (!settle(reason ? { allowed: false, reason } : { allowed: true })) return
      if (decision === 'allow_session' || decision === 'always') {
        ctx.permissionManager.setSessionPermission(toolName, 'allow')
      }
    }
    // Register first: publication can synchronously answer or fail.
    ctx.pendingPermissions.set(requestId, respond)
    try {
      ctx.window.webContents.send('permission-request', ctx.tabId, { requestId, toolName, input, category })
    } catch (error) {
      settle({ allowed: false, reason: `Permission request delivery failed: ${error instanceof Error ? error.message : String(error)}` })
    }
  })
}

/** Run the user's hook command on the host with full PATH/env, send result
 *  back over the WS. Errors caught into a hook:response with stderr +
 *  exitCode 1 + outcome:error. */
export async function handleHookExecute(ctx: McpCallCtx, msg: any): Promise<void> {
  try {
    const { handleMonitorTriggerCommand } = await import('../plugin-monitors')
    if (await handleMonitorTriggerCommand(ctx.tabId, String(msg.command ?? ''), msg.payload ?? {}, msg.pluginId)) {
      ctx.send({
        type: 'hook:response',
        requestId: msg.requestId,
        result: {
          stdout: '',
          stderr: '',
          exitCode: 0,
          outcome: 'success',
          durationMs: 0,
        },
      })
      return
    }
    const { executeHook } = await import('../hooks/executor')
    const result = await executeHook(msg, ctx.workingDirectory?.())
    ctx.send({ type: 'hook:response', requestId: msg.requestId, result })
  } catch (err) {
    ctx.send({
      type: 'hook:response',
      requestId: msg.requestId,
      result: {
        stdout: '',
        stderr: err instanceof Error ? err.message : String(err),
        exitCode: 1,
        outcome: 'error',
        durationMs: 0,
      },
    })
  }
}

/** This host's share of one call, in milliseconds (0.1 ms), for the server's
 *  per-call trace. Durations only: the two clocks never need to agree. */
export function callTiming(
  receivedAt: number,
  toolStartedAt: number,
  toolEndedAt: number,
  sentAt: number,
): { queuedMs: number; toolMs: number; totalMs: number } {
  const ms = (n: number): number => Math.max(0, Math.round(n * 10) / 10)
  return { queuedMs: ms(toolStartedAt - receivedAt), toolMs: ms(toolEndedAt - toolStartedAt), totalMs: ms(sentAt - receivedAt) }
}

/** Handle incoming MCP tool call from server. Stores as pending, checks
 *  permissions, then executes via MCP executor. mcp-activity IPC fires for
 *  pending → completed/denied so the renderer can paint the call row. */
export async function handleMcpCall(
  ctx: McpCallCtx,
  msg: {
    requestId: string
    toolName: string
    input: Record<string, unknown>
    taskOwner?: string
    taskDeadlineMs?: number
  },
): Promise<void> {
  if (ctx.pendingMcpCalls.has(msg.requestId)) return
  const receivedAt = performance.now()
  const correlation = { requestId: msg.requestId, toolName: msg.toolName }
  mcpDiagnostic('request-received', correlation)
  const controller = new AbortController()
  const call: PendingMcpExecution = {
    toolName: msg.toolName,
    input: msg.input,
    taskOwner: msg.taskOwner,
    controller,
  }
  ctx.pendingMcpCalls.set(msg.requestId, call)

  ctx.window.webContents.send('mcp-activity', ctx.tabId, {
    requestId: msg.requestId,
    toolName: msg.toolName,
    input: msg.input,
    timestamp: Date.now(),
    status: 'pending',
  })

  const permission = await checkToolPermission(ctx, msg.requestId, msg.toolName, msg.input)
  if (ctx.pendingMcpCalls.get(msg.requestId) !== call) return
  mcpDiagnostic(permission.allowed ? 'permission-allowed' : 'permission-denied', correlation)
  if (!permission.allowed || controller.signal.aborted) {
    ctx.denyMcp(msg.requestId, permission.allowed ? 'Permission request cancelled' : permission.reason)
    return
  }

  const start = Date.now()
  const toolStartedAt = performance.now()
  mcpDiagnostic('execution-started', correlation)
  let result: unknown
  try {
    const { executeTool } = await import('../mcp/executor')
    result = await executeTool(msg.toolName, msg.input, {
      tabId: ctx.tabId,
      cwd: ctx.workingDirectory?.(),
      signal: controller.signal,
      taskDeadlineMs: msg.taskOwner ? msg.taskDeadlineMs : undefined,
      shellOwner: ctx.commandOwner?.(),
      onOutput: (chunk) => {
        if (ctx.pendingMcpCalls.get(msg.requestId) === call) call.output = ((call.output ?? '') + chunk).slice(-16384)
      },
    })
  } catch (err) {
    result = `Error: ${err instanceof Error ? err.message : String(err)}`
  }
  const toolEndedAt = performance.now()
  mcpDiagnostic('execution-finished', {
    ...correlation,
    durationMs: Date.now() - start,
  })
  if (ctx.pendingMcpCalls.get(msg.requestId) !== call) return
  try {
    const delivery = ctx.deliverResult({
      type: 'mcp:response',
      requestId: msg.requestId,
      result,
      taskOwner: msg.taskOwner,
      timing: callTiming(receivedAt, toolStartedAt, toolEndedAt, performance.now()),
    })
    // The outbox owns its bounded serialized frame while awaiting an ack.
    result = undefined
    const delivered = await delivery
    result = delivered.type === 'mcp:response' ? delivered.result : delivered.reason
    if (ctx.pendingMcpCalls.get(msg.requestId) !== call) return
    ctx.window.webContents.send('mcp-activity', ctx.tabId, {
      requestId: msg.requestId,
      toolName: msg.toolName,
      input: msg.input,
      timestamp: Date.now(),
      status: 'completed',
      result,
      durationMs: Date.now() - start,
    })
  } catch (error) {
    if (ctx.pendingMcpCalls.get(msg.requestId) !== call) return
    ctx.reportDeliveryFailure()
    ctx.window.webContents.send('mcp-activity', ctx.tabId, {
      requestId: msg.requestId,
      toolName: msg.toolName,
      input: msg.input,
      timestamp: Date.now(),
      status: 'denied',
      result: `Delivery failed: ${String(error)}`,
    })
  } finally {
    if (ctx.pendingMcpCalls.get(msg.requestId) === call) ctx.pendingMcpCalls.delete(msg.requestId)
  }
}

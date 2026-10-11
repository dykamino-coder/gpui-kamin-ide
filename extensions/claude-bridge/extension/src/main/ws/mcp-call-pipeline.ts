// MCP-call + hook-execute + permission-check helpers extracted from
// connection-manager.ts (Sprint 5 / Stage E1). Pure handlers — caller is
// responsible for owning the pendingMcpCalls map, the WS send channel,
// and the renderer IPC channel.

import type { BrowserWindow } from '@kaminide/host-compat'
import type { PermissionDecision } from '../../shared/types'
import type { ClientMessage } from '../../shared/mcp-protocol'
import type { PermissionManager } from '../mcp/permission-manager'
import type { PermissionMode } from './connection-manager'

export interface McpCallCtx {
  window: BrowserWindow
  tabId: string
  send: (msg: ClientMessage) => void
  reportDeliveryFailure: () => void
  deliverResult: (outcome: Extract<ClientMessage, { type: 'mcp:response' | 'mcp:denied' }>) => Promise<void>
  pendingMcpCalls: Map<string, { toolName: string; input: Record<string, unknown> }>
  pendingPermissions: Map<string, (decision: PermissionDecision) => void>
  permissionMode: () => PermissionMode
  permissionManager: PermissionManager
  denyMcp: (requestId: string, reason: string) => void
}

/** Permission gate. Returns true if allowed, false if denied, or a Promise
 *  that resolves to true/false after the user responds (for 'ask' scenarios). */
export function checkToolPermission(
  ctx: McpCallCtx,
  requestId: string,
  toolName: string,
  input: Record<string, unknown>,
): boolean | Promise<boolean> {
  // Internal resource/prompt relays are continuations of an MCP protocol
  // request, not model-invokable shell tools. They are never advertised in
  // tools/list and must not surface a misleading permission dialog.
  if (toolName.startsWith('__bridge_mcp_')) return true
  const mode = ctx.permissionMode()
  if (mode === 'bypassPermissions') return true

  const category = ctx.permissionManager.getToolCategory(toolName)

  if (mode === 'acceptEdits') {
    if (category !== 'shell' && category !== 'git') return true
    // Shell/git → ask user
  } else {
    const decision = ctx.permissionManager.checkPermission(toolName, input)
    if (decision === 'allow') return true
    if (decision === 'deny') return false
    // decision === 'ask' → ask user
  }

  ctx.window.webContents.send('permission-request', ctx.tabId, {
    requestId,
    toolName,
    input,
    category,
  })

  return new Promise<boolean>((resolve) => {
    ctx.pendingPermissions.set(requestId, (decision: PermissionDecision) => {
      const allowed = decision !== 'deny'
      if (decision === 'allow_session' || decision === 'always') {
        ctx.permissionManager.setSessionPermission(toolName, 'allow')
      }
      resolve(allowed)
    })
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
        type: 'hook:response', requestId: msg.requestId,
        result: { stdout: '', stderr: '', exitCode: 0, outcome: 'success', durationMs: 0 },
      })
      return
    }
    const { executeHook } = await import('../hooks/executor')
    const result = await executeHook(msg)
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

/** Handle incoming MCP tool call from server. Stores as pending, checks
 *  permissions, then executes via MCP executor. mcp-activity IPC fires for
 *  pending → completed/denied so the renderer can paint the call row. */
export async function handleMcpCall(
  ctx: McpCallCtx,
  msg: { requestId: string; toolName: string; input: Record<string, unknown> },
): Promise<void> {
  if (ctx.pendingMcpCalls.has(msg.requestId)) return
  ctx.pendingMcpCalls.set(msg.requestId, { toolName: msg.toolName, input: msg.input })

  ctx.window.webContents.send('mcp-activity', ctx.tabId, {
    requestId: msg.requestId,
    toolName: msg.toolName,
    input: msg.input,
    timestamp: Date.now(),
    status: 'pending',
  })

  const allowed = await checkToolPermission(ctx, msg.requestId, msg.toolName, msg.input)
  if (!allowed) {
    ctx.denyMcp(msg.requestId, 'User denied permission')
    return
  }

  const start = Date.now()
  let result: unknown
  try {
    const { executeTool } = await import('../mcp/executor')
    result = await executeTool(msg.toolName, msg.input, { tabId: ctx.tabId })
  } catch (err) {
    result = `Error: ${err instanceof Error ? err.message : String(err)}`
  }
  if (!ctx.pendingMcpCalls.has(msg.requestId)) return
  try {
    await ctx.deliverResult({ type: 'mcp:response', requestId: msg.requestId, result })
    if (!ctx.pendingMcpCalls.has(msg.requestId)) return
    ctx.window.webContents.send('mcp-activity', ctx.tabId, {
      requestId: msg.requestId, toolName: msg.toolName, input: msg.input,
      timestamp: Date.now(), status: 'completed', result, durationMs: Date.now() - start,
    })
  } catch (error) {
    if (!ctx.pendingMcpCalls.has(msg.requestId)) return
    ctx.reportDeliveryFailure()
    ctx.window.webContents.send('mcp-activity', ctx.tabId, {
      requestId: msg.requestId, toolName: msg.toolName, input: msg.input,
      timestamp: Date.now(), status: 'denied', result: `Delivery failed: ${String(error)}`,
    })
  } finally {
    ctx.pendingMcpCalls.delete(msg.requestId)
  }
}

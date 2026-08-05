// MCP-call + hook-execute + permission-check helpers extracted from
// connection-manager.ts (Sprint 5 / Stage E1). Pure handlers — caller is
// responsible for owning the pendingMcpCalls map, the WS send channel,
// and the renderer IPC channel.

import type { BrowserWindow } from 'electron'
import type { PermissionDecision } from '../../shared/types'
import type { ClientMessage } from '../../shared/mcp-protocol'
import type { PermissionManager } from '../mcp/permission-manager'
import type { PermissionMode } from './connection-manager'

export interface McpCallCtx {
  window: BrowserWindow
  tabId: string
  send: (msg: ClientMessage) => void
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

  try {
    const { executeTool } = await import('../mcp/executor')
    // Pass the source tabId so widget-based tools (AskUserQuestion, etc.)
    // show in the correct chat even when the user is looking elsewhere.
    const start = Date.now()
    const result = await executeTool(msg.toolName, msg.input, { tabId: ctx.tabId })
    const durationMs = Date.now() - start

    ctx.send({ type: 'mcp:response', requestId: msg.requestId, result })
    ctx.pendingMcpCalls.delete(msg.requestId)

    ctx.window.webContents.send('mcp-activity', ctx.tabId, {
      requestId: msg.requestId,
      toolName: msg.toolName,
      input: msg.input,
      timestamp: Date.now(),
      status: 'completed',
      result,
      durationMs,
    })
  } catch (err) {
    const errMsg = err instanceof Error ? err.message : String(err)
    ctx.send({ type: 'mcp:response', requestId: msg.requestId, result: `Error: ${errMsg}` })
    ctx.pendingMcpCalls.delete(msg.requestId)
  }
}

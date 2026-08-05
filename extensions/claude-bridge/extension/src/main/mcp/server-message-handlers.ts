// Inbound MCP message routing — handle server→client requests / notifications
// extracted from `manager.ts` (Sprint 2 / Stage C, C3 final). The manager
// passes a `MessageHandlerCtx` (slim view of its mutable state) so these
// remain pure functions over per-server state.

import { randomUUID } from 'crypto'
import type { BrowserWindow, IpcMainEvent } from 'electron'
import type { McpResourceInfo, McpResourceTemplateInfo, McpPromptInfo } from '../../shared/types'
import type { McpServerState } from './transports/context'
import { collectMcpList } from './pagination'

export interface MessageHandlerCtx {
  servers: Map<string, McpServerState>
  window: BrowserWindow
  notifyChanged: () => void
  onToolsChanged: (() => void) | null
  /** Transport-agnostic outbound RPC — used to refetch tool/resource/prompt
   *  catalogs after `*_list_changed` notifications. */
  rpcRequest: (id: string, method: string, params: Record<string, unknown>) => Promise<unknown>
}

/** Top-level dispatcher — routes incoming server message to one of three
 *  paths: server-initiated request (id+method), notification (no id), or
 *  response to one of our pending requests (id only). */
export function handleServerMessage(
  ctx: MessageHandlerCtx,
  serverId: string,
  msg: any,
  sendResponse: (response: Record<string, unknown>) => void,
): void {
  const state = ctx.servers.get(serverId)
  if (!state) return

  // Server → client request (needs a response)
  if (msg.id !== undefined && typeof msg.method === 'string') {
    handleServerRequest(ctx, serverId, msg).then(result => {
      sendResponse({ jsonrpc: '2.0', id: msg.id, result })
    }).catch(err => {
      sendResponse({ jsonrpc: '2.0', id: msg.id, error: { code: -32603, message: err instanceof Error ? err.message : String(err) } })
    })
    return
  }

  // Notification
  if (msg.id === undefined && typeof msg.method === 'string') {
    handleServerNotification(ctx, serverId, msg)
    return
  }

  // Response to one of our requests
  if (msg.id !== undefined) {
    const pending = state.pendingRequests.get(msg.id)
    if (pending) {
      state.pendingRequests.delete(msg.id)
      if (pending.timer) clearTimeout(pending.timer)
      // stdio pending entries expect full msg (they read .result / .error on the
      // returned object); ws entries expect the same shape. Pass it through —
      // callers handle both shapes.
      pending.resolve(msg)
    }
  }
}

/**
 * Handle a request initiated by the MCP server. Currently:
 *  - `elicitation/create` → forwarded to the renderer (user prompt dialog)
 *  - `sampling/createMessage` → not supported (direct LLM access unavailable)
 *  - `ping` → echo
 *  - everything else → reject as unimplemented
 */
async function handleServerRequest(ctx: MessageHandlerCtx, serverId: string, msg: any): Promise<any> {
  const method = msg.method as string
  const state = ctx.servers.get(serverId)!

  if (method === 'elicitation/create') {
    // Ask the renderer for user input. The renderer responds via IPC.
    const requestId = randomUUID()
    const params = msg.params ?? {}
    const { ipcMain } = await import('electron')
    return await new Promise<any>((resolve) => {
      const responseChannel = `mcp-elicitation-response:${requestId}`
      let settled = false
      let timer: ReturnType<typeof setTimeout> | null = null
      const settle = (value: any): void => {
        if (settled) return
        settled = true
        try { ipcMain.removeListener(responseChannel, listener) } catch { /* noop */ }
        if (timer) { clearTimeout(timer); timer = null }
        resolve(value)
      }
      const listener = (_event: IpcMainEvent, payload: any) => settle(payload)
      ipcMain.on(responseChannel, listener)
      try {
        ctx.window.webContents.send('mcp-elicitation-request', {
          requestId,
          serverId,
          serverName: state.config.name,
          message: params.message,
          requestedSchema: params.requestedSchema,
        })
      } catch {
        settle({ action: 'dismiss' })
        return
      }
      // Auto-dismiss after 10 minutes. Guarded by `settled` so a prompt user
      // answer cancels the timeout and prevents a second resolve / leak.
      timer = setTimeout(() => settle({ action: 'dismiss' }), 10 * 60 * 1000)
    })
  }

  if (method === 'sampling/createMessage') {
    // MCP server is asking us to run an LLM sampling turn. We don't have
    // direct LLM access (would need to route through an active PTY session
    // with careful context isolation), so report unsupported. Servers that
    // depend on this will fall back to their own strategies.
    throw new Error('sampling/createMessage is not implemented by open-claude-bridge')
  }

  if (method === 'ping') {
    return {}
  }

  throw new Error(`Unsupported server request: ${method}`)
}

/** Handle notifications (no id). Forwards progress / logs to the renderer
 *  and re-fetches tool/resource/prompt catalogs on `*_list_changed`. */
function handleServerNotification(ctx: MessageHandlerCtx, serverId: string, msg: any): void {
  const method = msg.method as string
  const state = ctx.servers.get(serverId)!

  if (method === 'notifications/progress') {
    // Forward to renderer; UI may show a progress bar for long-running tools.
    try {
      ctx.window.webContents.send('mcp-progress', {
        serverId,
        serverName: state.config.name,
        progressToken: msg.params?.progressToken,
        progress: msg.params?.progress,
        total: msg.params?.total,
        message: msg.params?.message,
      })
    } catch { /* window may be closing */ }
    return
  }

  if (method === 'notifications/tools/list_changed') {
    // Re-fetch the tool catalog
    void collectMcpList<any>(
      (listMethod, params) => ctx.rpcRequest(serverId, listMethod, params),
      'tools/list',
      'tools',
    ).then(tools => {
      state.tools = tools.map((tool: { name: string }) => tool.name)
      state.toolSchemas.clear()
      for (const tool of tools) state.toolSchemas.set(tool.name, tool)
      ctx.notifyChanged()
      ctx.onToolsChanged?.()
    }).catch(() => {})
    return
  }

  if (method === 'notifications/resources/list_changed') {
    const request = (listMethod: string, params: Record<string, unknown>) => ctx.rpcRequest(serverId, listMethod, params)
    void Promise.allSettled([
      collectMcpList<any>(request, 'resources/list', 'resources'),
      collectMcpList<any>(request, 'resources/templates/list', 'resourceTemplates'),
    ]).then(([resourcesResult, templatesResult]) => {
      let changed = false
      if (resourcesResult.status === 'fulfilled') {
        state.resources = resourcesResult.value.map((resource: any) => ({
          uri: String(resource.uri ?? ''), name: String(resource.name ?? resource.uri ?? ''), description: resource.description, mimeType: resource.mimeType,
        })).filter((resource: McpResourceInfo) => resource.uri)
        changed = true
      }
      if (templatesResult.status === 'fulfilled') {
        state.resourceTemplates = templatesResult.value.map((template: any) => ({
          uriTemplate: String(template.uriTemplate ?? ''), name: String(template.name ?? template.uriTemplate ?? ''), description: template.description, mimeType: template.mimeType,
        })).filter((template: McpResourceTemplateInfo) => template.uriTemplate)
        changed = true
      }
      if (changed) {
        ctx.notifyChanged()
        ctx.onToolsChanged?.()
      }
    })
    return
  }

  if (method === 'notifications/prompts/list_changed') {
    void collectMcpList<any>(
      (listMethod, params) => ctx.rpcRequest(serverId, listMethod, params),
      'prompts/list',
      'prompts',
    ).then(prompts => {
      state.prompts = prompts.map((prompt: any) => ({
        name: String(prompt.name ?? ''), description: prompt.description, arguments: Array.isArray(prompt.arguments) ? prompt.arguments : undefined,
      })).filter((prompt: McpPromptInfo) => prompt.name)
      ctx.notifyChanged()
      ctx.onToolsChanged?.()
    }).catch(() => {})
    return
  }

  if (method === 'notifications/message' || method === 'notifications/logMessage') {
    const level = msg.params?.level ?? 'info'
    const data = msg.params?.data ?? msg.params?.message ?? ''
    console.log(`[MCP ${state.config.name}] ${level}:`, typeof data === 'string' ? data : JSON.stringify(data))
    return
  }

  // Unknown notification → ignore
}

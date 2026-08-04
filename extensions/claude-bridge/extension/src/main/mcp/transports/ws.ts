// WebSocket transport for MCP servers.
//
// Server runs a ws:// endpoint; we JSON-RPC over it with full duplex. Used
// primarily for servers that need to push server-initiated requests (e.g.
// elicitation), which streamable HTTP doesn't support cleanly.

import WebSocket from 'ws'
import { resolveEnvVars } from '../discovery'
import { loadTokens } from '../oauth-store'
import type { McpResult } from '../tool-registry'
import type { TransportContext } from './context'

export async function connectWs(ctx: TransportContext, id: string): Promise<void> {
  const state = ctx.servers.get(id)!
  const url = state.config.url
  if (!url) throw new Error('WebSocket server URL is required')

  const headers: Record<string, string> = {}
  if (state.config.headers) {
    for (const [k, v] of Object.entries(state.config.headers)) headers[k] = resolveEnvVars(v)
  }
  if (state.config.oauth) {
    const tokens = loadTokens(id)
    if (tokens) headers['Authorization'] = `Bearer ${tokens.access_token}`
  }

  ctx.appendLog(id, 'info', `[ws] connecting to ${url}`)
  await new Promise<void>((resolve, reject) => {
    const ws = new WebSocket(url, { headers })
    state.ws = ws
    const fail = (err: Error) => {
      try { ws.close() } catch {}
      // Drop the reference so the state doesn't carry a dead socket with
      // dangling handlers forward to the next connect attempt.
      if (state.ws === ws) state.ws = undefined
      ctx.appendLog(id, 'error', `[ws] ${err.message}`)
      reject(err)
    }
    const timeout = setTimeout(() => fail(new Error('WebSocket connect timeout (30s)')), 30000)

    ws.on('open', () => { clearTimeout(timeout); ctx.appendLog(id, 'info', '[ws] open'); resolve() })
    ws.on('error', (err) => { clearTimeout(timeout); fail(err as Error) })
    ws.on('close', () => {
      if (state.ws === ws) state.ws = undefined
      state.status = 'disconnected'
      state.tools = []
      ctx.notifyChanged()
      ctx.onToolsChanged?.()
    })
    ws.on('message', (data: WebSocket.RawData) => {
      try {
        const msg = JSON.parse(data.toString())
        ctx.handleServerMessage(id, msg, (response) => {
          try { ws.send(JSON.stringify(response)) } catch { /* closed */ }
        })
      } catch {
        // ignore non-JSON frames
      }
    })
  })

  // initialize
  ctx.appendLog(id, 'info', '[ws] → initialize')
  const initResult: any = await wsJsonRpc(ctx, id, {
    jsonrpc: '2.0', id: state.nextRequestId++, method: 'initialize',
    params: {
      protocolVersion: '2024-11-05',
      capabilities: { sampling: {}, elicitation: {} },
      clientInfo: { name: 'open-claude-bridge', version: '4.0.0' },
    },
  })
  state.capabilities = initResult?.capabilities ?? {}
  ctx.appendLog(id, 'info', initResult?.serverInfo?.name
    ? `[ws] ← initialize ok: server "${initResult.serverInfo.name}" v${initResult.serverInfo.version ?? '?'}`
    : '[ws] ← initialize ok')

  // initialized notification
  state.ws!.send(JSON.stringify({ jsonrpc: '2.0', method: 'notifications/initialized' }))

  // tools/list
  ctx.appendLog(id, 'info', '[ws] → tools/list')
  const toolsResult: any = await wsJsonRpc(ctx, id, {
    jsonrpc: '2.0', id: state.nextRequestId++, method: 'tools/list',
  })
  if (toolsResult?.tools) {
    state.tools = toolsResult.tools.map((t: { name: string }) => t.name)
    for (const tool of toolsResult.tools) state.toolSchemas.set(tool.name, tool)
    ctx.appendLog(id, 'info', `[ws] ← tools/list: ${state.tools.length} tools`)
  } else {
    ctx.appendLog(id, 'warn', '[ws] ← tools/list: empty response')
  }
}

export async function callWsTool(
  ctx: TransportContext,
  id: string,
  toolName: string,
  input: Record<string, unknown>,
): Promise<McpResult> {
  const state = ctx.servers.get(id)!
  try {
    const result: any = await wsJsonRpc(ctx, id, {
      jsonrpc: '2.0', id: state.nextRequestId++, method: 'tools/call',
      params: { name: toolName, arguments: input },
    })
    if (result?.content && Array.isArray(result.content)) return result
    return { content: [{ type: 'text', text: typeof result === 'string' ? result : JSON.stringify(result) }] }
  } catch (err) {
    return { content: [{ type: 'text', text: `MCP Error: ${err instanceof Error ? err.message : String(err)}` }] }
  }
}

/** Send a JSON-RPC request over WebSocket and await the matching response. */
export function wsJsonRpc(ctx: TransportContext, id: string, body: Record<string, unknown>): Promise<unknown> {
  const state = ctx.servers.get(id)!
  if (!state.ws || state.ws.readyState !== WebSocket.OPEN) {
    return Promise.reject(new Error('WebSocket not open'))
  }
  const reqId = body.id as number
  // Notifications (no id) → fire-and-forget, no pending entry.
  if (reqId === undefined) {
    state.ws.send(JSON.stringify(body))
    return Promise.resolve(undefined)
  }
  const promise = new Promise((resolve, reject) => {
    const entry = {
      resolve: (msg: any) => {
        if (msg?.error) reject(new Error(msg.error.message))
        else resolve(msg?.result)
      },
      reject,
      timer: undefined as ReturnType<typeof setTimeout> | undefined,
    }
    state.pendingRequests.set(reqId, entry)
    entry.timer = setTimeout(() => {
      if (state.pendingRequests.has(reqId)) {
        state.pendingRequests.delete(reqId)
        reject(new Error('WS request timeout (120s)'))
      }
    }, 120_000)
  })
  state.ws.send(JSON.stringify(body))
  return promise
}

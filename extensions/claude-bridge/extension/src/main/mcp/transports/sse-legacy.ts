// Legacy SSE transport (MCP 2024-11-05 HTTP+SSE).
//
// Old transport: client opens a long-lived GET /sse, server sends an
// `endpoint` event whose data is the URL to POST JSON-RPC requests to.
// Responses come back as `message` events on the SSE stream, matched
// by JSON-RPC id. Rider's MCP plugin still uses this transport; the
// unified streamable-http path (POST /sse) returns 405 because /sse
// only accepts GET.

import type { TransportContext } from './context'

export async function connectSseLegacy(ctx: TransportContext, id: string): Promise<void> {
  const state = ctx.servers.get(id)!
  const url = state.config.url
  if (!url) throw new Error('SSE server URL is required')

  const abort = new AbortController()
  state.sseAbort = abort

  const headers = await ctx.buildHttpHeaders(id)
  headers['Accept'] = 'text/event-stream'

  ctx.appendLog(id, 'info', `[sse] GET ${url}`)
  const res = await fetch(url, { method: 'GET', headers, signal: abort.signal })
  if (!res.ok || !res.body) {
    ctx.appendLog(id, 'error', `[sse] HTTP ${res.status} opening stream`)
    throw new Error(`HTTP ${res.status} opening SSE stream`)
  }
  ctx.appendLog(id, 'info', `[sse] stream open (${res.status})`)

  const endpointPromise = new Promise<string>((resolve, reject) => {
    const timer = setTimeout(() => reject(new Error('Timed out waiting for SSE endpoint event (10s)')), 10_000)
    ;(state as any).__sseEndpointResolver = (ep: string) => { clearTimeout(timer); resolve(ep) }
    ;(state as any).__sseEndpointReject = (err: Error) => { clearTimeout(timer); reject(err) }
  })

  // Spawn a reader loop that parses SSE events. Events are separated by
  // blank lines; each event has `event:` and `data:` fields.
  void sseReaderLoop(ctx, id, res).catch(err => {
    const reject = (state as any).__sseEndpointReject
    if (reject) reject(err instanceof Error ? err : new Error(String(err)))
  })

  const endpoint = await endpointPromise
  // Endpoint may be relative (`/message?sessionId=...`). Resolve against the
  // GET URL's origin so we have an absolute POST target.
  state.sseMessageEndpoint = endpoint.startsWith('http') ? endpoint : new URL(endpoint, url).toString()
  ctx.appendLog(id, 'info', `[sse] message endpoint: ${state.sseMessageEndpoint}`)

  const initResult = await sseJsonRpcRequest(ctx, id, {
    jsonrpc: '2.0',
    id: state.nextRequestId++,
    method: 'initialize',
    params: {
      protocolVersion: '2024-11-05',
      capabilities: { sampling: {}, elicitation: {} },
      clientInfo: { name: 'open-claude-bridge', version: '4.0.0' },
    },
  })
  state.capabilities = (initResult as any)?.capabilities ?? {}

  await sseSendNotification(ctx, id, { jsonrpc: '2.0', method: 'notifications/initialized' })

  const toolsResult = await sseJsonRpcRequest(ctx, id, {
    jsonrpc: '2.0',
    id: state.nextRequestId++,
    method: 'tools/list',
  })
  const tools = (toolsResult as any)?.tools
  if (Array.isArray(tools)) {
    state.tools = tools.map((t: { name: string }) => t.name)
    for (const tool of tools) state.toolSchemas.set(tool.name, tool)
    ctx.appendLog(id, 'info', `[sse] ← tools/list: ${state.tools.length} tools`)
  } else {
    ctx.appendLog(id, 'warn', '[sse] ← tools/list: empty response')
  }
}

/** Read the SSE stream byte-by-byte, split into events, dispatch.
 *  Runs until the AbortController fires or the server closes the stream. */
async function sseReaderLoop(ctx: TransportContext, id: string, res: Response): Promise<void> {
  const state = ctx.servers.get(id)!
  const reader = res.body!.getReader()
  const decoder = new TextDecoder()
  let buffer = ''
  while (true) {
    let chunk: { done: boolean; value?: Uint8Array }
    try { chunk = await reader.read() } catch { break }
    if (chunk.done) break
    buffer += decoder.decode(chunk.value!, { stream: true })
    // Events terminated by blank line (CRLF tolerant).
    let sep: number
    while ((sep = buffer.indexOf('\n\n')) !== -1 || (sep = buffer.indexOf('\r\n\r\n')) !== -1) {
      const marker = buffer[sep + 1] === '\n' ? '\r\n\r\n' : '\n\n'
      const rawEvent = buffer.slice(0, sep)
      buffer = buffer.slice(sep + marker.length)
      let eventName = 'message'
      let data = ''
      for (const line of rawEvent.split(/\r?\n/)) {
        if (line.startsWith('event:')) eventName = line.slice(6).trim()
        else if (line.startsWith('data:')) data += (data ? '\n' : '') + line.slice(5).trim()
      }
      if (eventName === 'endpoint') {
        const resolver = (state as any).__sseEndpointResolver as ((ep: string) => void) | undefined
        if (resolver) resolver(data)
      } else if (eventName === 'message' && data) {
        try {
          const msg = JSON.parse(data)
          if (typeof msg.id === 'number' && state.pendingRequests.has(msg.id)) {
            const pending = state.pendingRequests.get(msg.id)!
            if (pending.timer) clearTimeout(pending.timer)
            state.pendingRequests.delete(msg.id)
            if (msg.error) pending.reject(new Error(msg.error.message))
            else pending.resolve(msg.result)
          } else {
            ctx.handleServerMessage(id, msg, () => { /* SSE is one-way from server — ignore server-initiated */ })
          }
        } catch { /* malformed JSON, skip */ }
      }
    }
  }
}

/** Send a JSON-RPC request via POST to the SSE endpoint, await matching
 *  response on the open stream. */
export async function sseJsonRpcRequest(
  ctx: TransportContext,
  id: string,
  body: Record<string, unknown>,
): Promise<unknown> {
  const state = ctx.servers.get(id)!
  if (!state.sseMessageEndpoint) throw new Error('SSE endpoint not yet negotiated')
  const reqId = body.id as number
  const responsePromise = new Promise<unknown>((resolve, reject) => {
    const timer = setTimeout(() => {
      state.pendingRequests.delete(reqId)
      reject(new Error(`SSE request timed out (60s): ${body.method}`))
    }, 60_000)
    state.pendingRequests.set(reqId, { resolve, reject, timer })
  })
  const headers = await ctx.buildHttpHeaders(id)
  headers['Content-Type'] = 'application/json'
  const res = await fetch(state.sseMessageEndpoint, { method: 'POST', headers, body: JSON.stringify(body) })
  if (!res.ok && res.status !== 202) {
    state.pendingRequests.delete(reqId)
    throw new Error(`SSE POST ${res.status}: ${await res.text().catch(() => '')}`)
  }
  return responsePromise
}

/** Fire-and-forget JSON-RPC notification via SSE POST. */
async function sseSendNotification(
  ctx: TransportContext,
  id: string,
  body: Record<string, unknown>,
): Promise<void> {
  const state = ctx.servers.get(id)!
  if (!state.sseMessageEndpoint) throw new Error('SSE endpoint not yet negotiated')
  const headers = await ctx.buildHttpHeaders(id)
  headers['Content-Type'] = 'application/json'
  await fetch(state.sseMessageEndpoint, { method: 'POST', headers, body: JSON.stringify(body) }).catch(() => {})
}

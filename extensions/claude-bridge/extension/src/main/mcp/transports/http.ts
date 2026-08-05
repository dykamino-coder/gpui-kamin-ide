// Streamable HTTP transport for MCP servers (2024-11-05 unified POST).
//
// One-shot JSON-RPC over POST. Responses may be plain application/json or
// text/event-stream (for servers that stream progress events alongside the
// reply). OAuth 401 handling lives here: on first 401 we attempt RFC 9728
// protected-resource-metadata discovery, persist the auth server URL, and
// ask the user to re-authorize; on subsequent 401s we try a refresh once
// before surfacing the error.

import type { McpResult } from '../tool-registry'
import { collectMcpList } from '../pagination'
import type { TransportContext } from './context'

export async function connectHttp(ctx: TransportContext, id: string): Promise<void> {
  const state = ctx.servers.get(id)!
  const url = state.config.url
  if (!url) throw new Error('HTTP server URL is required')

  ctx.appendLog(id, 'info', `[http] POST ${url} → initialize`)
  const initResult = await httpJsonRpcAuth(ctx, id, {
    jsonrpc: '2.0',
    id: state.nextRequestId++,
    method: 'initialize',
    params: {
      protocolVersion: '2024-11-05',
      capabilities: { sampling: {}, elicitation: {} },
      clientInfo: { name: 'open-claude-bridge', version: '4.0.0' },
    },
  })

  if (initResult.error) {
    ctx.appendLog(id, 'error', `[http] initialize error: ${initResult.error.message}`)
    throw new Error(initResult.error.message)
  }
  state.capabilities = initResult.result?.capabilities ?? {}
  const serverInfo = initResult.result?.serverInfo
  ctx.appendLog(id, 'info', serverInfo?.name
    ? `[http] ← initialize ok: server "${serverInfo.name}" v${serverInfo.version ?? '?'}`
    : '[http] ← initialize ok')

  await httpJsonRpcAuth(ctx, id, {
    jsonrpc: '2.0',
    method: 'notifications/initialized',
  })

  ctx.appendLog(id, 'info', '[http] → tools/list')
  const tools = await collectMcpList<any>(async (method, params) => {
    const response = await httpJsonRpcAuth(ctx, id, {
      jsonrpc: '2.0', id: state.nextRequestId++, method, params,
    })
    if (response.error) throw new Error(response.error.message)
    return response.result
  }, 'tools/list', 'tools')
  state.tools = tools.map((tool: { name: string }) => tool.name)
  state.toolSchemas.clear()
  for (const tool of tools) {
    state.toolSchemas.set(tool.name, tool)
  }
  ctx.appendLog(id, 'info', `[http] ← tools/list: ${state.tools.length} tools (${state.tools.slice(0, 5).join(', ')}${state.tools.length > 5 ? ', …' : ''})`)
}

export async function callHttpTool(
  ctx: TransportContext,
  id: string,
  toolName: string,
  input: Record<string, unknown>,
): Promise<McpResult> {
  const state = ctx.servers.get(id)!

  const result = await httpJsonRpcAuth(ctx, id, {
    jsonrpc: '2.0',
    id: state.nextRequestId++,
    method: 'tools/call',
    params: { name: toolName, arguments: input },
  })

  if (result.error) {
    return { content: [{ type: 'text', text: `MCP Error: ${result.error.message}` }] }
  }

  if (result.result?.content && Array.isArray(result.result.content)) {
    return result.result
  }

  return { content: [{ type: 'text', text: typeof result.result === 'string' ? result.result : JSON.stringify(result.result) }] }
}

/**
 * Auth-aware JSON-RPC: builds headers (incl. Bearer for OAuth servers), sends,
 * retries once on 401 after refreshing the token. Throws on unrecoverable 401
 * so the UI can prompt for re-authorization.
 */
export async function httpJsonRpcAuth(
  ctx: TransportContext,
  id: string,
  body: Record<string, unknown>,
): Promise<{ result?: any; error?: { code: number; message: string } }> {
  const state = ctx.servers.get(id)!
  const url = state.config.url!

  let headers = await ctx.buildHttpHeaders(id)
  let res = await httpFetch(url, headers, body)

  if (res.status === 401) {
    // RFC 9728: server may advertise its authorization server via the
    // `WWW-Authenticate: Bearer resource_metadata="<url>"` challenge.
    // If we don't have OAuth configured yet, use that hint to auto-discover.
    if (!state.config.oauth) {
      const discovered = await tryDiscoverOAuthFrom401(ctx, id, res)
      if (discovered) {
        throw new Error(`OAuth required. Auto-detected authorization server: ${discovered}. Reconnect this server to authorize.`)
      }
      throw new Error(`HTTP 401 — authorization required`)
    }
    // Try refresh once, then retry
    const refreshed = await ctx.tryRefreshTokens(id)
    if (!refreshed) {
      throw new Error('OAuth authorization required — please reconnect this server')
    }
    headers = await ctx.buildHttpHeaders(id)
    res = await httpFetch(url, headers, body)
    if (res.status === 401) {
      throw new Error('OAuth authorization expired — please reconnect this server')
    }
  }

  if (res.status === 202) return {}
  if (!res.ok) {
    throw new Error(`HTTP ${res.status}: ${await res.text()}`)
  }

  const contentType = res.headers.get('content-type') || ''

  if (contentType.includes('text/event-stream')) {
    const text = await res.text()
    // Streamable HTTP: a single response body may include multiple
    // JSON-RPC messages — our response, plus server-initiated requests
    // or progress notifications. We dispatch server messages through the
    // normal handler and return only the message that matches our id.
    const requestId = (body as any).id
    let matched: any = {}
    for (const line of text.split('\n')) {
      if (!line.startsWith('data: ')) continue
      let parsed: any
      try { parsed = JSON.parse(line.slice(6)) } catch { continue }
      if (parsed.id === requestId && requestId !== undefined) {
        matched = parsed
      } else {
        // Notifications / server requests — dispatch. Server requests over
        // streaming HTTP can't be answered inside this one-shot request, so
        // we log and skip (MCP servers typically use WS/stdio for that).
        ctx.handleServerMessage(id, parsed, () => { /* no reply channel */ })
      }
    }
    return matched
  }

  return res.json() as Promise<{ result?: any; error?: { code: number; message: string } }>
}

/**
 * Parse the WWW-Authenticate challenge on a 401 response and, if it points
 * to a Protected Resource Metadata document (RFC 9728), fetch that doc and
 * populate the server's `oauth.authServerUrl` so the user can immediately
 * run the authorize flow. Returns the discovered AS URL or null.
 */
async function tryDiscoverOAuthFrom401(
  ctx: TransportContext,
  id: string,
  res: Response,
): Promise<string | null> {
  const challenge = res.headers.get('www-authenticate') || res.headers.get('WWW-Authenticate')
  if (!challenge) return null
  const match = challenge.match(/resource_metadata\s*=\s*"?([^",\s]+)"?/i)
  if (!match) return null
  const prmUrl = match[1]
  try {
    // Same mild check as for the AS URL below — accept http(s) only.
    const prmParsed = new URL(prmUrl)
    if (prmParsed.protocol !== 'http:' && prmParsed.protocol !== 'https:') return null
    const prmRes = await fetch(prmUrl)
    if (!prmRes.ok) return null
    const prm = await prmRes.json() as any
    const authServers: unknown = prm?.authorization_servers
    if (!Array.isArray(authServers) || authServers.length === 0) return null
    const asRaw = authServers[0]
    if (typeof asRaw !== 'string') return null
    // Minimal safety: only accept well-formed http(s) URLs. Corporate
    // networks legitimately use http:// internal hosts, so we don't block
    // those — but we do block file://, javascript:, data: and malformed
    // inputs that would crash `new URL()` downstream.
    let parsed: URL
    try { parsed = new URL(asRaw) } catch { return null }
    if (parsed.protocol !== 'http:' && parsed.protocol !== 'https:') return null
    const asUrl = parsed.toString().replace(/\/$/, '')
    const scopes: string[] | undefined = Array.isArray(prm.scopes_supported) ? prm.scopes_supported : undefined

    ctx.setDiscoveredOAuthMetadata(id, asUrl, scopes)
    return asUrl
  } catch {
    return null
  }
}

async function httpFetch(url: string, headers: Record<string, string>, body: Record<string, unknown>) {
  try {
    return await fetch(url, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        'Accept': 'application/json, text/event-stream',
        ...headers,
      },
      body: JSON.stringify(body),
    })
  } catch (err: any) {
    // Node's undici throws a bare `TypeError: fetch failed` with the real
    // cause buried on `.cause`. Unwrap it so the user sees what actually
    // went wrong (DNS, connection refused, TLS error, etc.) in the MCP
    // test log instead of a useless "fetch failed".
    const cause: any = err?.cause
    const host = (() => { try { return new URL(url).host } catch { return url } })()
    const parts: string[] = []
    parts.push(err instanceof Error ? err.message : String(err))
    if (cause?.code) parts.push(`code=${cause.code}`)
    if (cause?.errno) parts.push(`errno=${cause.errno}`)
    if (cause?.hostname) parts.push(`host=${cause.hostname}`)
    if (cause?.syscall) parts.push(`syscall=${cause.syscall}`)
    if (cause?.address) parts.push(`addr=${cause.address}`)
    if (cause?.port) parts.push(`port=${cause.port}`)
    if (cause instanceof Error && cause.message && cause.message !== err?.message) parts.push(`cause="${cause.message}"`)
    const detailed = parts.join(' · ')
    const wrapped = new Error(`POST ${url} → ${detailed}`)
    ;(wrapped as any).cause = cause ?? err
    ;(wrapped as any).hint = suggestFetchHint(cause, host)
    throw wrapped
  }
}

function suggestFetchHint(cause: any, host: string): string | null {
  if (!cause) return null
  const code = cause?.code || cause?.cause?.code
  if (code === 'ENOTFOUND') return `DNS lookup failed — is "${host}" reachable from this machine? (VPN/corporate DNS?)`
  if (code === 'ECONNREFUSED') return `Nothing listening on ${host} — check the port and that the service is running.`
  if (code === 'ECONNRESET') return `Connection reset by ${host} — remote closed mid-handshake (TLS mismatch, firewall, service crashed).`
  if (code === 'CERT_HAS_EXPIRED' || code === 'UNABLE_TO_VERIFY_LEAF_SIGNATURE' || code === 'SELF_SIGNED_CERT_IN_CHAIN') {
    return `TLS certificate problem (${code}) — for private endpoints set NODE_TLS_REJECT_UNAUTHORIZED=0 in the server env, or install the corp CA root.`
  }
  if (code === 'ETIMEDOUT') return `Connection to ${host} timed out — firewall or VPN may be blocking.`
  if (String(code).startsWith('DEPTH_ZERO')) return `TLS chain validation failed — likely missing corp CA root.`
  return null
}

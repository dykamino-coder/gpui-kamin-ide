// ============================================================================
// Request Context Middleware
// Creates a RequestContext for each request with reqId, startTime,
// endpoint type detection. Authentication is only for dashboard routes.
// MCP routes are authenticated by session ID; WS routes by token.
// ============================================================================

import type { Context, Next } from "hono"

export interface RequestContext {
  reqId: string | undefined
  startTime: number
  endpoint: "health" | "mcp" | "dashboard"
  userName: string | undefined
  tokenId: string | undefined
}

let nextId = 1
function generateReqId(): string {
  return `req_${Date.now()}_${nextId++}`
}

/**
 * Detect the endpoint type from the request path.
 */
function detectEndpoint(path: string): RequestContext["endpoint"] {
  if (path.startsWith("/mcp/")) return "mcp"
  if (path.includes("/api/dashboard")) return "dashboard"
  return "health"
}

export async function requestContextMiddleware(c: Context, next: Next) {
  const endpoint = detectEndpoint(c.req.path)

  const ctx: RequestContext = {
    reqId: endpoint === "health" ? undefined : generateReqId(),
    startTime: Date.now(),
    endpoint,
    userName: undefined,
    tokenId: undefined,
  }

  c.set("reqCtx", ctx)
  await next()
}

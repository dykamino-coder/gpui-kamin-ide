// ============================================================================
// MCP tool call flow (Server ↔ Electron)
// ============================================================================

export interface McpToolRequest {
  requestId: string
  sessionId: string
  toolName: string
  input: Record<string, unknown>
}

export interface McpToolResponse {
  requestId: string
  result: unknown
  durationMs: number
}

export interface McpToolDenied {
  requestId: string
  reason: string
}

// ---------------------------------------------------------------------------
// MCP pending call tracking
// ---------------------------------------------------------------------------

export interface PendingMcpCall {
  resolve: (result: unknown) => void
  reject: (error: Error) => void
  timer: ReturnType<typeof setTimeout> | null
  toolName: string
  createdAt: number
  /** Session that owns this call — lets the reaper skip sessions with
   *  inflight work, and lets graceful shutdown reject only the relevant
   *  calls. Required since 6.2.58. */
  sessionId: string
  /** Original tool input — kept so an undelivered call can be re-sent when
   *  the client reattaches after a WS drop. */
  input: Record<string, unknown>
  /** Whether the mcp:call frame actually went out on an OPEN socket. Calls
   *  issued while the session was detached are re-sent on reattach; calls
   *  that were delivered are NOT (the surviving client may still be
   *  executing them — re-sending would double side effects). */
  delivered: boolean
}

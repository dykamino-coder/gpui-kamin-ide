// ============================================================================
// Elicitation (MCP elicitation/create)
// ============================================================================

export interface PendingElicitation {
  /** Resolve the MCP HTTP response when user responds */
  resolve: (result: ElicitationResult) => void
  /** Reject on timeout/error */
  reject: (error: Error) => void
  /** Tool name this elicitation is for (e.g. AskUserQuestion, ExitPlanMode) */
  toolName: string
  /** The message/question shown to user */
  message: string
  /** JSON Schema for expected response */
  requestedSchema?: Record<string, unknown>
  createdAt: number
  /** JSON-RPC request id for response matching */
  jsonRpcId?: string | number
  /**
   * Deferred mode — HTTP response was already sent with a placeholder; when the
   * user eventually answers, deliver it via PTY stdin as a normal user message
   * instead of the dead HTTP promise. Set when we bypass the hanging HTTP flow
   * to avoid client-side elicitation timeouts on long waits.
   */
  deferred?: boolean
}

export interface ElicitationResult {
  action: 'accept' | 'deny' | 'dismiss'
  content?: Record<string, unknown>
}

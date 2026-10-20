// Result of one project session tool call before MCP wrapping and access logging.
export interface ToolOutput {
  /** Text blocks: the body first, then a JSON page/summary footer if any. */
  blocks: string[]
  /** Target conversation, for the access log (never its content). */
  conversationId?: string
  self?: boolean
  entries?: number
}

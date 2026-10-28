// Opt-in correlation metadata only: never accept tool arguments or output.
type Metadata = {
  requestId?: string; sessionId?: string; toolName?: string
  bytes?: number; queuedBytes?: number; queuedCount?: number; durationMs?: number
}

export function mcpDiagnostic(phase: string, metadata: Metadata): void {
  if (process.env.KAMIN_BRIDGE_MCP_DIAGNOSTICS !== '1') return
  console.info('[bridge:mcp]', JSON.stringify({ timestamp: Date.now(), phase, ...metadata }))
}

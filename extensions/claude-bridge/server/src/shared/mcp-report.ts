export interface McpUsageMetrics {
  schemaBytes: number
  observedSessions: number
  unusedSessions: number
  incompleteSessions: number
  calls: number
  lastCall: string | null
  wastedBytes: number
}

export interface McpUsageBreakdown extends McpUsageMetrics {
  userName: string
  project: string
}

export interface McpUsageRow extends McpUsageMetrics {
  toolName: string
  /** Bridge built-ins also travel through MCP; native CLI tools are excluded. */
  source: string
  breakdown: McpUsageBreakdown[]
}

export interface McpServerUsage extends McpUsageMetrics {
  source: string
  toolCount: number
  breakdown: McpUsageBreakdown[]
}

export interface McpUsageReport {
  rows: McpUsageRow[]
  servers: McpServerUsage[]
  sessionLimit: number
  observedSessions: number
  wastedBytes: number
}

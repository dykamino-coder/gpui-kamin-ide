export interface McpUsageRow {
  userName: string
  project: string
  toolName: string
  schemaBytes: number
  observedSessions: number
  unusedSessions: number
  calls: number
}

export interface McpUsageReport {
  rows: McpUsageRow[]
  sessionLimit: number
  truncated: boolean
}

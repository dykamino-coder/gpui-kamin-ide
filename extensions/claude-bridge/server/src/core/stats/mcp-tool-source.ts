import { MCP_TOOLS } from '../pty/mcp-tools-defs'
import { PROJECT_SESSION_TOOLS } from '../project-sessions/tools'
import type { ToolDefinition } from '../pty/types'

const builtins = new Map<string, ToolDefinition>([...MCP_TOOLS, ...PROJECT_SESSION_TOOLS].map((t) => [t.name, t]))

export function builtinMcpTool(name: string): ToolDefinition | undefined {
  return builtins.get(name)
}

export function mcpToolSource(tool: Pick<ToolDefinition, 'name' | 'serverName'>): string {
  if (builtins.has(tool.name)) return 'built-in'
  return tool.serverName || 'MCP (server unknown)'
}

/** Only the advertised wire definition contributes catalog bytes. */
export function mcpCatalogBytes(tool: ToolDefinition): number {
  return Buffer.byteLength(
    JSON.stringify({ name: tool.name, description: tool.description, inputSchema: tool.inputSchema }),
    'utf8',
  )
}

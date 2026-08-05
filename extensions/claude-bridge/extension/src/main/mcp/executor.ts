// MCP Tool Executor — dispatches tool calls to appropriate handlers

import { toolRegistry, type McpResult } from './tool-registry'

// External MCP server manager (set by main process after creation)
let externalManager: {
  hasExternalTool(name: string): boolean
  callTool(name: string, input: Record<string, unknown>): Promise<McpResult>
  readCatalogResource(uri: string, templateServerId?: string): Promise<unknown>
  getCatalogPrompt(name: string, args?: Record<string, unknown>): Promise<unknown>
} | null = null

export function setExternalMcpManager(manager: typeof externalManager): void {
  externalManager = manager
}

/**
 * Execute an MCP tool by name with the given input parameters.
 *
 * Returns an MCP-formatted result with content array.
 * Never throws — errors are returned as text content.
 *
 * Priority: built-in tools > external MCP servers
 */
export interface ToolCallContext {
  /** Source tab that issued this MCP call. Used to route UI widgets back to the right chat. */
  tabId?: string | null
}

export async function executeTool(
  toolName: string,
  input: Record<string, unknown>,
  context?: ToolCallContext,
): Promise<McpResult> {
  if (toolName === '__bridge_mcp_resource_read') {
    if (!externalManager || typeof input.uri !== 'string') throw new Error('External MCP resource bridge is unavailable')
    const templateServerId = typeof input.serverId === 'string' ? input.serverId : undefined
    return await externalManager.readCatalogResource(input.uri, templateServerId) as McpResult
  }
  if (toolName === '__bridge_mcp_prompt_get') {
    if (!externalManager || typeof input.name !== 'string') throw new Error('External MCP prompt bridge is unavailable')
    return await externalManager.getCatalogPrompt(input.name, input.arguments as Record<string, unknown> | undefined) as McpResult
  }
  // 1. Check built-in tool registry
  const handler = toolRegistry.get(toolName)

  if (handler) {
    try {
      return await handler(input, context)
    } catch (err: unknown) {
      const message = err instanceof Error ? err.message : String(err)
      const stack = err instanceof Error ? err.stack : undefined

      return {
        content: [{
          type: 'text',
          text: `Error executing tool "${toolName}": ${message}${stack ? `\n\nStack trace:\n${stack}` : ''}`,
        }],
      }
    }
  }

  // 2. Check external MCP servers
  if (externalManager?.hasExternalTool(toolName)) {
    try {
      return await externalManager.callTool(toolName, input)
    } catch (err: unknown) {
      const message = err instanceof Error ? err.message : String(err)
      return {
        content: [{
          type: 'text',
          text: `Error executing external tool "${toolName}": ${message}`,
        }],
      }
    }
  }

  // 3. Unknown tool
  const builtinTools = Array.from(toolRegistry.keys()).join(', ')
  const externalTools = externalManager
    ? ` External tools: (check MCP settings)`
    : ''
  return {
    content: [{
      type: 'text',
      text: `Error: Unknown tool "${toolName}". Built-in tools: ${builtinTools}.${externalTools}`,
    }],
  }
}

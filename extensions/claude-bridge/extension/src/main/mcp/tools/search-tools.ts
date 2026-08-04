// ToolSearch — search available MCP tools by keyword

import type { McpResult } from '../tool-registry'
import { getRegisteredTools } from '../tool-registry'

/**
 * ToolSearch — search among available tools by keyword.
 * Returns matching tools with descriptions.
 */
export async function toolSearch(input: Record<string, unknown>): Promise<McpResult> {
  const query = ((input.query as string) || '').toLowerCase()
  const maxResults = (input.max_results as number) || 5

  const allTools = getRegisteredTools()

  if (!query) {
    return {
      content: [{
        type: 'text',
        text: `Available tools (${allTools.length}):\n${allTools.join(', ')}`,
      }],
    }
  }

  const matches = allTools.filter(name =>
    name.toLowerCase().includes(query)
  ).slice(0, maxResults)

  if (matches.length === 0) {
    return {
      content: [{
        type: 'text',
        text: `No tools found matching "${query}". Available tools: ${allTools.join(', ')}`,
      }],
    }
  }

  return {
    content: [{
      type: 'text',
      text: `Tools matching "${query}" (${matches.length}):\n${matches.join('\n')}`,
    }],
  }
}

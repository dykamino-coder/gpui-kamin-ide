// Workspace preview tools share normal MCP dispatch; no page owns their state.
import { commands } from 'vscode'
import type { McpResult } from '../tool-registry'

async function run(operation: string, input: unknown): Promise<McpResult> {
  const result = await commands.executeCommand(`kaminBridge.preview.${operation}`, input)
  return { content: [{ type: 'text', text: JSON.stringify(result ?? { mounted: false }) }] }
}

export const previewMount = (input: Record<string, unknown>) => run('mount', input)
export const previewUpdate = (input: Record<string, unknown>) => run('update', input.props)
export const previewUnmount = (): Promise<McpResult> => run('unmount', undefined)

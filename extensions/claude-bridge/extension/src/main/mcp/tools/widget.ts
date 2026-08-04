// ShowWidget — a display-only MCP tool. The actual rendering happens in the
// webview (JsonlToolUse sees the tool_use and draws `input.html` in a sandboxed
// frame inline in the chat); this handler just validates + acknowledges so the
// CLI's tools/call gets a result and the turn continues.

import type { McpResult } from '../tool-registry'

export async function showWidget(input: Record<string, unknown>): Promise<McpResult> {
  const html = typeof input.html === 'string' ? input.html : ''
  if (!html.trim()) {
    return { content: [{ type: 'text', text: 'Error: `html` is required (a self-contained HTML fragment).' }] }
  }
  // Nothing to run host-side — the widget is rendered from the tool_use input.
  return { content: [{ type: 'text', text: 'Widget rendered inline in the chat.' }] }
}

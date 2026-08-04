// Single source of truth for the UI-visible list of bridge-provided
// (built-in) MCP tools. Both `ConnectorsList` and `ConnectorDetail` render
// from this array.
//
// This is a CURATED subset of what the server advertises via tools/list —
// LSP tools (LspDiagnostics, LspHover, LspDefinition, LspReferences) are
// intentionally hidden because they only work when a plugin provides a
// language server, and showing them upfront would confuse users with no
// plugins installed.
//
// The canonical full schema list lives server-side in
// `src/core/pty/mcp-http-handler.ts:MCP_TOOLS`. If you add a new bridge
// tool that should show in the panel, add the name here too.

export const BUILTIN_TOOLS: readonly string[] = [
  'Read', 'Write', 'Edit', 'Glob', 'Grep',
  'Bash', 'PowerShell', 'NotebookEdit',
  'WebFetch', 'EnterWorktree', 'ExitWorktree', 'AskUserQuestion',
  'PushNotification', 'TodoWrite', 'TodoList',
]

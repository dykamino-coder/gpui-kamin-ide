// Bridge MCP tools that wait for the user and therefore have no Bridge-side
// deadline (session-mcp-call.ts). Shared with the codex config, which must
// give them an equally unbounded engine-side timeout.
export const INTERACTIVE_TOOLS: ReadonlySet<string> = new Set(['AskUserQuestion', 'ExitPlanMode', 'EnterPlanMode'])

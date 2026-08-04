import { readFile, writeFile, editFile, globSearch, grepSearch } from './tools/file-system'
import { bashExecute, powerShellExecute, readBackgroundOutput, killBackgroundShell } from './tools/shell'
import { notebookEdit } from './tools/notebook'
import { webFetch } from './tools/web'
import { enterWorktree, exitWorktree } from './tools/git'
import { agentSpawn, teamCreate, teamDelete, sendMessage, taskCreate, taskGet, taskUpdate, taskList, taskOutput, taskStop } from './tools/agent-tools'
import { askUserQuestion, enterPlanMode, exitPlanMode, skillLoad } from './tools/ui-tools'
import { pushNotification, todoWrite, todoList } from './tools/notifications'
import { showWidget } from './tools/widget'
import { toolSearch } from './tools/search-tools'
import { lspDiagnostics, lspHover, lspDefinition, lspReferences } from './tools/lsp'

export type McpResult = { content: Array<{ type: string; text?: string; data?: string; mimeType?: string }> }
export interface ToolContext { tabId?: string | null }
export type ToolHandler = (input: Record<string, unknown>, context?: ToolContext) => Promise<McpResult>

/**
 * Map of tool name to handler function.
 * Each handler receives input parameters and returns MCP-formatted result.
 *
 * Only tools that match CLI's built-in tool names are registered here.
 * LS, NotebookRead, MultiEdit — NOT real CLI tools, removed.
 */
export const toolRegistry: Map<string, ToolHandler> = new Map<string, ToolHandler>([
  // ── File system ─────────────────────────────────────
  ['Read', readFile],
  ['Write', writeFile],
  ['Edit', editFile],
  ['Glob', globSearch],
  ['Grep', grepSearch],

  // ── Shell ───────────────────────────────────────────
  ['Bash', bashExecute],
  ['PowerShell', powerShellExecute],
  // Background-shell polling/termination — the run_in_background output was
  // captured but had no reader until these landed (native TaskOutput can't see
  // client-side background shells, and the bridge disables native bg tasks).
  ['BashOutput', readBackgroundOutput],
  ['KillShell', killBackgroundShell],

  // ── Notebook ────────────────────────────────────────
  ['NotebookEdit', notebookEdit],

  // ── Web ─────────────────────────────────────────────
  ['WebFetch', webFetch],

  // ── Git ─────────────────────────────────────────────
  ['EnterWorktree', enterWorktree],
  ['ExitWorktree', exitWorktree],

  // ── Agent & Tasks ───────────────────────────────────
  ['Agent', agentSpawn],
  ['TeamCreate', teamCreate],
  ['TeamDelete', teamDelete],
  ['SendMessage', sendMessage],
  ['TaskCreate', taskCreate],
  ['TaskGet', taskGet],
  ['TaskUpdate', taskUpdate],
  ['TaskList', taskList],
  ['TaskOutput', taskOutput],
  ['TaskStop', taskStop],

  // ── UI tools ────────────────────────────────────────
  ['AskUserQuestion', askUserQuestion],
  ['EnterPlanMode', enterPlanMode],
  ['ExitPlanMode', exitPlanMode],
  ['Skill', skillLoad],
  ['PushNotification', pushNotification],
  ['ShowWidget', showWidget],
  ['TodoWrite', todoWrite],
  ['TodoList', todoList],

  // ── Search ──────────────────────────────────────────
  ['ToolSearch', toolSearch],

  // ── LSP (plugin-provided language servers) ──────────
  //
  // These are always registered but return an error when no plugin LSP
  // server knows the file's extension. Claude can probe with Diagnostics
  // and fall back to Grep/Read if the result is "no server for X".
  ['LspDiagnostics', lspDiagnostics],
  ['LspHover', lspHover],
  ['LspDefinition', lspDefinition],
  ['LspReferences', lspReferences],
])

/**
 * Get all registered tool names.
 */
export function getRegisteredTools(): string[] {
  return Array.from(toolRegistry.keys())
}

/**
 * Check if a tool is registered.
 */
export function hasToolHandler(toolName: string): boolean {
  return toolRegistry.has(toolName)
}

/**
 * Get the handler for a tool.
 */
export function getToolHandler(toolName: string): ToolHandler | undefined {
  return toolRegistry.get(toolName)
}

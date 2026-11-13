// The `if` condition of a hook handler written for a native tool, in a Bridge
// session where that tool is `mcp__user-tools__<Tool>` (INC-2026-0107). The
// CLI compares the rule's tool name with the called tool and has no argument
// matcher for an MCP tool, so `Bash` and `Bash(git *)` both never hold. The
// CLI is given the Bridge tool name; an argument pattern is evaluated here,
// before the relayed hook is dispatched, with the native tool's rule.

import { NATIVE_REPLACED_TOOL_NAMES } from '../pty/mcp-tools-defs'
import { pathRuleMatches, type PathRuleContext } from './path-rule'
import { bashRuleMatches, powershellRuleMatches, wildcardMatches } from './rule-matchers'
import { BRIDGE_TOOL_PREFIX } from './tool-names'
import type { HookInputPayload } from './types'

/** Events whose `if` the CLI evaluates (the others never run a hook with `if`). */
const CONDITION_EVENTS: ReadonlySet<string> = new Set([
  'PreToolUse',
  'PostToolUse',
  'PostToolUseFailure',
  'PermissionRequest',
  'PermissionDenied',
])

export interface ParsedRule {
  tool: string
  content?: string
}

function unescapedIndex(text: string, ch: string, fromEnd: boolean): number {
  const positions: number[] = []
  for (let i = 0; i < text.length; i++) {
    if (text[i] !== ch) continue
    let backslashes = 0
    for (let j = i - 1; j >= 0 && text[j] === '\\'; j--) backslashes++
    if (backslashes % 2 === 0) positions.push(i)
  }
  if (positions.length === 0) return -1
  return fromEnd ? positions[positions.length - 1]! : positions[0]!
}

/** Permission-rule syntax as the CLI reads it: `Tool`, `Tool()` and `Tool(*)`
 *  name the whole tool; `\(` and `\)` escape parentheses in the content. A
 *  malformed rule is null: the CLI then compares the whole text as a name. */
export function parseRule(rule: string): ParsedRule | null {
  if (!rule.includes('(') && !rule.includes(')')) return { tool: rule }
  const open = unescapedIndex(rule, '(', false)
  const close = unescapedIndex(rule, ')', true)
  if (open === -1 && close === -1) return { tool: rule }
  const tool = rule.slice(0, open)
  if (open <= 0 || close <= open || close !== rule.length - 1 || /[()]/.test(tool)) return null
  const raw = rule.slice(open + 1, close)
  if (raw === '' || raw === '*') return { tool }
  return { tool, content: raw.replaceAll('\\(', '(').replaceAll('\\)', ')').replaceAll('\\\\', '\\') }
}

export interface CliCondition {
  /** The `if` to write into the CLI settings. */
  cliIf: string | undefined
  /** The rule that only the server can evaluate, or undefined. */
  serverRule?: Required<ParsedRule>
}

/** Translate a handler's `if` for the CLI. A rule naming a replaced native
 *  tool names the Bridge tool instead; its argument pattern, if any, is left
 *  to the server. Every other condition is passed on unchanged. */
export function conditionForCli(
  event: string,
  condition: string | undefined,
  nativeNames: readonly string[] = NATIVE_REPLACED_TOOL_NAMES,
): CliCondition {
  if (condition === undefined || !CONDITION_EVENTS.has(event)) return { cliIf: condition }
  const rule = parseRule(condition)
  if (!rule || !nativeNames.includes(rule.tool)) return { cliIf: condition }
  const cliIf = BRIDGE_TOOL_PREFIX + rule.tool
  return rule.content === undefined ? { cliIf } : { cliIf, serverRule: { tool: rule.tool, content: rule.content } }
}

function stringField(input: unknown, key: string): string | undefined {
  const value = input && typeof input === 'object' ? (input as Record<string, unknown>)[key] : undefined
  return typeof value === 'string' ? value : undefined
}

/** Whether a server-evaluated rule accepts this tool call, with the matcher
 *  the CLI has for the native tool. A native tool without an argument matcher
 *  never accepts a rule with content, as in the CLI. */
export function conditionHolds(
  rule: Required<ParsedRule>,
  payload: HookInputPayload,
  ctx: PathRuleContext = {},
): boolean {
  const toolName = typeof payload.tool_name === 'string' ? payload.tool_name : ''
  const called = toolName.startsWith(BRIDGE_TOOL_PREFIX) ? toolName.slice(BRIDGE_TOOL_PREFIX.length) : toolName
  if (called !== rule.tool) return false
  const input = payload.tool_input
  const context: PathRuleContext = {
    cwd: ctx.cwd ?? (typeof payload.cwd === 'string' ? payload.cwd : undefined),
    home: ctx.home,
  }
  const value = (key: string, match: (v: string) => boolean): boolean => {
    const v = stringField(input, key)
    return v !== undefined && match(v)
  }
  switch (rule.tool) {
    case 'Bash':
      return value('command', (v) => bashRuleMatches(rule.content, v))
    case 'PowerShell':
      return value('command', (v) => powershellRuleMatches(rule.content, v))
    case 'Read':
    case 'Write':
    case 'Edit':
      return value('file_path', (v) => pathRuleMatches(rule.content, v, context))
    case 'NotebookEdit':
      return value('notebook_path', (v) => pathRuleMatches(rule.content, v, context))
    case 'Glob':
    case 'Grep':
      return value('pattern', (v) => wildcardMatches(rule.content, v))
    default:
      return false
  }
}

// Hooks are written for plain Claude Code, where the file and shell tools are
// called `Write`, `Bash` and so on. In a Bridge session the same tools are
// served by the `user-tools` MCP server, and the CLI matches and reports them
// as `mcp__user-tools__Write`. This module makes a hook written for the native
// name work for the Bridge tool that replaces it.

import { NATIVE_REPLACED_TOOL_NAMES } from '../pty/mcp-tools-defs'
import type { HookInputPayload } from './types'

export const BRIDGE_TOOL_PREFIX = 'mcp__user-tools__'

/** Events whose matcher is compared with the tool name. */
const TOOL_EVENTS: ReadonlySet<string> = new Set([
  'PreToolUse',
  'PostToolUse',
  'PostToolUseFailure',
  'PermissionRequest',
  'PermissionDenied',
])

const EXACT_LIST = /^[a-zA-Z0-9_|, -]+$/

function matchesAll(matcher: string | undefined): boolean {
  return !matcher || matcher === '*'
}

/** Claude Code's matcher rule for tool events: a matcher made only of
 *  letters, digits, `_`, `-`, spaces, `,` and `|` is a list of exact names;
 *  anything else is an unanchored JavaScript regular expression. */
export function matcherAccepts(matcher: string | undefined, toolName: string): boolean {
  if (!matcher || matchesAll(matcher)) return true
  if (EXACT_LIST.test(matcher)) {
    return matcher
      .split(/[|,]/)
      .map((name) => name.trim())
      .includes(toolName)
  }
  try {
    return new RegExp(matcher).test(toolName)
  } catch {
    return false
  }
}

/** The matcher to give the CLI: the user's matcher, extended so that every
 *  native tool it names also matches the Bridge tool that replaces it. A
 *  matcher that already matches the Bridge name is left alone, so no hook
 *  runs twice. */
export function matcherForCli(
  event: string,
  matcher: string | undefined,
  nativeNames: readonly string[] = NATIVE_REPLACED_TOOL_NAMES,
): string | undefined {
  if (matcher === undefined || matchesAll(matcher) || !TOOL_EVENTS.has(event)) return matcher
  const added = nativeNames
    .filter((name) => matcherAccepts(matcher, name) && !matcherAccepts(matcher, BRIDGE_TOOL_PREFIX + name))
    .map((name) => BRIDGE_TOOL_PREFIX + name)
  if (added.length === 0) return matcher
  if (EXACT_LIST.test(matcher)) return [matcher, ...added].join('|')
  return `(?:${matcher})|^(?:${added.join('|')})$`
}

/** The hook input for a hook written for native names: `tool_name` is the
 *  native name when the hook's own matcher accepts it. A hook that matches
 *  only the MCP name keeps the MCP name. */
export function payloadForHook(
  matcher: string | undefined,
  payload: HookInputPayload,
  nativeNames: readonly string[] = NATIVE_REPLACED_TOOL_NAMES,
): HookInputPayload {
  const raw = (payload as { tool_name?: unknown }).tool_name
  if (typeof raw !== 'string' || !raw.startsWith(BRIDGE_TOOL_PREFIX)) return payload
  const native = raw.slice(BRIDGE_TOOL_PREFIX.length)
  if (!nativeNames.includes(native) || !matcherAccepts(matcher, native)) return payload
  return { ...payload, tool_name: native }
}

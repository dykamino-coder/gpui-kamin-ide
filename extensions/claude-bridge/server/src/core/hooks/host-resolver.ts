// Resolve the effective host for a hook handler given event/matcher/type.
// Default = 'local' for command-type hooks (host has access to user's
// filesystem, git, npm bins, IDE, native notifications). Server-side
// command hooks live in a near-empty container — they can only do pure
// JSON transformations, HTTP calls and bridge-internal writes. So local
// is the safe default; server is opt-in.

import type { HookEvent, HookHandler, HookHost } from './types'

/** Built-in CLI tools that run inside the CLI process — host has nothing
 *  to validate they don't already have access to. PreToolUse hooks
 *  matching these prefer server (no host round-trip cost). */
const CLI_INTERNAL_TOOL_MATCHERS = new Set([
  'Agent', 'TaskCreate', 'TaskGet', 'TaskUpdate', 'TaskList', 'TaskOutput', 'TaskStop',
  'SendMessage', 'WebSearch', 'Skill', 'ToolSearch',
  'EnterPlanMode', 'ExitPlanMode', 'AskUserQuestion', 'PushNotification',
  'TodoWrite', 'TodoList',
])

/** Events whose handlers most naturally read/write host state. */
const HOST_NATURAL_EVENTS = new Set<HookEvent>([
  'PostToolUse', 'PostToolUseFailure', 'Notification',
  'SessionStart', 'SessionEnd', 'Stop', 'Setup',
  'PreCompact',
  'PermissionRequest', 'MessageDisplay',
  'TeammateIdle',
  'BridgeReconnect', 'MicAudioDetected', 'ConnectorStatusChanged',
  'AutoUpdateAvailable', 'PluginInstalled', 'PluginUninstalled', 'MarketplaceUpdated',
])

/** Events where the payload is already known to bridge before the event
 *  fires (we synced it / proxied it ourselves) — local handler is technically
 *  possible but informationally redundant. We default to server but allow
 *  override. */
const SERVER_PREFERRED_EVENTS = new Set<HookEvent>([
  'UserPromptSubmit',          // per-turn latency cost on local, transformation OK on server
  'Elicitation',               // bridge proxied it itself
  'SubagentStart', 'SubagentStop',
  'TaskCompleted',             // postfactum, MCP-mirror works server-side
])

/**
 * Decide where a `command` hook physically runs.
 *
 *   - explicit user override (handler.host !== 'auto')   wins
 *   - non-command types (prompt/agent) always 'server'   (pure LLM)
 *   - command + 'http' as URL-target                     handled separately
 *   - PreToolUse with CLI-internal matcher               'server'
 *   - HOST_NATURAL_EVENTS                                'local'
 *   - SERVER_PREFERRED_EVENTS                            'server'
 *   - anything else                                      'local' (safe default)
 */
export function resolveHost(
  event: HookEvent,
  handler: HookHandler,
  matcher?: string,
): 'local' | 'server' | 'http' {
  // prompt/agent are pure LLM-calls — local doesn't add anything.
  if (handler.type === 'prompt' || handler.type === 'agent') return 'server'

  // http handler — kept as-is (CLI fetches the URL).
  if (handler.type === 'http') return 'http'

  // command type — check explicit override first.
  const explicit = (handler as { host?: HookHost }).host
  if (explicit && explicit !== 'auto') {
    if (explicit === 'http') return 'http'
    return explicit
  }

  // PreToolUse on CLI-internal tools → server (host can't help)
  if (event === 'PreToolUse' && matcher && CLI_INTERNAL_TOOL_MATCHERS.has(matcher)) {
    return 'server'
  }

  if (SERVER_PREFERRED_EVENTS.has(event)) return 'server'
  if (HOST_NATURAL_EVENTS.has(event)) return 'local'

  // Unknown / wildcard → local is the safe default (host strictly more capable).
  return 'local'
}

/** Plain reasons string for the UI to explain why a host was chosen. */
export function explainHost(
  event: HookEvent,
  handler: HookHandler,
  matcher?: string,
): string {
  const explicit = (handler as { host?: HookHost }).host
  if (explicit && explicit !== 'auto') return `User-set host: ${explicit}`
  if (handler.type === 'prompt' || handler.type === 'agent') {
    return 'LLM-call hook — runs in-place inside CLI'
  }
  if (handler.type === 'http') return 'HTTP hook — CLI fetches the URL directly'

  if (event === 'PreToolUse' && matcher && CLI_INTERNAL_TOOL_MATCHERS.has(matcher)) {
    return `Tool "${matcher}" runs inside CLI — host has nothing to validate`
  }
  if (SERVER_PREFERRED_EVENTS.has(event)) {
    return `Event "${event}" payload is already known to bridge — local would be redundant`
  }
  if (HOST_NATURAL_EVENTS.has(event)) {
    return `Event "${event}" handlers typically need host filesystem / git / IDE / notifications`
  }
  return 'Default to local — host environment is strictly more capable than container'
}

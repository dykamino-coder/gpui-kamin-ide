// Type definitions for the bridge hook system.
// Mirror Claude Code 2.1.198's hook schema (4 handler types, 17 events —
// verified against the actual CLI binary 2026-07-02) and add bridge-emit
// custom events. Events the CLI никогда не fires (StopFailure, PostCompact,
// PermissionDenied, TaskCreated, ElicitationResult, ConfigChange,
// WorktreeCreate/Remove, InstructionsLoaded, CwdChanged, FileChanged from the
// old 2.1.121 list) were dropped: configuring them produced silently-ignored
// settings.json keys.

export const CLI_HOOK_EVENTS = [
  'PreToolUse',
  'PostToolUse',
  'PostToolUseFailure',
  'Notification',
  'UserPromptSubmit',
  'SessionStart',
  'SessionEnd',
  'Stop',
  'SubagentStart',
  'SubagentStop',
  'PreCompact',
  'PermissionRequest',
  'MessageDisplay',
  'Elicitation',
  'TaskCompleted',
  'TeammateIdle',
  'Setup',
  // CLI 2.1.219: после /add-dir или SDK register_repo_root.
  'DirectoryAdded',
] as const
export type CliHookEvent = (typeof CLI_HOOK_EVENTS)[number]

export const BRIDGE_HOOK_EVENTS = [
  'BridgeReconnect',
  'MicAudioDetected',
  'ConnectorStatusChanged',
  'AutoUpdateAvailable',
  'PluginInstalled',
  'PluginUninstalled',
  'MarketplaceUpdated',
] as const
export type BridgeHookEvent = (typeof BRIDGE_HOOK_EVENTS)[number]
export type HookEvent = CliHookEvent | BridgeHookEvent

export type HookHost = 'auto' | 'local' | 'server' | 'http'
export type HookType = 'command' | 'prompt' | 'agent' | 'http'

/** Single hook handler config. Subset of Claude Code's HookCommandSchema +
 *  our `host:` annotation. */
export interface BashCommandHook {
  type: 'command'
  command: string
  if?: string
  shell?: 'bash' | 'powershell' | 'sh' | 'zsh'
  timeout?: number
  statusMessage?: string
  once?: boolean
  async?: boolean
  asyncRewake?: boolean
  /** Where to physically execute. 'auto' = bridge picks default. Default 'auto'. */
  host?: HookHost
}
export interface PromptHook {
  type: 'prompt'
  prompt: string
  if?: string
  timeout?: number
  model?: string
  statusMessage?: string
  once?: boolean
}
export interface AgentHook {
  type: 'agent'
  prompt: string
  if?: string
  timeout?: number
  model?: string
  statusMessage?: string
  once?: boolean
}
export interface HttpHookHandler {
  type: 'http'
  url: string
  if?: string
  timeout?: number
  headers?: Record<string, string>
  allowedEnvVars?: string[]
  statusMessage?: string
  once?: boolean
}
export type HookHandler = BashCommandHook | PromptHook | AgentHook | HttpHookHandler

export interface HookMatcher {
  matcher?: string
  hooks: HookHandler[]
}

/** settings.json:hooks shape — partial record, only events with hooks present. */
export type HookSettings = Partial<Record<HookEvent, HookMatcher[]>>

/** What we register in the per-session registry. The `id` is the route token
 *  inserted into the proxy URL so an incoming webhook can be matched back to
 *  its original handler. */
export interface RegisteredHook {
  id: string                  // uuid, unique per (session, event, handler) tuple
  sessionId: string
  event: HookEvent
  matcher?: string
  handler: HookHandler
  source: HookSource
  effectiveHost: 'local' | 'server' | 'http'  // resolved from handler.host + defaults
}

export type HookSource =
  | { kind: 'user' }
  | { kind: 'project'; projectPath: string }
  | { kind: 'plugin'; pluginId: string; manifestPath: string }
  | { kind: 'library'; templateId: string }
  | { kind: 'bridge' }                            // bridge-emit defaults

/** What an incoming webhook from CLI carries (per Claude Code hook input
 *  spec — the JSON written to hook's stdin OR posted as JSON body for http
 *  hooks). Mirrors src/entrypoints/sdk/coreSchemas.ts:BaseHookInputSchema +
 *  event-specific extensions. Optional fields per-event:
 *
 *   - PreToolUse / PostToolUse / PostToolUseFailure / PermissionDenied:
 *     tool_name, tool_input, tool_use_id; PostToolUse also tool_response
 *   - Notification:           message, title?, notification_type
 *   - UserPromptSubmit:       prompt
 *   - SessionStart:           source ∈ {startup, resume, clear, compact}
 *   - Setup:                  trigger ∈ {init, maintenance}
 *   - Stop:                   stop_hook_active, last_assistant_message?
 *   - StopFailure:            error, error_details?, last_assistant_message?
 *   - SubagentStart:          agent_id, agent_type
 *   - SubagentStop:           stop_hook_active, agent_id, agent_transcript_path,
 *                             agent_type, last_assistant_message?
 *   - PreCompact / PostCompact: trigger ∈ {manual, auto}, custom_instructions / compact_summary
 *   - WorktreeCreate / Remove: path
 *   - FileChanged:            path, change_type
 *   - CwdChanged:             new_cwd, previous_cwd
 *   - TaskCreated / Completed: task_id, task_subject, task_status
 *   - Elicitation*:           request_id, schema, response?
 *   - PermissionRequest:      tool_name, tool_input, permission_suggestions?
 *   - InstructionsLoaded:     loaded_instructions[]
 *   - ConfigChange:           changed_paths[]
 *   - TeammateIdle:           agent_id, idle_seconds
 */
export interface HookInputPayload {
  hook_event_name: HookEvent | string
  session_id: string
  transcript_path?: string
  cwd?: string
  permission_mode?: string
  agent_id?: string
  agent_type?: string
  /** Per-event fields. */
  [k: string]: unknown
}

/** Hook execution outcome — bridge → CLI shape (mirrors what a real script
 *  would print to stdout/stderr + exit code). */
export interface HookExecutionResult {
  stdout: string
  stderr: string
  exitCode: number
  outcome: 'success' | 'error' | 'timeout' | 'cancelled'
  /** Optional structured output a `command` hook can return as JSON on
   *  stdout — picked up by CLI for `permissionDecision`, `updatedToolOutput`,
   *  `additionalContext`, etc. Bridge passes it through verbatim. */
  jsonOutput?: Record<string, unknown>
  durationMs: number
}

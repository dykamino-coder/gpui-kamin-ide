// ============================================================================
// PTY Session — environment + CLI args + system prompt builders.
// Pure factories pulled out of `session-core.ts` so they can be unit-tested
// and so the core file shrinks below the 1k-LOC ceiling.
// ============================================================================

import { injectProxyEnv } from '../config/settings'
import { config, DEFAULT_SESSION_MODEL } from '../config'
import type { SessionConfig } from './types'
import { MCP_TOOL_NAMES } from './mcp-http-handler'
import { buildSystemPrompt } from './system-prompt'

export { buildSystemPrompt }

// ---------------------------------------------------------------------------
// Built-in tools blocked at the CLI flag level. Three layers:
//   1. `MCP_TOOL_NAMES` — every tool the bridge re-implements via MCP.
//   2. `EXTRA_NATIVE_DENY` — CLI internal tools we DON'T support remotely
//      (background tasks, cron, monitor).
//   3. `LEGACY_DENY` — vestigial names kept for safety.
// Native survivors: Agent, TaskCreate/etc, Skill, ToolSearch, WebSearch,
// EnterPlanMode/ExitPlanMode, Sleep — all kept. (TeamCreate/TeamDelete were
// removed from the CLI in 2.1.178 — every session now has one implicit team —
// so they're no longer named here.)
//
// SendMessage was here on the assumption that inter-agent messaging needs
// remote/host support — it does NOT. It is a native CLI tool that runs fully
// in-process against file mailboxes under `~/.claude/teams/` (no MCP, no host
// round-trip, no Read/exec dependency), and `CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS`
// is already on (session-settings). Denying it just cut teammates off from the
// team-lead — they could only return via final text. Allowed now.
// ---------------------------------------------------------------------------
export const EXTRA_NATIVE_DENY = [
  'Monitor', 'CronCreate', 'CronDelete', 'CronList',
  'RemoteTrigger',
]
// The old 'ListMcpResources'/'ReadMcpResource' entries were misspelled (the
// real natives are ListMcpResourcesTool/ReadMcpResourceTool) so they never
// denied anything. Dropped — the bridge now answers resources/list itself
// (empty), so the natives resolve cleanly instead of erroring.
// LEGACY_DENY выпилен: TodoRead не существует в CLI годами (аудит 2.1.220).
export const LEGACY_DENY: string[] = []
/** Lazily-resolved (avoids import-time circular evaluation with
 *  mcp-http-handler.ts: that module triggers session-manager, which re-exports
 *  this file → spreading MCP_TOOL_NAMES at module load throws "not iterable"
 *  during the initialisation half-order). At runtime when buildClaudeArgs is
 *  invoked, the constant is fully populated. */
export function getDisallowedBuiltinTools(): string[] {
  return [...MCP_TOOL_NAMES, ...EXTRA_NATIVE_DENY, ...LEGACY_DENY]
}
/** @deprecated kept for tests — prefer `getDisallowedBuiltinTools()` to get a
 *  freshly-evaluated list that's safe under cyclic imports. */
export const DISALLOWED_BUILTIN_TOOLS = new Proxy([] as string[], {
  get(_t, prop) {
    const arr = getDisallowedBuiltinTools()
    return (arr as any)[prop]
  },
})

/**
 * Compute the env block handed to the CLI process. Heavy on hard-coded
 * Anthropic / Claude-Code feature flags — see inline comments for why
 * each one is set / unset.
 */
export function buildSessionEnv(sessionId: string, userName: string, effort?: string): Record<string, string> {
  const env = { ...process.env } as Record<string, string>

  // Remove Claude Code session markers (avoid nesting)
  delete env.CLAUDECODE
  delete env.CLAUDE_CODE_SESSION

  // Inject proxy settings
  injectProxyEnv(env)

  // Skip ALL interactive dialogs (effort, trust, onboarding, grove, etc.)
  // IS_DEMO=1 bypasses the entire pcq() dialog gate function in CLI
  env.IS_DEMO = '1'

  // Pre-set effort level. MUST track the session's chosen effort, not a
  // hard-coded 'high': with IS_DEMO=1 the CLI reads this env as the authoritative
  // effort and it OVERRODE the `--effort` flag, so the effort selector silently
  // did nothing (every session ran 'high'). Now the env + the flag agree.
  env.CLAUDE_CODE_EFFORT_LEVEL = effort || 'high'

  // Suppress background API calls that waste quota:
  // - prompt_suggestion: forks context after every assistant turn
  // - session_memory: reads/writes .md after every turn
  // - background tasks: agent run_in_background
  // - file checkpointing: useless for bridge (files are on client)
  // NOTE: terminal title (topic detection via haiku) is ENABLED — it generates
  //       session summary that we use for resume labels and sidebar display.
  env.CLAUDE_CODE_ENABLE_PROMPT_SUGGESTION = 'false'
  env.CLAUDE_CODE_DISABLE_BACKGROUND_TASKS = '1'
  env.CLAUDE_CODE_DISABLE_AUTO_MEMORY = '1'
  env.CLAUDE_CODE_DISABLE_FILE_CHECKPOINTING = '1'

  // Per-CLI cost trims for the headless multi-session pool (verified against the
  // real 2.1.202 binary — every flag's read-site + gated feature grep-confirmed):
  //
  // NONESSENTIAL_TRAFFIC → "essential-traffic" mode: kills the per-session
  //   version-check, MCP-registry/directory fetch, Grove settings, Claude
  //   Projects, DesignSync, asset fetch and error-reporting (Sentry). Verified it
  //   sits on a SEPARATE branch from telemetry ("no-telemetry" is a different
  //   enum) and does NOT touch OTEL export, streaming, resume, MCP tool routing,
  //   or the terminal title — all of which we depend on.
  env.CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC = '1'
  // Memory subsystem: we already run with auto-memory off, so kill its two
  // background costs too — the 60-min periodic resync timer and the startup
  // multi-store bulk inflate.
  env.CLAUDE_CODE_DISABLE_MEMORY_PERIODIC_RESYNC = '1'
  env.CLAUDE_CODE_DISABLE_MEMORY_BULK_INFLATE = '1'
  // We route our own MCP and use no marketplace plugins, cron, or workflows —
  // stop the startup marketplace autoinstall and the cron/workflow schedulers.
  env.CLAUDE_CODE_DISABLE_OFFICIAL_MARKETPLACE_AUTOINSTALL = '1'
  env.CLAUDE_CODE_DISABLE_CRON = '1'
  env.CLAUDE_CODE_DISABLE_WORKFLOWS = '1'
  // Pin the flicker-free fullscreen (alt-screen, virtualized-scrollback)
  // renderer. Without this the renderer is chosen by a gradual-rollout feature
  // flag, so sessions land on EITHER fullscreen or the classic main-screen
  // renderer non-deterministically. Live frame-timing A/B (2.1.202, animated
  // turn): classic emits ~2× the per-frame terminal patches of fullscreen
  // (44 vs 22 avg) — the patches are exactly what our output hot-path processes.
  // Pinning fullscreen makes the per-session CPU both lower AND reproducible.
  env.CLAUDE_CODE_NO_FLICKER = '1'

  // Disable the "This session is X old / Y tokens — Resume from summary?"
  // dialog that CLI shows on resume of long-running sessions. Defaults are
  // 70 minutes / 100k tokens; setting both to absurdly large numbers keeps
  // the gate from ever firing. Bridge already routes resume through
  // settingsDir + JSONL so the user always wants "continue full session"
  // here — the dialog just steals focus and breaks PTY auto-flow when
  // the chat panel is hidden. Same trick neutralises the older
  // IdleReturnDialog ("You've been away …").
  env.CLAUDE_CODE_RESUME_THRESHOLD_MINUTES = '99999999'
  env.CLAUDE_CODE_RESUME_TOKEN_THRESHOLD = '999999999'
  env.CLAUDE_CODE_IDLE_THRESHOLD_MINUTES = '99999999'
  env.CLAUDE_CODE_IDLE_TOKEN_THRESHOLD = '999999999'

  // MCP_TIMEOUT in Claude Code is actually the CONNECTION timeout for the
  // MCP server handshake, not the per-tool-call timeout (per CLI source
  // `services/mcp/client.ts:getConnectionTimeoutMs()`; default 30s). For
  // tool calls CLI uses MCP_TOOL_TIMEOUT with a default of ~27.8h, so we
  // don't need to set it. Keeping MCP_TIMEOUT at 60s gives the bridge
  // plenty of room to come up under load without hanging forever on a
  // broken setup.
  env.MCP_TIMEOUT = '60000'

  // BASH_DEFAULT_TIMEOUT_MS / BASH_MAX_TIMEOUT_MS apply to CLI's built-in
  // Bash tool, which we disable via --disallowedTools. They do NOT propagate
  // to MCP-routed tools (verified in CLI source `tools/BashTool/prompt.ts`).
  // Still set defensively in case a future CLI version starts honouring
  // them for MCP — costs nothing if they're a no-op.
  env.BASH_DEFAULT_TIMEOUT_MS = '1500000' // 25 min default
  env.BASH_MAX_TIMEOUT_MS = '1800000'     // 30 min ceiling

  // Anthropic API request timeout. CLI 2.1.101 fixed a hardcoded 5min cap
  // that was ignoring this var — extended thinking and slow gateways
  // (Bedrock/Vertex with high-effort Opus 4.8) routinely exceed 5min.
  // 30 minutes lets long deliberations finish; the CLI's own stalled-stream
  // detection (5min no-data abort, also added in 2.1.105) handles genuine
  // hangs, so we don't risk masking real failures by raising this.
  env.API_TIMEOUT_MS = '1800000'

  // Raise MCP output token limit (default 25k) for tools returning big
  // results. Grep -C 20 on a 100-hit match easily produces 400+ KB of text
  // (~100k tokens). Below this, Claude Code spills the result to a tool-
  // results file and asks the model to read it back in chunks — functional
  // but clunky, and it eats more context than just returning the text
  // directly. 200k keeps us under MAX_GREP_BYTES (10 MB) which is our own
  // hard ceiling, while letting realistic grep results pass through cleanly.
  env.MAX_MCP_OUTPUT_TOKENS = '200000'

  // OpenTelemetry — send metrics to our OTLP receiver
  env.CLAUDE_CODE_ENABLE_TELEMETRY = '1'
  env.OTEL_METRICS_EXPORTER = 'otlp'
  env.OTEL_LOGS_EXPORTER = 'otlp'
  env.OTEL_EXPORTER_OTLP_PROTOCOL = 'http/json'
  env.OTEL_EXPORTER_OTLP_ENDPOINT = `http://127.0.0.1:${config.port}/otel`
  // Export every 30s (was 10s/5s). At N concurrent sessions each CLI POSTs OTLP
  // metrics+logs to /otel on this cadence and the server parses + writes them to
  // DuckDB; 30s cuts that per-session CPU/net/DB load ~3-6× — dashboard stats
  // just refresh a bit less often (correctness unchanged).
  env.OTEL_METRIC_EXPORT_INTERVAL = '30000'
  env.OTEL_LOGS_EXPORT_INTERVAL = '30000'
  env.OTEL_LOG_TOOL_DETAILS = '1'
  env.OTEL_RESOURCE_ATTRIBUTES = `user.name=${userName},session.custom_id=${sessionId}`

  return env
}

export function buildClaudeArgs(sessionConfig: SessionConfig): string[] {
  const args: string[] = [
    '--disallowedTools', getDisallowedBuiltinTools().join(','),
    '--dangerously-skip-permissions',
    '--effort', sessionConfig.effort || 'high',
  ]

  // Default every fresh session to DEFAULT_SESSION_MODEL unless the client
  // explicitly picked something else. Sessions that are --resume'd will pick up
  // their prior model from the JSONL before this arg kicks in (CLI gives
  // --resume precedence over --model for historical model of the conversation).
  const requestedModel = sessionConfig.model || DEFAULT_SESSION_MODEL
  // Опус 4.x выпилен (клиентский пикер его не выдаёт): легаси id из старых
  // сохранённых сессий не должен воскрешать снятую модель.
  // Граница после «4»: не зацепить гипотетический claude-opus-45.
  const effectiveModel = /claude-opus-4(?:[.-]|$|\[)/.test(requestedModel) ? DEFAULT_SESSION_MODEL : requestedModel
  args.push('--model', effectiveModel)

  // Resume a previous conversation
  if (sessionConfig.resumeConversationId) {
    args.push('--resume', sessionConfig.resumeConversationId)
  }

  // Inject system prompt with working directory info and MCP instructions
  const systemPrompt = buildSystemPrompt(sessionConfig.cwd, sessionConfig.basePrompt, sessionConfig.transcriptMirrorDir)
  args.push('--append-system-prompt', systemPrompt)

  if (sessionConfig.extraArgs) {
    args.push(...sessionConfig.extraArgs)
  }

  return args
}


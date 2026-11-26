// Codex has no client-side native hook runtime. Its authenticated MCP endpoint
// is the common execution boundary for workspace, external and Bridge tools.
import { createHash, randomUUID } from 'node:crypto'
import path from 'node:path'
import { withSyncSnapshotLock } from '../sync/lock'
import { getProjectSyncDir, getUserSyncDir } from '../sync/storage'
import { INTERACTIVE_TOOLS } from '../pty/interactive-tools'
import { record } from '../pty/mcp-modern'
import type { PtySession } from '../pty/types'
import { mergeClientHooks } from './source-merger'
import { clearSession, registerSessionHooks, listSession } from './registry'
import { dispatchHook } from './dispatcher'
import { BRIDGE_TOOL_PREFIX, matcherAccepts, matcherForCli } from './tool-names'
import { conditionHolds, parseRule } from './hook-condition'
import type { HookExecutionResult, HookInputPayload, RegisteredHook } from './types'

export const HOOK_SYNC_ERROR =
  'Workspace hooks could not be synchronized. Reconnect this chat after workspace sync succeeds.'

const events = ['PreToolUse', 'PostToolUse', 'PostToolUseFailure'] as const
type ToolEvent = (typeof events)[number]

const snapshots = new WeakMap<PtySession, string | null>()
export function registerCodexToolHooks(session: PtySession): void {
  if (!session.bearerHash) return
  const user = getUserSyncDir(session.bearerHash)
  const settings = [path.join(user, 'settings.json')]
  if (session.cwd) settings.push(path.join(getProjectSyncDir(session.bearerHash, session.cwd), 'settings.json'))
  let merged
  try {
    merged = mergeClientHooks(session.cwd, settings, path.join(user, 'plugins.json'))
  } catch (error) {
    snapshots.set(session, null)
    clearSession(session.id)
    throw error
  }
  const tools = Object.fromEntries(events.map((event) => [event, merged.hooks[event] ?? []]))
  const signature = createHash('sha256')
    .update(
      JSON.stringify(
        events.map((event) => (merged.hooks[event] ?? []).map((m) => [event, m, merged.sourceByMatcher.get(m)])),
      ),
    )
    .digest('hex')
  if (snapshots.get(session) === signature) return
  snapshots.set(session, signature)
  clearSession(session.id)
  const registered = registerSessionHooks(session.id, tools, (m) => merged.sourceByMatcher.get(m)!)
  for (const hook of registered.values()) hook.codexToolHook = true
}

export async function refreshCodexToolHooks(session: PtySession, signal?: AbortSignal) {
  if (session.bearerHash)
    await withSyncSnapshotLock(session.bearerHash, () => {
      signal?.throwIfAborted()
      registerCodexToolHooks(session)
    })
}

function hookProblem(event: ToolEvent, result: HookExecutionResult): string | undefined {
  if (result.exitCode === 2) return 'blocked by the hook'
  if (result.outcome !== 'success' || result.exitCode !== 0) return 'the hook failed (' + result.outcome + ')'
  if (typeof result.stdout !== 'string') return 'invalid hook output'
  const stdout = result.stdout.trim()
  let output: unknown = result.jsonOutput
  if (!output && stdout.startsWith('{')) {
    try {
      output = JSON.parse(stdout)
    } catch {
      return 'invalid hook output'
    }
  }
  if (!output) return
  const fields = record(output)
  const detail = record(fields?.hookSpecificOutput)
  if (!fields || (fields.hookSpecificOutput !== undefined && !detail)) return 'invalid hook output'
  if (fields.continue !== undefined && typeof fields.continue !== 'boolean') return 'invalid continuation decision'
  if (detail?.hookEventName !== undefined && detail.hookEventName !== event) return 'unexpected hook event'
  if (fields.continue === false || fields.decision === 'block' || detail?.permissionDecision === 'deny')
    return 'blocked by the hook'
  // Changing arguments after guards have seen them, or acknowledging an ask
  // without a user decision, would authorize a different operation.
  if (
    (fields.decision !== undefined && fields.decision !== 'approve') ||
    (detail?.permissionDecision !== undefined && detail.permissionDecision !== 'allow') ||
    detail?.updatedInput !== undefined ||
    detail?.updatedMCPToolOutput !== undefined
  )
    return 'the hook requested a decision or transformation unavailable in Codex'
}

export function hookToolError(message: string) {
  return { isError: true, content: [{ type: 'text', text: message }] }
}

export function codexToolCall(session: PtySession, tool: string, input: Record<string, unknown>) {
  if (session.engine !== 'codex') return
  const signature = snapshots.get(session)
  if (signature === null)
    return {
      mustFinishInline: true,
      before: async () => HOOK_SYNC_ERROR,
      after: async (_result: unknown) => hookToolError(HOOK_SYNC_ERROR),
    }
  const cliName = BRIDGE_TOOL_PREFIX + tool
  const codexName = (INTERACTIVE_TOOLS.has(tool) ? 'mcp__bridge_interactive__' : 'mcp__bridge__') + tool
  const hooks = listSession(session.id).flatMap((h) => {
    if (!events.some((event) => event === h.event)) return []
    if (matcherAccepts(matcherForCli(h.event, h.matcher), cliName)) return [{ h, toolName: cliName }]
    if (matcherAccepts(h.matcher, codexName)) return [{ h, toolName: codexName }]
    if (matcherAccepts(h.matcher, tool)) return [{ h, toolName: tool }]
    return []
  })
  if (!hooks.length) return
  const callId = randomUUID()
  const context = { cwd: session.cwd || session.host?.home, home: session.host?.home, bashHome: session.host?.home }
  async function run(event: ToolEvent, data: Record<string, unknown> = {}) {
    for (const { h, toolName } of hooks) {
      if (h.event !== event) continue
      const payload: HookInputPayload = {
        hook_event_name: event,
        session_id: session.cliConversationId || session.id,
        ...(session.cwd ? { cwd: session.cwd } : {}),
        tool_use_id: callId,
        tool_name: toolName,
        tool_input: input,
        ...data,
      }
      const applies = matchesCondition(h)
      if (applies === false) continue
      let reason: string | undefined
      if (session.launchConfig?.codexHookProtocol !== 1) reason = 'update the client to verify the local hook'
      else if (applies === undefined) reason = 'unsupported hook condition'
      else if (h.source.kind === 'project') reason = 'project commands require a reviewed plugin or user hook'
      else if (h.handler.type !== 'command' || h.effectiveHost !== 'local')
        reason = 'this hook requires a different execution host or engine'
      else {
        const result = await dispatchHook(session.id, h.id, payload, () => session.ws ?? null, session.tokenId, context)
        reason =
          snapshots.get(session) !== signature
            ? 'hook configuration changed during execution; retry the call'
            : hookProblem(event, result)
      }
      if (reason)
        return (
          event +
          ' (' +
          tool +
          '): ' +
          reason +
          (event === 'PreToolUse' ? '. Tool was not run.' : '. Tool already ran; its result was withheld.')
        )
    }
  }
  function matchesCondition(h: RegisteredHook): boolean | undefined {
    if (!h.handler.if) return true
    const rule = parseRule(h.handler.if)
    if (!rule) return undefined
    if (![tool, cliName, codexName].includes(rule.tool)) return false
    if (rule.content === undefined) return true
    if (!['Bash', 'PowerShell', 'Read', 'Write', 'Edit', 'NotebookEdit', 'Glob', 'Grep'].includes(tool))
      return undefined
    return conditionHolds(
      { tool, content: rule.content },
      { tool_name: cliName, tool_input: input, hook_event_name: h.event, session_id: session.id },
      context,
    )
  }
  const mustFinishInline = hooks.some(({ h }) => h.event !== 'PreToolUse' && matchesCondition(h) !== false)
  return {
    mustFinishInline,
    before: async () => {
      if (mustFinishInline && ['Bash', 'PowerShell'].includes(tool) && input.run_in_background === true)
        return 'A workspace hook checks the finished command. Run this command in the foreground.'
      return run('PreToolUse')
    },
    after: async (result: unknown) => {
      const failed = record(result)?.isError === true
      const problem = failed
        ? await run('PostToolUseFailure', { error: result })
        : await run('PostToolUse', { tool_response: result })
      return problem ? hookToolError(problem) : result
    },
  }
}

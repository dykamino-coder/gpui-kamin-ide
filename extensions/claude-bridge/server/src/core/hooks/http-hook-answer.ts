// Answer for a client command hook that the CLI calls as a native http hook.
//
// The CLI sends http hooks in-process, so a hook on every tool call costs no
// container process. Its http hooks do not read exit codes, though: they
// read a JSON body. This module turns the client's command result into the
// body that makes the CLI act as it would on that exit code. Only the events
// whose every outcome was checked against a real CLI use http
// (`HTTP_HOOK_EVENTS`); the rest stay on the command relay.

import type { HookExecutionResult, HookHandler } from './types'

/** Events whose command hooks the CLI calls as http hooks. The CLI refuses
 *  http hooks on SessionStart and Setup, and the other events either fire
 *  once per turn (no latency to win) or render a blocked answer differently
 *  from a command hook. */
export const HTTP_HOOK_EVENTS: ReadonlySet<string> = new Set(['PreToolUse', 'PostToolUse'])

/** Header the http form of a relayed hook carries, so the endpoint answers in
 *  the http-hook shape instead of the command-relay envelope. */
export const HTTP_HOOK_RELAY = 'http'

/** How the CLI names a command hook in the messages it writes for exit 2. */
export function commandLabel(handler: HookHandler): string {
  if (handler.type !== 'command') return ''
  return [handler.command, ...(handler.args ?? [])].join(' ')
}

export interface HttpHookAnswer {
  status: number
  body: string
}

/**
 * The http-hook answer that reproduces a command hook's result:
 *
 * - exit 0: stdout that starts with `{` is the hook's JSON output and goes out
 *   unchanged, so the CLI parses and validates it as it does command output.
 *   Other stdout is only shown in the transcript, so the answer is `{}`.
 * - exit 2: the event's blocking decision, with the reason the CLI would
 *   build from stderr: `[<command>]: <stderr>`, or `No stderr output`. The
 *   model then reads the same text as from the command hook; only a
 *   PostToolUse block names the hook by its URL in the CLI's own prefix.
 * - any other code, a timeout or a cancel: a non-2xx status, which the CLI
 *   reports as a non-blocking hook error, as it does for these exit codes.
 */
export function httpHookAnswer(event: string, result: HookExecutionResult, command: string): HttpHookAnswer {
  if (result.httpStatus !== undefined) return { status: result.httpStatus, body: result.stdout }
  const json = (value: unknown): HttpHookAnswer => ({ status: 200, body: JSON.stringify(value) })
  if (result.outcome !== 'timeout' && result.outcome !== 'cancelled') {
    if (result.exitCode === 0) {
      return result.stdout.trim().startsWith('{') ? { status: 200, body: result.stdout } : json({})
    }
    if (result.exitCode === 2) {
      const reason = `[${command}]: ${result.stderr.trim() === '' ? 'No stderr output' : result.stderr}`
      if (event === 'PreToolUse') {
        return json({
          hookSpecificOutput: { hookEventName: event, permissionDecision: 'deny', permissionDecisionReason: reason },
        })
      }
      if (event === 'PostToolUse') return json({ decision: 'block', reason })
    }
  }
  return { status: 500, body: '' }
}

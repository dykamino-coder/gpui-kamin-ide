// Bridge-owned status hooks — deterministic session state from the CLI's own
// lifecycle events instead of scraping OSC spinner glyphs off the PTY stream.
//
// We inject a no-op `command` hook (host:'local' so proxy-rewriter turns it
// into an http webhook — a 'server' host would run in-container and never call
// back) for each of the five events that map 1:1 onto session status. The
// FIRING of the webhook is the signal: routes.ts emits an authoritative
// `session:activity` and skips the no-op dispatch. This runs ALONGSIDE any
// user/project/plugin hooks for the same event — the CLI runs every matcher.
//
//   UserPromptSubmit → working (turn started)
//   Stop             → idle + prompt-ready (turn ended; carries last message)
//   SubagentStop     → a subagent finished (informational)
//   SessionStart     → prompt-ready (CLI is now accepting input — kills the
//                      cold-start "Enter into the void" window)
//   Notification     → waiting-for-user (CLI needs attention: permission,
//                      idle nudge, question)

import type { HookMatcher, HookSource, HookInputPayload } from './types'
import { sendToClient, setSessionPromptReady } from '../pty/session-io'
import type { PtySession } from '../types/pty'

export const BRIDGE_STATUS_EVENTS = [
  'UserPromptSubmit', 'Stop', 'SubagentStop', 'SessionStart', 'Notification',
  // SubagentStart fires the instant the CLI creates a subagent's transcript —
  // we use it to kick an immediate subagent-file scan instead of waiting up to
  // 2s for the periodic sweep. SessionEnd gives a deterministic idle at teardown.
  'SubagentStart', 'SessionEnd',
] as const

export function isBridgeStatusEvent(event: string): boolean {
  return (BRIDGE_STATUS_EVENTS as readonly string[]).includes(event)
}

/** The matchers to splice into the merged hook settings (with their source so
 *  rewriteHooksForCli tags the registered hook `bridge`). host:'local' forces
 *  the http-webhook rewrite even for SERVER_PREFERRED events. */
export function bridgeStatusHookMatchers(): Array<{ event: string; matcher: HookMatcher; source: HookSource }> {
  return BRIDGE_STATUS_EVENTS.map(event => ({
    event,
    matcher: { hooks: [{ type: 'command' as const, command: ':', host: 'local' as const }] },
    source: { kind: 'bridge' as const },
  }))
}

/** Map a fired status event → an authoritative `session:activity` push. Called
 *  for EVERY webhook of these events (idempotent — a user Stop hook firing too
 *  just re-asserts idle). `hookDriven:true` tells the client this beats the
 *  ❯-glyph heuristic. */
export function applyDeterministicStatus(
  session: PtySession, event: string, payload: HookInputPayload,
): void {
  // SubagentStart is ws-independent: the CLI just wrote a new subagents/*.jsonl,
  // so trigger an immediate discovery scan (the tile then appears at once rather
  // than after the periodic 2s sweep). Runs even if no client is attached.
  if (event === 'SubagentStart') {
    void session.jsonlWatcher?.scanSubagents?.()
    return
  }
  if (event === 'UserPromptSubmit' || event === 'SessionEnd') setSessionPromptReady(session, false)
  else if (event === 'Stop' || event === 'SessionStart') setSessionPromptReady(session, true)

  const ws = session.ws
  if (!ws) return
  const base = { type: 'session:activity', sessionId: session.id, hookDriven: true as const }
  switch (event) {
    case 'UserPromptSubmit':
      sendToClient(ws, { ...base, isWorking: true, promptReady: false, waiting: false })
      break
    case 'Stop': {
      const last = typeof payload.last_assistant_message === 'string' ? payload.last_assistant_message : undefined
      sendToClient(ws, { ...base, isWorking: false, promptReady: true, waiting: false, ...(last ? { lastMessage: last } : {}) })
      break
    }
    case 'SessionStart':
      sendToClient(ws, { ...base, isWorking: false, promptReady: true, waiting: false })
      break
    case 'Notification': {
      const message = typeof payload.message === 'string' ? payload.message : undefined
      sendToClient(ws, { ...base, waiting: true, ...(message ? { notificationMessage: message } : {}) })
      break
    }
    case 'SubagentStop':
      // Informational only — a subagent finished. Doesn't change the main tab's
      // working state, and the agent tiles already track subagents via JSONL, so
      // we don't emit a client message (nothing consumes it yet). The webhook
      // still fires + short-circuits; kept registered for future use.
      break
    case 'SessionEnd':
      // Deterministic teardown → idle (vs waiting on the 30-min reaper). Not
      // prompt-ready (the CLI is exiting), just no longer working.
      sendToClient(ws, { ...base, isWorking: false, waiting: false })
      break
  }
}

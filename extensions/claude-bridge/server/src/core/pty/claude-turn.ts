// Claude CLI turn verbs shared by the session WebSocket and the engine-neutral
// ClaudeAgentSession. Extracted verbatim from session-ws.ts; behaviour is the
// same for both callers.
import type { PtySession } from './types'
import { sessionCliControl } from './session-cli-control'
import { interruptSessionToolCalls } from './mcp-foreground-calls'
import { armSwitchConfirm, modelCommand } from './live-switch'
import { rememberModelSelection } from './model-selection'

export type WriteInput = (sessionId: string, data: string) => boolean

/**
 * User pressed Stop. Ctrl+C alone only ends the CLI's NEXT turn — the upstream
 * SSE it already requested keeps flushing until the CLI drops the socket, so
 * the streamed message "arrives to completion" first. Abort the in-flight
 * upstream request directly so the stream stops NOW, then SIGINT the CLI to
 * end the turn.
 */
export function interruptClaudeTurn(
  sessionId: string,
  getSession: (sessionId: string) => PtySession | undefined,
  writeInput: WriteInput,
): void {
  const session = getSession(sessionId)
  if (!(session && sessionCliControl(session)?.interrupt())) {
    try {
      getSession(sessionId)?.streamingProxy?.interrupt()
    } catch {
      /* no proxy this session */
    }
    writeInput(sessionId, '\x03')
  }
  // Ending the turn does not reach the client host: stop the tool calls the
  // CLI was waiting on, after the CLI was told (INC-2026-0096).
  if (session) interruptSessionToolCalls(session)
}

/**
 * Hot model switch: `/model` into a live CLI that already has a conversation.
 * Returns false when the caller must fall back to a restart (no conversation
 * yet, restarting, or the PTY did not take the write — INC-2026-0053).
 */
export function switchClaudeModelInPlace(
  session: PtySession,
  sessionId: string,
  model: string,
  writeInput: WriteInput,
): boolean {
  if (!session.cliConversationId || session.isRestarting) return false
  if (!writeInput(sessionId, modelCommand(model))) return false
  // The Bridge-owned PreModelSwitch `allow` hook (session-settings.ts) skips
  // the CLI's cache-warning dialog; the responder is a safety net.
  armSwitchConfirm(session, 'model', undefined)
  session.model = model
  // A selection made before the first answer is recorded nowhere in JSONL:
  // without this a respawn would bring the previous model back.
  rememberModelSelection(session.cliConversationId, model)
  return true
}

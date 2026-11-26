// The Claude model's view of its reasoning effort after a live `/effort`.
//
// Claude Code passes `--effort` to the API but never tells the model the level;
// Bridge's system prompt line (session-env.ts claudeEffortLine) does, and it is
// fixed for the life of the process. A live `/effort` changes the level in
// place without a respawn, so that line went stale and the model kept naming
// the level the chat started with. The next user message after a live change
// now carries one hidden line with the current level, in the same hidden block
// the webview and the browser host already strip from the chat.

import type { PtySession } from './types'
import { CONTEXT_TAG } from '../engine/handoff'

const changed = new WeakSet<PtySession>()

/** A live `/effort` was written to this session's CLI. */
export function noteEffortChanged(session: PtySession): void {
  changed.add(session)
}

/** The process was respawned: its new system prompt already names the level. */
export function forgetEffortChange(session: PtySession): void {
  changed.delete(session)
}

/** The hidden block for this user message, taken once; null when the model
 *  already knows the level. A slash command goes to the CLI, not to the
 *  model, so the note waits for the next real message. */
export function takeEffortNote(session: PtySession, userText: string): string | null {
  if (session.agentInput || !changed.has(session) || !session.effort) return null
  if (userText.trimStart().startsWith('/')) return null
  changed.delete(session)
  return [
    `<${CONTEXT_TAG}>`,
    `Reasoning effort is now \`${session.effort}\`: it was changed with \`/effort\` during this chat and replaces the level the system prompt names.`,
    `</${CONTEXT_TAG}>`,
  ].join('\n')
}

// Production wiring of ClaudeAgentSession to the live PTY session registry.
// Kept apart from claude-session.ts so the adapter loads without session-core.
import { eventBus } from '../events/bus'
import { destroySession, getSession, submitText, writeInput } from '../pty/session-core'
import { interruptClaudeTurn, switchClaudeModelInPlace } from '../pty/claude-turn'
import type { ClaudeSessionOps } from './claude-session'

export const claudeSessionOps: ClaudeSessionOps = {
  conversationId: (id) => getSession(id)?.cliConversationId ?? null,
  exists: (id) => getSession(id) !== undefined,
  submitText: (id, text, submissionId) => submitText(id, text, submissionId),
  interrupt: (id) => interruptClaudeTurn(id, getSession, writeInput),
  switchModel: (id, model) => {
    const session = getSession(id)
    return session ? switchClaudeModelInPlace(session, id, model, writeInput) : false
  },
  destroy: (id, reason) => destroySession(id, reason),
  onDestroyed: (id, listener) =>
    eventBus.on('session:destroyed', (event) => {
      const data = event.data as { sessionId?: string; exitCode?: number | null }
      if (data?.sessionId === id) listener(typeof data.exitCode === 'number' ? data.exitCode : null)
    }),
}

// `/effort <level>` typed as a Claude chat message changes the level in the
// running CLI exactly like the effort selector's live path (session-ws
// session:change-effort), but only the selector did Bridge's bookkeeping: the
// chat kept showing the old level, the model was never told the new one
// (effort-note.ts) and a respawn restored the old one (INC-2026-0099).

import type { PtySession } from './types'
import { armSwitchConfirm, typedEffort } from './live-switch'
import { noteEffortChanged } from './effort-note'

/** Record a typed `/effort <level>` of a Claude chat; `notify` tells the client. */
export function applyTypedEffort(
  session: PtySession,
  text: string,
  notify: (msg: Record<string, unknown>) => void,
): void {
  if (session.agentInput) return
  const effort = typedEffort(text)
  if (!effort) return
  // The CLI may ask to confirm the switch (cache warning), as for the selector.
  armSwitchConfirm(session, 'effort', effort)
  session.effort = effort
  noteEffortChanged(session)
  // Loaded on use: effort-selection pulls the data-dir stack, which the input
  // path (session-io) must not load for every importer.
  const conversationId = session.cliConversationId
  if (conversationId)
    void import('./effort-selection')
      .then((m) => m.rememberEffortSelection(conversationId, effort))
      .catch(() => undefined)
  notify({ type: 'session:model-changed', sessionId: session.id, model: session.model, effort, inPlace: true })
}

import { signal, computed } from '@preact/signals'
import type { SavedSession } from '../../shared/types'

// Saved sessions store. Sourced from `bridge.getSavedSessions()` and
// kept in sync via `onSavedSessionsChanged`. Promoted to a signal so
// non-sidebar components (TabsBar in particular) can fall back to the
// human-readable session label when a pinned ghost tab carries no
// title of its own.
export const savedSessionsSignal = signal<SavedSession[]>([])

/** Index by conversationId for cheap O(1) lookup of pinned ghost tabs. */
export const savedSessionsByConvId = computed<Map<string, SavedSession>>(() => {
  const m = new Map<string, SavedSession>()
  for (const s of savedSessionsSignal.value) {
    if (s.conversationId) m.set(s.conversationId, s)
  }
  return m
})

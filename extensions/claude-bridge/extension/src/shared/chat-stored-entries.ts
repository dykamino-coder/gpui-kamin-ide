/** Which transcript rows the chat keeps. Shared by the host replay tail and
 *  the webview store so that they agree on what a "row" of chat history is.
 *
 *  `user` / `assistant` / `system` / `attachment` are what the chat renders.
 *  `mode` / `permission-mode` feed the permissions dropdown and
 *  `queue-operation` reconstructs the pending queue. They render nothing, but
 *  they are read.
 *
 *  Everything else (`cost-state`, `atis-latch`, `tools-used`, `api-request*`,
 *  titles, snapshots, …) is bookkeeping that the chat never reads. An allowlist,
 *  because a denylist only knows the types that existed when it was written:
 *  newer rows got through, took window slots and replay-tail slots, and a
 *  bookkeeping-heavy transcript tail left the chat with nothing to show. */
import { newestInFileOrder } from './jsonl-cache-order'

export const CHAT_STORED_ENTRY_TYPES: ReadonlySet<string> = new Set([
  'user', 'assistant', 'system', 'attachment',
  'mode', 'permission-mode', 'queue-operation',
])

export function isChatStoredEntry(entry: { type?: unknown } | null | undefined): boolean {
  return typeof entry?.type === 'string' && CHAT_STORED_ENTRY_TYPES.has(entry.type)
}

/** The newest `max` rows the chat keeps, in file order. Rows the chat drops
 *  do not count against `max`: otherwise a long run of bookkeeping rows at the
 *  end of a transcript pushes the whole conversation out of a reload replay. */
export function chatReplayTail<T extends { type?: unknown; _pos?: number; _ord?: number }>(
  entries: readonly T[], max: number,
): T[] {
  const kept = entries.filter(isChatStoredEntry)
  if (kept.length <= max) return kept
  // File order with untagged live rows AFTER the tagged ones (jsonl-cache-order):
  // `_pos ?? _ord ?? 0` ranked them oldest and cut the session's middle out (D2).
  return newestInFileOrder(kept, max)
}

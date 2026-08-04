// Per-session live dashboard counters derived from the JSONL stream. The
// watcher re-reads the whole file on every replay (start + client reattach), so
// these are RESET at replay start and then accumulated over every parsed entry
// (replay + tail). That keeps them accurate across reattaches without the
// double-counting a naive "increment forever" would cause. Independent of the
// inputCount / insertRequest path in session-stats-recorder.

import type { PtySession } from './types'
import type { JsonlEntry } from '../../shared/jsonl-types'

interface Usage {
  input_tokens?: number
  output_tokens?: number
  cache_read_input_tokens?: number
  cache_creation_input_tokens?: number
}

// Internal per-session bookkeeping kept off the typed PtySession: the id of the
// last assistant message we counted. CLI v2.1.x splits one model turn across
// several JSONL lines (thinking → text → tool_use) that share a `message.id`;
// without this we'd count each split as its own message and sum its usage
// multiple times. WeakMap so a destroyed session's entry is GC'd automatically.
const lastAsstMsgId = new WeakMap<PtySession, string>()

/** Reset the live counters — called when the watcher begins a full replay. */
export function resetJsonlStats(session: PtySession): void {
  session.userMessages = 0
  session.assistantMessages = 0
  session.contextTokens = 0
  session.totalTokens = 0
  lastAsstMsgId.delete(session)
}

/** Fold one parsed JSONL entry into the session's live counters. Fired for
 *  every entry the watcher parses (both the initial replay and live tail). */
export function accumulateJsonlStats(session: PtySession, entry: JsonlEntry): void {
  if (entry.type === 'user') {
    // Tool-result rows are also `type:"user"` but aren't real user messages —
    // match session-stats-recorder's filter so the count means "prompts sent".
    const content = entry.message?.content
    if (Array.isArray(content) && content.some((b) => b.type === 'tool_result')) return
    session.userMessages++
    return
  }
  if (entry.type !== 'assistant') return

  const msg = entry.message as { id?: string; usage?: Usage } | undefined
  const u = msg?.usage
  if (u) {
    // Context window at this (latest) turn = everything fed as input. Updated on
    // every usage-bearing line so a trailing usage-less split can't zero it.
    session.contextTokens = (u.input_tokens ?? 0) + (u.cache_read_input_tokens ?? 0) + (u.cache_creation_input_tokens ?? 0)
  }

  // Count + total ONCE per message.id — skip continuation lines of the same
  // turn. If a line has no `id` we can't dedup, so we count it: that only
  // happens on pre-v2.1.x CLI formats, which also don't split turns across
  // lines (one line = one message there), so counting each is correct.
  const id = msg?.id
  if (id && lastAsstMsgId.get(session) === id) return
  if (id) lastAsstMsgId.set(session, id)
  session.assistantMessages++
  if (u) {
    session.totalTokens += (u.input_tokens ?? 0) + (u.output_tokens ?? 0)
      + (u.cache_read_input_tokens ?? 0) + (u.cache_creation_input_tokens ?? 0)
  }
}

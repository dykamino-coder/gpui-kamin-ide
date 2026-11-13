// User / Asst / Context / Total of the dashboard's Active PTY Sessions table
// and the Sessions card. One source for both the WS init snapshot and the
// polled REST list: the session's live counters (session-jsonl-stats.ts),
// folded from its transcript on every replay and tail, so they are current to
// the last line instead of to the last 5-minute sweep.
//
// While the transcript is being indexed, Context can use the conversation's
// persisted snapshot. session_tokens has no resume-parent column: compact links
// are stored separately, so a recursive SQL query cannot follow them here.

import { getDb } from '../stats/database/lifecycle'
import { warnLog } from '../logging'
import type { PtySession } from '../pty/types'

export interface PtySessionCounters {
  userMessages: number
  assistantMessages: number
  contextTokens: number
  totalTokens: number
}

type CounterSource = Pick<
  PtySession,
  'userMessages' | 'assistantMessages' | 'contextTokens' | 'totalTokens' | 'cliConversationId'
>

/** Persisted context of each conversation whose live context is 0. */
export async function persistedContextTokens(sessions: readonly CounterSource[]): Promise<Map<string, number>> {
  const ids = [
    ...new Set(sessions.filter((s) => !(s.contextTokens > 0) && s.cliConversationId).map((s) => s.cliConversationId!)),
  ]
  const out = new Map<string, number>()
  if (ids.length === 0) return out
  try {
    const db = await getDb()
    const rows = (
      await db.runAndReadAll(
        `SELECT session_id AS root, context_tokens AS context
         FROM session_tokens
         WHERE session_id IN (${ids.map(() => '?').join(', ')})
           AND COALESCE(context_tokens, 0) > 0`,
        ids,
      )
    ).getRowObjects() as Array<{ root: string; context: bigint | number | null }>
    for (const r of rows) out.set(r.root, Number(r.context ?? 0))
  } catch (err) {
    // The table still shows the live counters; only the fallback is missing.
    warnLog('pty sessions: persisted context query failed', { sessions: ids.length, error: String(err) })
  }
  return out
}

export function ptySessionCounters(s: CounterSource, persisted: ReadonlyMap<string, number>): PtySessionCounters {
  const fallback = s.cliConversationId ? (persisted.get(s.cliConversationId) ?? 0) : 0
  return {
    userMessages: s.userMessages ?? 0,
    assistantMessages: s.assistantMessages ?? 0,
    contextTokens: s.contextTokens > 0 ? s.contextTokens : fallback,
    totalTokens: s.totalTokens ?? 0,
  }
}

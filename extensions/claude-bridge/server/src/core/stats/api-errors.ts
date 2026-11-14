import { warnLog } from '../logging'
// Provider errors of an agent turn → `api_errors` (dashboard Errors card).
// Claude CLI errors are parsed from its terminal output (session-io.ts); a
// codex turn reports them as events, recorded here with engine 'codex'.

import type { AgentEngineId } from '../../shared/agent-events'
import { getDb } from './database/lifecycle'
import { withStatsWrite } from './database/write-lock'

export interface ApiErrorRow {
  sessionId: string | null
  userName: string | null
  engine: AgentEngineId
  errorType: string
  raw: string
}

/** Short error class for the chart: `429`, `timeout`, … or the first words. */
export function codexErrorType(message: string): string {
  const status = /\b(4\d\d|5\d\d)\b/.exec(message)?.[1]
  if (status) return status
  if (/timed? ?out|timeout/i.test(message)) return 'timeout'
  if (/rate.?limit|usage limit/i.test(message)) return 'rate_limit'
  if (/ECONNREFUSED|connection refused/i.test(message)) return 'ConnectionRefused'
  return (
    message
      .split(/[\s:.]/)
      .filter(Boolean)
      .slice(0, 3)
      .join(' ')
      .slice(0, 40) || 'error'
  )
}

/** Best effort: an error report must never break the session that hit it. */
export async function recordApiError(row: ApiErrorRow): Promise<void> {
  try {
    const db = await getDb()
    await withStatsWrite(() =>
      db.run(
        `INSERT INTO api_errors (timestamp, session_id, user_name, error_type, url, attempt, max_attempts, retry_ms, raw, engine)
         VALUES (?, ?, ?, ?, NULL, 0, 0, 0, ?, ?)`,
        [new Date().toISOString(), row.sessionId, row.userName, row.errorType, row.raw.slice(0, 2000), row.engine],
      ),
    )
  } catch (error) {
    warnLog('stats: API error insert failed', { error: String(error) })
  }
}

// Deterministic synthetic stats corpus for rollup tests and the size bench.
//
// Shape follows the production admin page: a few dozen users with skewed
// activity, sessions spread over months, split assistant rows sharing one
// message.id, tool_use/tool_result rows, subagent sessions, several models,
// proxy-only side-calls and soft-deleted sessions. Every value is derived
// from hash(seed, n, k), so the same parameters always produce the same rows
// (needed to compare rollup numbers with a raw-event recomputation).

import type { DuckDBConnection } from '@duckdb/node-api'
import { initSessionOwners } from './session-owners'

export interface SyntheticStatsOptions {
  users: number
  sessions: number
  /** Mean JSONL rows per session (actual count varies 0.2x..1.8x). */
  eventsPerSession: number
  /** History depth in days, ending at `endIso`. */
  days: number
  endIso: string
  seed?: number
  /** Sessions per INSERT statement (bounds generator memory). */
  chunkSessions?: number
  onProgress?: (doneSessions: number) => void
}

export const SYNTHETIC_MODELS = [
  'claude-opus-5-5',
  'claude-opus-5',
  'claude-sonnet-5-5',
  'claude-sonnet-5',
  'claude-haiku-5-5',
  'claude-haiku-4-5',
  'claude-fable-5-2',
  'claude-fable-5-1',
  'claude-opus-4-8',
  'gpt-5.5-codex',
]

export async function generateSyntheticStats(db: DuckDBConnection, o: SyntheticStatsOptions): Promise<void> {
  const seed = o.seed ?? 7
  const chunk = o.chunkSessions ?? 2000
  const models = `[${SYNTHETIC_MODELS.map((m) => `'${m}'`).join(',')}]`
  await db.run(`CREATE OR REPLACE TEMP MACRO synth_r(a, b) AS (hash(${seed}, a, b) % 1000003) / 1000003.0`)
  // Sessions: user skewed toward low indexes (a few heavy users, a long tail),
  // start day biased toward recent history, 20% subagent children.
  await db.run(`CREATE OR REPLACE TEMP TABLE synth_sessions AS
    SELECT n,
      md5('s' || n) AS session_id,
      'user' || lpad(CAST(floor(${o.users} * pow(synth_r(n, 'u'), 1.7)) AS INT)::TEXT, 2, '0') AS user_name,
      CAST(${o.endIso.length ? `'${o.endIso}'` : 'now()'} AS TIMESTAMP)
        - to_days(CAST(floor(${o.days} * pow(synth_r(n, 'd'), 0.8)) AS INT))
        - to_hours(CAST(floor(24 * synth_r(n, 'h')) AS INT)) AS start_ts,
      greatest(3, CAST(${o.eventsPerSession} * (0.2 + 1.6 * synth_r(n, 'e')) AS INT)) AS nev,
      ${models}[1 + CAST(floor(${SYNTHETIC_MODELS.length} * pow(synth_r(n, 'm'), 1.5)) AS INT)] AS model,
      synth_r(n, 'sa') < 0.2 AS is_sub,
      synth_r(n, 'del') < 0.03 AS deleted
    FROM range(${o.sessions}) t(n)`)
  await db.run(`INSERT INTO session_tokens (session_id, token_id, user_name, created_at, deleted)
    SELECT session_id, 'tok-' || user_name, user_name, strftime(start_ts, '%Y-%m-%dT%H:%M:%S.000Z'),
      CASE WHEN deleted THEN 1 ELSE 0 END
    FROM synth_sessions WHERE NOT is_sub`)
  // Rows per session follow a 9-step turn: user, split assistant pair (one
  // message.id), tool_use, tool_result, assistant, tool_use, tool_result,
  // assistant. Context grows along the session like a real transcript.
  for (let lo = 0; lo < o.sessions; lo += chunk) {
    const hi = Math.min(o.sessions, lo + chunk)
    await db.run(`INSERT INTO jsonl_events
      (uuid, session_id, timestamp, type, model, input_tokens, output_tokens, cache_read_tokens,
       cache_write_tokens, tool_name, message_id, token_id, is_subagent, parent_session_id, user_name,
       cache_write_1h_tokens)
      SELECT md5(s.session_id || '-' || k) AS uuid, s.session_id,
        strftime(s.start_ts + to_milliseconds(CAST(k * 45000 + 30000 * synth_r(s.n, k) AS BIGINT)), '%Y-%m-%dT%H:%M:%S.%gZ'),
        CASE k % 9 WHEN 0 THEN 'user' WHEN 3 THEN 'tool_use' WHEN 6 THEN 'tool_use'
          WHEN 4 THEN 'tool_result' WHEN 7 THEN 'tool_result' ELSE 'assistant' END AS type,
        CASE WHEN k % 9 IN (4, 7) THEN NULL WHEN synth_r(s.n, k + 7) < 0.002 THEN '<synthetic>'
          WHEN synth_r(s.n, k + 3) < 0.08 THEN ${models}[1 + CAST(floor(${SYNTHETIC_MODELS.length} * synth_r(s.n, k + 5)) AS INT)]
          ELSE s.model END AS model,
        CASE WHEN k % 9 IN (1, 2, 5, 8) THEN CAST(1 + 40 * synth_r(s.n, k + 11) AS INT) ELSE 0 END,
        CASE WHEN k % 9 = 1 THEN 0 WHEN k % 9 IN (2, 5, 8) THEN CAST(20 + 3000 * pow(synth_r(s.n, k + 13), 2) AS INT) ELSE 0 END,
        CASE WHEN k % 9 IN (1, 2, 5, 8) THEN CAST(least(180000, 9000 + k * 400 * synth_r(s.n, 'c')) AS INT) ELSE 0 END,
        CASE WHEN k % 9 IN (1, 2, 5, 8) THEN CAST(2000 * synth_r(s.n, k + 17) AS INT) ELSE 0 END,
        CASE WHEN k % 9 = 3 THEN 'Bash' WHEN k % 9 = 6 THEN 'Read' END,
        CASE WHEN k % 9 IN (1, 2, 3) THEN 'msg_' || s.n || '_' || (k // 9) || 'a'
          WHEN k % 9 IN (5, 6) THEN 'msg_' || s.n || '_' || (k // 9) || 'b'
          WHEN k % 9 = 8 THEN 'msg_' || s.n || '_' || (k // 9) || 'c' END,
        'tok-' || s.user_name,
        CASE WHEN s.is_sub THEN 1 ELSE 0 END,
        CASE WHEN s.is_sub THEN md5('s' || greatest(0, s.n - 1)) END,
        s.user_name,
        CASE WHEN k % 9 IN (1, 2, 5, 8) AND synth_r(s.n, 'ttl') < 0.3 THEN CAST(1000 * synth_r(s.n, k + 19) AS INT) ELSE 0 END
      FROM synth_sessions s, range(s.nev) r(k)
      WHERE s.n >= ${lo} AND s.n < ${hi}`)
    o.onProgress?.(hi)
  }
  // Proxy-only side-calls (no JSONL twin) on ~5% of sessions.
  await db.run(`INSERT INTO proxy_usage (message_id, session_id, timestamp, model, input_tokens, output_tokens,
      cache_read_tokens, cache_write_tokens, cache_write_1h_tokens, token_id, user_name, is_subagent)
    SELECT 'side_' || n, session_id, strftime(start_ts, '%Y-%m-%dT%H:%M:%S.000Z'), 'claude-haiku-5-5',
      200, 50, 0, 1500, 0, 'tok-' || user_name, user_name, 1
    FROM synth_sessions WHERE synth_r(n, 'px') < 0.05
    ON CONFLICT DO NOTHING`)
  await db.run('DROP TABLE synth_sessions')
  // Bulk fixtures exercise the same one-time migration as an existing database.
  await db.run("DELETE FROM settings WHERE key = 'session_owners_v1'")
  await initSessionOwners(db)
}

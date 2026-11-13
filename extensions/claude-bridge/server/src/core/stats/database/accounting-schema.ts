// Учёт ответов API отдельно от сырого зеркала JSONL: split-строки одного
// message.id оплачиваются один раз; proxy-only side-calls сохраняются до JSONL.
import type { DuckDBConnection } from '@duckdb/node-api'

/** A short session list resolved through the indexes; `proxyMessageIds` are the
 *  message ids of those sessions' proxy_usage rows (read beforehand). */
export interface IndexedFilter {
  proxyMessageIds: string[]
}

/** SELECT behind `jsonl_accounting_events`. `filter` is an optional predicate
 *  over a bare `session_id` column, injected into every base-table branch so a
 *  per-session recompute (stats rollup) never evaluates the dedupe window over
 *  the whole table. With no filter this is exactly the view definition.
 *  `indexed`: a session list served by the ART indexes (the drain); DuckDB uses
 *  an index only for a bare predicate, not one combined with the branches' `type`
 *  filters, which would scan the whole table. */
export function accountingEventsSelect(filter = 'TRUE', indexed?: IndexedFilter): string {
  const f = indexed ? 'TRUE' : `(${filter})`
  const ev = indexed ? 'ev' : 'jsonl_events'
  // A correlated NOT EXISTS over jsonl_events becomes a full anti-join scan; for a
  // short list the JSONL twins of the proxy rows come from the message_id index.
  const q = (s: string) => `'${s.replace(/'/g, "''")}'`
  const twins = !indexed ? 'jsonl_events' : 'tw'
  // Сырой журнал не меняется: повторы нужны для восстановления дерева чата.
  // MAX сохраняет финальные счётчики при неполных split/stream записях.
  return `
    WITH ${
      indexed
        ? // Materialized, so the branches' type filters are not merged into these
          // bare predicates (DuckDB serves only a bare indexed predicate by the index).
          `ev AS MATERIALIZED (SELECT * FROM jsonl_events WHERE ${filter}),
          tw AS MATERIALIZED (SELECT * FROM jsonl_events WHERE ${
            indexed.proxyMessageIds.length ? `message_id IN (${indexed.proxyMessageIds.map(q).join(', ')})` : 'FALSE'
          }),`
        : ''
    }
    assistant_usage AS (
      SELECT session_id, COALESCE(NULLIF(message_id, ''), uuid) AS usage_id,
        MAX(input_tokens) AS input_tokens, MAX(output_tokens) AS output_tokens,
        MAX(cache_read_tokens) AS cache_read_tokens, MAX(cache_write_tokens) AS cache_write_tokens,
        MAX(cache_write_1h_tokens) AS cache_write_1h_tokens
      FROM ${ev} WHERE type = 'assistant' AND ${f} GROUP BY session_id, usage_id
    ), canonical AS (
      SELECT * FROM ${ev} WHERE type = 'assistant' AND ${f}
      QUALIFY ROW_NUMBER() OVER (
        PARTITION BY session_id, COALESCE(NULLIF(message_id, ''), uuid)
        ORDER BY timestamp DESC, uuid DESC) = 1
    )
    SELECT uuid, session_id, timestamp, type, model, input_tokens, output_tokens,
      cache_read_tokens, cache_write_tokens, tool_name, message_id, token_id,
      is_subagent, parent_session_id, user_name, cache_write_1h_tokens
    FROM ${ev} WHERE type != 'assistant' AND ${f}
    UNION ALL
    SELECT c.uuid, c.session_id, c.timestamp, c.type, c.model,
      GREATEST(u.input_tokens, COALESCE(p.input_tokens, 0)),
      GREATEST(u.output_tokens, COALESCE(p.output_tokens, 0)),
      GREATEST(u.cache_read_tokens, COALESCE(p.cache_read_tokens, 0)),
      GREATEST(u.cache_write_tokens, COALESCE(p.cache_write_tokens, 0)), c.tool_name, c.message_id, c.token_id,
      c.is_subagent, c.parent_session_id, c.user_name,
      GREATEST(u.cache_write_1h_tokens, COALESCE(p.cache_write_1h_tokens, 0))
    FROM canonical c JOIN assistant_usage u ON u.session_id = c.session_id
      AND u.usage_id = COALESCE(NULLIF(c.message_id, ''), c.uuid)
    LEFT JOIN proxy_usage p ON p.message_id = c.message_id AND p.token_id = c.token_id
    UNION ALL
    SELECT 'proxy:' || p.message_id, p.session_id, p.timestamp, 'assistant', p.model,
      p.input_tokens, p.output_tokens, p.cache_read_tokens, p.cache_write_tokens,
      NULL, p.message_id, p.token_id, p.is_subagent, NULL, p.user_name, p.cache_write_1h_tokens
    FROM proxy_usage p WHERE (${filter.replace(/\bsession_id\b/g, 'p.session_id')}) AND NOT EXISTS (
      SELECT 1 FROM ${twins} e WHERE e.type = 'assistant'
        AND e.message_id = p.message_id AND e.token_id = p.token_id
    )`
}

export async function initAccountingSchema(db: DuckDBConnection): Promise<void> {
  await db.run(`CREATE TABLE IF NOT EXISTS proxy_usage (
    message_id TEXT PRIMARY KEY, session_id TEXT NOT NULL, timestamp TEXT NOT NULL,
    model TEXT, input_tokens INTEGER DEFAULT 0, output_tokens INTEGER DEFAULT 0,
    cache_read_tokens INTEGER DEFAULT 0, cache_write_tokens INTEGER DEFAULT 0,
    cache_write_1h_tokens INTEGER DEFAULT 0, token_id TEXT, user_name TEXT,
    is_subagent INTEGER DEFAULT 0
  )`)
  await db.run(`CREATE INDEX IF NOT EXISTS idx_proxy_usage_session ON proxy_usage(session_id)`)
  await db.run(`CREATE OR REPLACE VIEW jsonl_accounting_events AS ${accountingEventsSelect()}`)
}

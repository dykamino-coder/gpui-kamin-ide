// Учёт ответов API отдельно от сырого зеркала JSONL: split-строки одного
// message.id оплачиваются один раз; proxy-only side-calls сохраняются до JSONL.
import type { DuckDBConnection } from '@duckdb/node-api'

export async function initAccountingSchema(db: DuckDBConnection): Promise<void> {
  await db.run(`CREATE TABLE IF NOT EXISTS proxy_usage (
    message_id TEXT PRIMARY KEY, session_id TEXT NOT NULL, timestamp TEXT NOT NULL,
    model TEXT, input_tokens INTEGER DEFAULT 0, output_tokens INTEGER DEFAULT 0,
    cache_read_tokens INTEGER DEFAULT 0, cache_write_tokens INTEGER DEFAULT 0,
    cache_write_1h_tokens INTEGER DEFAULT 0, token_id TEXT, user_name TEXT,
    is_subagent INTEGER DEFAULT 0
  )`)
  await db.run(`CREATE INDEX IF NOT EXISTS idx_proxy_usage_session ON proxy_usage(session_id)`)
  // Сырой журнал не меняется: повторы нужны для восстановления дерева чата.
  // MAX сохраняет финальные счётчики при неполных split/stream записях.
  await db.run(`CREATE OR REPLACE VIEW jsonl_accounting_events AS
    WITH assistant_usage AS (
      SELECT session_id, COALESCE(NULLIF(message_id, ''), uuid) AS usage_id,
        MAX(input_tokens) AS input_tokens, MAX(output_tokens) AS output_tokens,
        MAX(cache_read_tokens) AS cache_read_tokens, MAX(cache_write_tokens) AS cache_write_tokens,
        MAX(cache_write_1h_tokens) AS cache_write_1h_tokens
      FROM jsonl_events WHERE type = 'assistant' GROUP BY session_id, usage_id
    ), canonical AS (
      SELECT * FROM jsonl_events WHERE type = 'assistant'
      QUALIFY ROW_NUMBER() OVER (
        PARTITION BY session_id, COALESCE(NULLIF(message_id, ''), uuid)
        ORDER BY timestamp DESC, uuid DESC) = 1
    )
    SELECT uuid, session_id, timestamp, type, model, input_tokens, output_tokens,
      cache_read_tokens, cache_write_tokens, tool_name, message_id, token_id,
      is_subagent, parent_session_id, user_name, cache_write_1h_tokens
    FROM jsonl_events WHERE type != 'assistant'
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
    FROM proxy_usage p WHERE NOT EXISTS (
      SELECT 1 FROM jsonl_events e WHERE e.type = 'assistant'
        AND e.message_id = p.message_id AND e.token_id = p.token_id
    )`)
}

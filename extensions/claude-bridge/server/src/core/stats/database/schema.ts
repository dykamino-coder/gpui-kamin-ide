// ============================================================================
// Database Schema — DuckDB tables + migrations
// ============================================================================
//
// DuckDB-flavored DDL. Differences from the previous SQLite version:
//   - SEQUENCE for autoincrement (DuckDB doesn't support AUTOINCREMENT)
//   - VACUUM replaced with CHECKPOINT (DuckDB native)
//   - INSERT OR IGNORE / OR REPLACE → ON CONFLICT DO NOTHING / DO UPDATE
//     (handled at callsite, not schema)
// Idempotent on every boot via CREATE TABLE IF NOT EXISTS / try-catch
// ALTER for additive migrations.

import type { DuckDBConnection } from '@duckdb/node-api'

async function tryExec(db: DuckDBConnection, sql: string): Promise<void> {
  try { await db.run(sql) } catch { /* idempotent migration — swallow */ }
}

export async function initSchema(db: DuckDBConnection): Promise<void> {
  await db.run(`
    CREATE TABLE IF NOT EXISTS requests (
      id TEXT PRIMARY KEY,
      timestamp TEXT NOT NULL,
      endpoint TEXT NOT NULL,
      method TEXT DEFAULT 'POST',
      model TEXT,
      plugin_id TEXT NOT NULL,
      user_name TEXT,
      duration_ms INTEGER DEFAULT 0,
      input_tokens INTEGER DEFAULT 0,
      output_tokens INTEGER DEFAULT 0,
      tools_used TEXT,
      status TEXT DEFAULT 'streaming',
      status_code INTEGER DEFAULT 0,
      error TEXT,
      request_body TEXT,
      response_text TEXT
    )
  `)

  await db.run(`
    CREATE TABLE IF NOT EXISTS settings (
      key TEXT PRIMARY KEY,
      value TEXT NOT NULL
    )
  `)

  await db.run(`CREATE INDEX IF NOT EXISTS idx_requests_timestamp ON requests(timestamp)`)
  await db.run(`CREATE INDEX IF NOT EXISTS idx_requests_plugin_id ON requests(plugin_id)`)
  await db.run(`CREATE INDEX IF NOT EXISTS idx_requests_user_name ON requests(user_name)`)
  await db.run(`CREATE INDEX IF NOT EXISTS idx_requests_endpoint ON requests(endpoint)`)
  await db.run(`CREATE INDEX IF NOT EXISTS idx_requests_status ON requests(status)`)

  await tryExec(db, 'ALTER TABLE requests ADD COLUMN is_user_request INTEGER DEFAULT 0')
  await tryExec(db, 'ALTER TABLE requests ADD COLUMN user_message TEXT')
  await tryExec(db, 'CREATE INDEX IF NOT EXISTS idx_requests_is_user ON requests(is_user_request)')

  await tryExec(db, 'ALTER TABLE requests ADD COLUMN session_key TEXT')
  await tryExec(db, 'CREATE INDEX IF NOT EXISTS idx_requests_session_key ON requests(session_key)')

  await tryExec(db, 'ALTER TABLE requests ADD COLUMN cache_read_tokens INTEGER DEFAULT 0')
  await tryExec(db, 'ALTER TABLE requests ADD COLUMN cache_write_tokens INTEGER DEFAULT 0')

  await tryExec(db, 'ALTER TABLE requests ADD COLUMN client_task_id TEXT')
  await tryExec(db, 'ALTER TABLE requests ADD COLUMN raw_headers TEXT')

  await db.run(`
    CREATE TABLE IF NOT EXISTS api_tokens (
      id TEXT PRIMARY KEY,
      name TEXT NOT NULL,
      plugin_id TEXT NOT NULL,
      token TEXT NOT NULL UNIQUE,
      created_at TEXT NOT NULL
    )
  `)
  await db.run(`CREATE INDEX IF NOT EXISTS idx_api_tokens_token ON api_tokens(token)`)
  await tryExec(db, 'ALTER TABLE api_tokens ADD COLUMN cache INTEGER DEFAULT 1')

  await db.run(`
    CREATE TABLE IF NOT EXISTS dashboard_sessions (
      id TEXT PRIMARY KEY,
      token TEXT NOT NULL UNIQUE,
      created_at TEXT NOT NULL,
      expires_at TEXT NOT NULL
    )
  `)
  await db.run(`CREATE INDEX IF NOT EXISTS idx_dashboard_sessions_token ON dashboard_sessions(token)`)
  await db.run(`CREATE INDEX IF NOT EXISTS idx_dashboard_sessions_expires ON dashboard_sessions(expires_at)`)

  // ── JSONL events — durable mirror of every meaningful entry from
  //    Claude CLI's per-session JSONL files.
  await db.run(`
    CREATE TABLE IF NOT EXISTS jsonl_events (
      uuid TEXT PRIMARY KEY,
      session_id TEXT NOT NULL,
      timestamp TEXT NOT NULL,
      type TEXT NOT NULL,
      model TEXT,
      input_tokens INTEGER DEFAULT 0,
      output_tokens INTEGER DEFAULT 0,
      cache_read_tokens INTEGER DEFAULT 0,
      cache_write_tokens INTEGER DEFAULT 0,
      tool_name TEXT,
      message_id TEXT
    )
  `)
  await db.run(`CREATE INDEX IF NOT EXISTS idx_jsonl_events_session ON jsonl_events(session_id)`)
  await db.run(`CREATE INDEX IF NOT EXISTS idx_jsonl_events_timestamp ON jsonl_events(timestamp)`)
  await db.run(`CREATE INDEX IF NOT EXISTS idx_jsonl_events_type ON jsonl_events(type)`)
  await db.run(`CREATE INDEX IF NOT EXISTS idx_jsonl_events_model ON jsonl_events(model)`)
  await db.run(`CREATE INDEX IF NOT EXISTS idx_jsonl_events_message_id ON jsonl_events(message_id)`)

  // Token attribution + subagent flag (added later as migrations).
  await tryExec(db, 'ALTER TABLE jsonl_events ADD COLUMN token_id TEXT')
  await tryExec(db, 'ALTER TABLE jsonl_events ADD COLUMN is_subagent INTEGER DEFAULT 0')
  await tryExec(db, 'ALTER TABLE jsonl_events ADD COLUMN parent_session_id TEXT')
  await tryExec(db, 'ALTER TABLE jsonl_events ADD COLUMN user_name TEXT')
  await tryExec(db, 'CREATE INDEX IF NOT EXISTS idx_jsonl_events_token_id ON jsonl_events(token_id)')
  await tryExec(db, 'CREATE INDEX IF NOT EXISTS idx_jsonl_events_user_name ON jsonl_events(user_name)')
  await tryExec(db, 'CREATE INDEX IF NOT EXISTS idx_jsonl_events_is_subagent ON jsonl_events(is_subagent)')

  // Drop legacy day/hour columns if migrated from SQLite (clients group
  // by local TZ from `timestamp`).
  await tryExec(db, 'DROP INDEX IF EXISTS idx_jsonl_events_day')
  await tryExec(db, 'ALTER TABLE jsonl_events DROP COLUMN day')
  await tryExec(db, 'ALTER TABLE jsonl_events DROP COLUMN hour')

  // ── session_tokens: bridge-managed conversation index, soft-delete,
  //    persisted per-session metadata (title, last_context, compacts...)
  await db.run(`
    CREATE TABLE IF NOT EXISTS session_tokens (
      session_id TEXT PRIMARY KEY,
      token_id TEXT NOT NULL,
      user_name TEXT,
      created_at TEXT NOT NULL
    )
  `)
  await db.run(`CREATE INDEX IF NOT EXISTS idx_session_tokens_token ON session_tokens(token_id)`)
  await tryExec(db, 'ALTER TABLE session_tokens ADD COLUMN deleted INTEGER DEFAULT 0')
  await tryExec(db, 'CREATE INDEX IF NOT EXISTS idx_session_tokens_deleted ON session_tokens(deleted)')
  await tryExec(db, 'ALTER TABLE session_tokens ADD COLUMN cwd TEXT')
  await tryExec(db, 'ALTER TABLE session_tokens ADD COLUMN title TEXT')
  await tryExec(db, 'ALTER TABLE session_tokens ADD COLUMN last_context INTEGER DEFAULT 0')
  await tryExec(db, 'ALTER TABLE session_tokens ADD COLUMN compact_boundaries INTEGER DEFAULT 0')
  await tryExec(db, 'ALTER TABLE session_tokens ADD COLUMN pre_boundary_snapshots TEXT')
  await tryExec(db, 'ALTER TABLE session_tokens ADD COLUMN leaf_uuids TEXT')
  await tryExec(db, 'ALTER TABLE session_tokens ADD COLUMN context_tokens INTEGER DEFAULT 0')

  // ── user_token_settings: per-token bridge preferences. Keyed by tokenId
  //    (one row per dashboard token). Used by the MITM streaming feature
  //    so per-user toggles persist across bridge restarts.
  await db.run(`
    CREATE TABLE IF NOT EXISTS user_token_settings (
      token_id TEXT PRIMARY KEY,
      streaming_enabled BOOLEAN DEFAULT TRUE,
      show_side_quests BOOLEAN DEFAULT FALSE,
      show_thinking_stream BOOLEAN DEFAULT TRUE,
      capture_request_bodies BOOLEAN DEFAULT FALSE,
      updated_at TEXT
    )
  `)

  // ── API errors — captured from CLI output stream when Claude API
  //    retries fail (e.g. "ConnectionRefused · https://api.anthropic.com
  //    /v1/messages · retry in 529ms (attempt 1/10)"). Drives the error
  //    count badge + per-period error chart on the dashboard.
  await tryExec(db, `CREATE SEQUENCE IF NOT EXISTS seq_api_errors_id START 1`)
  await db.run(`
    CREATE TABLE IF NOT EXISTS api_errors (
      id BIGINT PRIMARY KEY DEFAULT nextval('seq_api_errors_id'),
      timestamp TEXT NOT NULL,
      session_id TEXT,
      user_name TEXT,
      error_type TEXT,
      url TEXT,
      attempt INTEGER,
      max_attempts INTEGER,
      retry_ms INTEGER,
      raw TEXT
    )
  `)
  await db.run(`CREATE INDEX IF NOT EXISTS idx_api_errors_timestamp ON api_errors(timestamp)`)
  await db.run(`CREATE INDEX IF NOT EXISTS idx_api_errors_user_name ON api_errors(user_name)`)
  await db.run(`CREATE INDEX IF NOT EXISTS idx_api_errors_error_type ON api_errors(error_type)`)

  // ── jsonl_offsets: incremental scan offsets per file.
  await db.run(`
    CREATE TABLE IF NOT EXISTS jsonl_offsets (
      file_path TEXT PRIMARY KEY,
      last_offset INTEGER DEFAULT 0,
      last_size INTEGER DEFAULT 0,
      last_scanned_at TEXT
    )
  `)

  // Drop legacy tables that aren't used in PTY mode.
  await tryExec(db, 'DROP TABLE IF EXISTS sessions')          // old SDK session table
  await tryExec(db, 'DROP TABLE IF EXISTS pty_sessions')       // unused under new sweeper
  await tryExec(db, 'DROP TABLE IF EXISTS counters')

  // ── OTel telemetry — metrics + events from Claude Code CLI via OTLP receiver
  // DuckDB SEQUENCE replaces SQLite AUTOINCREMENT.
  await tryExec(db, `CREATE SEQUENCE IF NOT EXISTS seq_otel_metrics_id START 1`)
  await db.run(`
    CREATE TABLE IF NOT EXISTS otel_metrics (
      id BIGINT PRIMARY KEY DEFAULT nextval('seq_otel_metrics_id'),
      session_id TEXT,
      user_name TEXT,
      metric_name TEXT NOT NULL,
      value DOUBLE NOT NULL,
      model TEXT,
      attributes TEXT,
      timestamp TEXT NOT NULL
    )
  `)
  await tryExec(db, `CREATE INDEX IF NOT EXISTS idx_otel_session ON otel_metrics(session_id)`)
  await tryExec(db, `CREATE INDEX IF NOT EXISTS idx_otel_user ON otel_metrics(user_name)`)
  await tryExec(db, `CREATE INDEX IF NOT EXISTS idx_otel_metric ON otel_metrics(metric_name)`)
  await tryExec(db, `CREATE INDEX IF NOT EXISTS idx_otel_timestamp ON otel_metrics(timestamp)`)

  await tryExec(db, `CREATE SEQUENCE IF NOT EXISTS seq_otel_events_id START 1`)
  await db.run(`
    CREATE TABLE IF NOT EXISTS otel_events (
      id BIGINT PRIMARY KEY DEFAULT nextval('seq_otel_events_id'),
      session_id TEXT,
      user_name TEXT,
      event_name TEXT NOT NULL,
      data TEXT,
      timestamp TEXT NOT NULL
    )
  `)
  await tryExec(db, `CREATE INDEX IF NOT EXISTS idx_otel_events_session ON otel_events(session_id)`)
  await tryExec(db, `CREATE INDEX IF NOT EXISTS idx_otel_events_user ON otel_events(user_name)`)
  await tryExec(db, `CREATE INDEX IF NOT EXISTS idx_otel_events_name ON otel_events(event_name)`)
  await tryExec(db, `CREATE INDEX IF NOT EXISTS idx_otel_events_timestamp ON otel_events(timestamp)`)

  // CHECKPOINT replaces SQLite VACUUM in DuckDB — flushes WAL to main file.
  await tryExec(db, 'CHECKPOINT')

  // One-shot post-migration: reset offsets so sweeper re-scans every
  // JSONL once and populates new metadata columns. Idempotent via flag.
  try {
    const rows = (await db.runAndReadAll(`SELECT value FROM settings WHERE key = 'session_meta_v1_seeded'`)).getRowObjects()
    if (rows.length === 0) {
      await db.run('DELETE FROM jsonl_offsets')
      await db.run(`INSERT INTO settings (key, value) VALUES ('session_meta_v1_seeded', ?) ON CONFLICT DO UPDATE SET value = excluded.value`, [new Date().toISOString()])
    }
  } catch { /* ignore */ }
}

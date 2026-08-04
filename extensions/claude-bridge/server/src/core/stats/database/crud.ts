// ============================================================================
// Database CRUD — insert, update, delete, query operations
// ============================================================================

import type { RequestLogEntry, RequestFilter } from '../types'
import { getDb } from './lifecycle'

// ---------------------------------------------------------------------------
// Row <-> Entry mapping
// ---------------------------------------------------------------------------

function rowToEntry(row: any): RequestLogEntry {
  return {
    id: row.id,
    timestamp: new Date(row.timestamp),
    endpoint: row.endpoint,
    method: row.method,
    model: row.model,
    pluginId: row.plugin_id,
    userName: row.user_name ?? undefined,
    durationMs: Number(row.duration_ms ?? 0),
    inputTokens: Number(row.input_tokens ?? 0),
    outputTokens: Number(row.output_tokens ?? 0),
    cacheReadTokens: Number(row.cache_read_tokens ?? 0),
    cacheWriteTokens: Number(row.cache_write_tokens ?? 0),
    toolsUsed: row.tools_used ? JSON.parse(row.tools_used) : [],
    status: row.status,
    statusCode: Number(row.status_code ?? 0),
    error: row.error ?? undefined,
    requestBody: row.request_body ? JSON.parse(row.request_body) : undefined,
    responseText: row.response_text ?? undefined,
    isUserRequest: !!Number(row.is_user_request ?? 0),
    userMessage: row.user_message ?? undefined,
    sessionKey: row.session_key ?? undefined,
    clientTaskId: row.client_task_id ?? undefined,
    rawHeaders: row.raw_headers ? JSON.parse(row.raw_headers) : undefined,
  }
}

// ---------------------------------------------------------------------------
// Insert / Update
// ---------------------------------------------------------------------------

export async function insertRequest(entry: RequestLogEntry): Promise<void> {
  const d = await getDb()
  // INSERT OR REPLACE (SQLite) → ON CONFLICT (id) DO UPDATE (DuckDB)
  await d.run(
    `
    INSERT INTO requests
      (id, timestamp, endpoint, method, model, plugin_id, user_name,
       duration_ms, input_tokens, output_tokens, cache_read_tokens, cache_write_tokens,
       tools_used, status, status_code, error, request_body, response_text,
       is_user_request, user_message, session_key, client_task_id, raw_headers)
    VALUES
      (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
    ON CONFLICT (id) DO UPDATE SET
      timestamp = excluded.timestamp,
      endpoint = excluded.endpoint,
      method = excluded.method,
      model = excluded.model,
      plugin_id = excluded.plugin_id,
      user_name = excluded.user_name,
      duration_ms = excluded.duration_ms,
      input_tokens = excluded.input_tokens,
      output_tokens = excluded.output_tokens,
      cache_read_tokens = excluded.cache_read_tokens,
      cache_write_tokens = excluded.cache_write_tokens,
      tools_used = excluded.tools_used,
      status = excluded.status,
      status_code = excluded.status_code,
      error = excluded.error,
      request_body = excluded.request_body,
      response_text = excluded.response_text,
      is_user_request = excluded.is_user_request,
      user_message = excluded.user_message,
      session_key = excluded.session_key,
      client_task_id = excluded.client_task_id,
      raw_headers = excluded.raw_headers
    `,
    [
      entry.id,
      entry.timestamp instanceof Date ? entry.timestamp.toISOString() : String(entry.timestamp),
      entry.endpoint,
      entry.method,
      entry.model,
      entry.pluginId,
      entry.userName ?? null,
      entry.durationMs,
      entry.inputTokens,
      entry.outputTokens,
      entry.cacheReadTokens ?? 0,
      entry.cacheWriteTokens ?? 0,
      JSON.stringify(entry.toolsUsed),
      entry.status,
      entry.statusCode,
      entry.error ?? null,
      entry.requestBody ? JSON.stringify(entry.requestBody) : null,
      entry.responseText ?? null,
      entry.isUserRequest ? 1 : 0,
      entry.userMessage ?? null,
      entry.sessionKey ?? null,
      entry.clientTaskId ?? null,
      entry.rawHeaders ? JSON.stringify(entry.rawHeaders) : null,
    ],
  )
}

export async function updateRequest(id: string, updates: Partial<RequestLogEntry>): Promise<void> {
  const d = await getDb()
  const sets: string[] = []
  const values: unknown[] = []

  if (updates.durationMs !== undefined) { sets.push('duration_ms = ?'); values.push(updates.durationMs) }
  if (updates.inputTokens !== undefined) { sets.push('input_tokens = ?'); values.push(updates.inputTokens) }
  if (updates.outputTokens !== undefined) { sets.push('output_tokens = ?'); values.push(updates.outputTokens) }
  if (updates.cacheReadTokens !== undefined) { sets.push('cache_read_tokens = ?'); values.push(updates.cacheReadTokens) }
  if (updates.cacheWriteTokens !== undefined) { sets.push('cache_write_tokens = ?'); values.push(updates.cacheWriteTokens) }
  if (updates.toolsUsed !== undefined) { sets.push('tools_used = ?'); values.push(JSON.stringify(updates.toolsUsed)) }
  if (updates.status !== undefined) { sets.push('status = ?'); values.push(updates.status) }
  if (updates.statusCode !== undefined) { sets.push('status_code = ?'); values.push(updates.statusCode) }
  if (updates.error !== undefined) { sets.push('error = ?'); values.push(updates.error) }
  if (updates.model !== undefined) { sets.push('model = ?'); values.push(updates.model) }
  if (updates.requestBody !== undefined) { sets.push('request_body = ?'); values.push(JSON.stringify(updates.requestBody)) }
  if (updates.responseText !== undefined) { sets.push('response_text = ?'); values.push(updates.responseText) }
  if (updates.isUserRequest !== undefined) { sets.push('is_user_request = ?'); values.push(updates.isUserRequest ? 1 : 0) }
  if (updates.userMessage !== undefined) { sets.push('user_message = ?'); values.push(updates.userMessage) }
  if (updates.sessionKey !== undefined) { sets.push('session_key = ?'); values.push(updates.sessionKey) }
  if (updates.clientTaskId !== undefined) { sets.push('client_task_id = ?'); values.push(updates.clientTaskId) }
  if (updates.rawHeaders !== undefined) { sets.push('raw_headers = ?'); values.push(JSON.stringify(updates.rawHeaders)) }

  if (sets.length === 0) return

  values.push(id)
  await d.run(`UPDATE requests SET ${sets.join(', ')} WHERE id = ?`, values as any[])
}

// ---------------------------------------------------------------------------
// Query
// ---------------------------------------------------------------------------

export async function getRequestById(id: string): Promise<RequestLogEntry | undefined> {
  const d = await getDb()
  const rows = (await d.runAndReadAll('SELECT * FROM requests WHERE id = ?', [id])).getRowObjects()
  const row = rows[0]
  return row ? rowToEntry(row) : undefined
}

export async function getRequests(filter: RequestFilter = {}, limit = 50, offset = 0): Promise<RequestLogEntry[]> {
  const d = await getDb()
  const wheres: string[] = []
  const values: unknown[] = []

  if (filter.endpoint) { wheres.push('endpoint = ?'); values.push(filter.endpoint) }
  if (filter.pluginId) { wheres.push('plugin_id = ?'); values.push(filter.pluginId) }
  if (filter.userName) { wheres.push('user_name = ?'); values.push(filter.userName) }
  if (filter.status) { wheres.push('status = ?'); values.push(filter.status) }
  if (filter.isUserRequest !== undefined) { wheres.push('is_user_request = ?'); values.push(filter.isUserRequest ? 1 : 0) }
  if (filter.sessionKey) { wheres.push('session_key = ?'); values.push(filter.sessionKey) }

  const where = wheres.length > 0 ? `WHERE ${wheres.join(' AND ')}` : ''
  values.push(limit, offset)

  const rows = (await d.runAndReadAll(
    `SELECT * FROM requests ${where} ORDER BY timestamp DESC LIMIT ? OFFSET ?`,
    values as any[],
  )).getRowObjects()
  return rows.map(rowToEntry)
}

// ---------------------------------------------------------------------------
// Counters (persistent key-value store for global counters like webSearches)
// ---------------------------------------------------------------------------

export async function getCounter(key: string): Promise<number> {
  const d = await getDb()
  const rows = (await d.runAndReadAll('SELECT value FROM counters WHERE key = ?', [key])).getRowObjects()
  const row = rows[0] as any
  return Number(row?.value) || 0
}

export async function getCounterString(key: string): Promise<string | null> {
  const d = await getDb()
  const rows = (await d.runAndReadAll('SELECT value FROM counters WHERE key = ?', [key])).getRowObjects()
  const row = rows[0] as any
  return row?.value ?? null
}

export async function incrementCounter(key: string, amount: number): Promise<void> {
  const d = await getDb()
  await d.run(
    `
    INSERT INTO counters (key, value) VALUES (?, ?)
    ON CONFLICT (key) DO UPDATE SET value = CAST(CAST(counters.value AS INTEGER) + ? AS TEXT)
    `,
    [key, String(amount), amount],
  )
}

export async function setCounter(key: string, value: string): Promise<void> {
  const d = await getDb()
  await d.run(
    `
    INSERT INTO counters (key, value) VALUES (?, ?)
    ON CONFLICT (key) DO UPDATE SET value = excluded.value
    `,
    [key, value],
  )
}

// ---------------------------------------------------------------------------
// Delete / Prune
// ---------------------------------------------------------------------------

/** Delete a single request by id (full removal from DB). */
export async function deleteRequest(id: string): Promise<boolean> {
  const d = await getDb()
  // DuckDB doesn't return a `changes` count from .run(); use COUNT before/after.
  const before = (await d.runAndReadAll('SELECT COUNT(*) AS n FROM requests WHERE id = ?', [id])).getRowObjects()[0] as any
  if (Number(before?.n ?? 0) === 0) return false
  await d.run('DELETE FROM requests WHERE id = ?', [id])
  return true
}

/** Delete requests older than a given date (full removal from DB). */
export async function deleteRequestsBefore(before: string): Promise<number> {
  const d = await getDb()
  const beforeRow = (await d.runAndReadAll('SELECT COUNT(*) AS n FROM requests WHERE timestamp < ?', [before])).getRowObjects()[0] as any
  const cnt = Number(beforeRow?.n ?? 0)
  if (cnt === 0) return 0
  await d.run('DELETE FROM requests WHERE timestamp < ?', [before])
  return cnt
}

// Dynamic — reads from settings DB on each call
function getMaxRequests(): number {
  try {
    const { getSettingInt } = require('../../config/settings')
    return getSettingInt('maxRequests', 1000)
  } catch {
    return parseInt(process.env.CLAUDE_PROXY_MAX_REQUESTS || '1000', 10)
  }
}

/**
 * Strip capture bodies (request_body, response_text) from requests beyond
 * the retention limit. Two-phase approach:
 *
 * Phase 1 — Strip bodies from requests that have a NEWER request in the same
 *   session (older versions are redundant because the newer request contains
 *   the full conversation history).
 *
 * Phase 2 — If the remaining count (latest-per-session + sessionless) still
 *   exceeds maxRequests, strip the oldest ones by timestamp.
 */
export async function pruneOldRequests(): Promise<number> {
  const d = await getDb()
  const maxReqs = getMaxRequests()
  let totalStripped = 0

  // Phase 1: strip non-latest same-session requests.
  // Count the matching rows first because DuckDB .run() doesn't return changes.
  const phase1Sql = `
    SELECT COUNT(*) AS n FROM requests
    WHERE id IN (
      SELECT r.id FROM requests r
      INNER JOIN (
        SELECT session_key, MAX(timestamp) as max_ts
        FROM requests
        WHERE session_key IS NOT NULL
          AND (request_body IS NOT NULL OR response_text IS NOT NULL)
        GROUP BY session_key
        HAVING COUNT(*) > 1
      ) latest ON r.session_key = latest.session_key AND r.timestamp < latest.max_ts
      WHERE r.request_body IS NOT NULL OR r.response_text IS NOT NULL
    )
  `
  const phase1Count = Number(((await d.runAndReadAll(phase1Sql)).getRowObjects()[0] as any)?.n ?? 0)
  if (phase1Count > 0) {
    await d.run(`
      UPDATE requests SET request_body = NULL, response_text = NULL
      WHERE id IN (
        SELECT r.id FROM requests r
        INNER JOIN (
          SELECT session_key, MAX(timestamp) as max_ts
          FROM requests
          WHERE session_key IS NOT NULL
            AND (request_body IS NOT NULL OR response_text IS NOT NULL)
          GROUP BY session_key
          HAVING COUNT(*) > 1
        ) latest ON r.session_key = latest.session_key AND r.timestamp < latest.max_ts
        WHERE r.request_body IS NOT NULL OR r.response_text IS NOT NULL
      )
    `)
    totalStripped += phase1Count
  }

  // Phase 2: if still over limit, strip oldest remaining
  const captureCountRow = (await d.runAndReadAll(
    'SELECT COUNT(*) as cnt FROM requests WHERE request_body IS NOT NULL OR response_text IS NOT NULL',
  )).getRowObjects()[0] as any
  const captureCount = Number(captureCountRow?.cnt ?? 0)
  if (captureCount > maxReqs) {
    const toStrip = captureCount - maxReqs
    await d.run(
      `
      UPDATE requests SET request_body = NULL, response_text = NULL
      WHERE id IN (
        SELECT id FROM requests
        WHERE request_body IS NOT NULL OR response_text IS NOT NULL
        ORDER BY timestamp ASC LIMIT ?
      )
      `,
      [toStrip],
    )
    totalStripped += toStrip
  }

  return totalStripped
}

/** Delete requests matching a filter. Returns number of deleted rows. */
export async function deleteRequestsByFilter(filter: RequestFilter): Promise<number> {
  const d = await getDb()
  const wheres: string[] = []
  const values: unknown[] = []

  if (filter.endpoint) { wheres.push('endpoint = ?'); values.push(filter.endpoint) }
  if (filter.pluginId) { wheres.push('plugin_id = ?'); values.push(filter.pluginId) }
  if (filter.userName) { wheres.push('user_name = ?'); values.push(filter.userName) }
  if (filter.status) { wheres.push('status = ?'); values.push(filter.status) }
  if (filter.isUserRequest !== undefined) { wheres.push('is_user_request = ?'); values.push(filter.isUserRequest ? 1 : 0) }
  if (filter.sessionKey) { wheres.push('session_key = ?'); values.push(filter.sessionKey) }

  if (wheres.length === 0) return 0 // Safety: refuse to delete everything without a filter

  const whereClause = wheres.join(' AND ')
  const cntRow = (await d.runAndReadAll(`SELECT COUNT(*) AS n FROM requests WHERE ${whereClause}`, values as any[])).getRowObjects()[0] as any
  const cnt = Number(cntRow?.n ?? 0)
  if (cnt === 0) return 0
  await d.run(`DELETE FROM requests WHERE ${whereClause}`, values as any[])
  return cnt
}

/** Clear all requests and counters. */
export async function clearAll(): Promise<void> {
  const d = await getDb()
  await d.run('DELETE FROM requests')
  await d.run('DELETE FROM counters')
}

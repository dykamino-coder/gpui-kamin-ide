// ============================================================================
// Database Sessions CRUD — persist session state to DuckDB
// ============================================================================

import { getDb } from './lifecycle'

export interface SessionRow {
  session_key: string
  sdk_session_id: string
  plugin_id: string | null
  model: string
  tools_hash: string | null
  system_prompt_hash: string | null
  message_count: number
  turn_count: number
  input_tokens: number
  output_tokens: number
  cache_read_tokens: number
  cache_write_tokens: number
  is_active: number
  is_broken: number
  broken_reason: string | null
  created_at: string
  last_accessed_at: string
}

function rowToSession(row: any): SessionRow {
  return {
    session_key: row.session_key,
    sdk_session_id: row.sdk_session_id,
    plugin_id: row.plugin_id ?? null,
    model: row.model,
    tools_hash: row.tools_hash ?? null,
    system_prompt_hash: row.system_prompt_hash ?? null,
    message_count: Number(row.message_count ?? 0),
    turn_count: Number(row.turn_count ?? 0),
    input_tokens: Number(row.input_tokens ?? 0),
    output_tokens: Number(row.output_tokens ?? 0),
    cache_read_tokens: Number(row.cache_read_tokens ?? 0),
    cache_write_tokens: Number(row.cache_write_tokens ?? 0),
    is_active: Number(row.is_active ?? 0),
    is_broken: Number(row.is_broken ?? 0),
    broken_reason: row.broken_reason ?? null,
    created_at: row.created_at,
    last_accessed_at: row.last_accessed_at,
  }
}

// ---------------------------------------------------------------------------
// Insert / Upsert
// ---------------------------------------------------------------------------

export async function upsertSession(row: SessionRow): Promise<void> {
  const d = await getDb()
  // INSERT OR REPLACE → ON CONFLICT (session_key) DO UPDATE
  await d.run(
    `
    INSERT INTO sessions
      (session_key, sdk_session_id, plugin_id, model, tools_hash, system_prompt_hash,
       message_count, turn_count, input_tokens, output_tokens, cache_read_tokens, cache_write_tokens,
       is_active, is_broken, broken_reason, created_at, last_accessed_at)
    VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
    ON CONFLICT (session_key) DO UPDATE SET
      sdk_session_id = excluded.sdk_session_id,
      plugin_id = excluded.plugin_id,
      model = excluded.model,
      tools_hash = excluded.tools_hash,
      system_prompt_hash = excluded.system_prompt_hash,
      message_count = excluded.message_count,
      turn_count = excluded.turn_count,
      input_tokens = excluded.input_tokens,
      output_tokens = excluded.output_tokens,
      cache_read_tokens = excluded.cache_read_tokens,
      cache_write_tokens = excluded.cache_write_tokens,
      is_active = excluded.is_active,
      is_broken = excluded.is_broken,
      broken_reason = excluded.broken_reason,
      created_at = excluded.created_at,
      last_accessed_at = excluded.last_accessed_at
    `,
    [
      row.session_key, row.sdk_session_id, row.plugin_id, row.model,
      row.tools_hash, row.system_prompt_hash,
      row.message_count, row.turn_count,
      row.input_tokens, row.output_tokens, row.cache_read_tokens, row.cache_write_tokens,
      row.is_active, row.is_broken, row.broken_reason,
      row.created_at, row.last_accessed_at,
    ],
  )
}

// ---------------------------------------------------------------------------
// Update fields
// ---------------------------------------------------------------------------

export async function updateSessionFields(sessionKey: string, fields: Partial<SessionRow>): Promise<void> {
  const d = await getDb()
  const sets: string[] = []
  const values: unknown[] = []

  if (fields.message_count !== undefined) { sets.push('message_count = ?'); values.push(fields.message_count) }
  if (fields.turn_count !== undefined) { sets.push('turn_count = ?'); values.push(fields.turn_count) }
  if (fields.input_tokens !== undefined) { sets.push('input_tokens = ?'); values.push(fields.input_tokens) }
  if (fields.output_tokens !== undefined) { sets.push('output_tokens = ?'); values.push(fields.output_tokens) }
  if (fields.cache_read_tokens !== undefined) { sets.push('cache_read_tokens = ?'); values.push(fields.cache_read_tokens) }
  if (fields.cache_write_tokens !== undefined) { sets.push('cache_write_tokens = ?'); values.push(fields.cache_write_tokens) }
  if (fields.is_active !== undefined) { sets.push('is_active = ?'); values.push(fields.is_active) }
  if (fields.is_broken !== undefined) { sets.push('is_broken = ?'); values.push(fields.is_broken) }
  if (fields.broken_reason !== undefined) { sets.push('broken_reason = ?'); values.push(fields.broken_reason) }
  if (fields.last_accessed_at !== undefined) { sets.push('last_accessed_at = ?'); values.push(fields.last_accessed_at) }

  if (sets.length === 0) return
  values.push(sessionKey)
  await d.run(`UPDATE sessions SET ${sets.join(', ')} WHERE session_key = ?`, values as any[])
}

// ---------------------------------------------------------------------------
// Query
// ---------------------------------------------------------------------------

export async function getSessionByKey(sessionKey: string): Promise<SessionRow | undefined> {
  const d = await getDb()
  const rows = (await d.runAndReadAll('SELECT * FROM sessions WHERE session_key = ?', [sessionKey])).getRowObjects()
  const row = rows[0]
  return row ? rowToSession(row) : undefined
}

export async function getAllSessions(limit = 100): Promise<SessionRow[]> {
  const d = await getDb()
  const rows = (await d.runAndReadAll(
    'SELECT * FROM sessions ORDER BY last_accessed_at DESC LIMIT ?',
    [limit],
  )).getRowObjects()
  return rows.map(rowToSession)
}

export async function getActiveSessions(): Promise<SessionRow[]> {
  const d = await getDb()
  const rows = (await d.runAndReadAll(
    'SELECT * FROM sessions WHERE is_active = 1 ORDER BY last_accessed_at DESC',
  )).getRowObjects()
  return rows.map(rowToSession)
}

export async function getSessionStats(): Promise<{
  total: number
  active: number
  broken: number
  totalTurns: number
  totalInputTokens: number
  totalOutputTokens: number
  totalCacheReadTokens: number
}> {
  const d = await getDb()
  const rows = (await d.runAndReadAll(`
    SELECT
      COUNT(*) as total,
      SUM(CASE WHEN is_active = 1 THEN 1 ELSE 0 END) as active,
      SUM(CASE WHEN is_broken = 1 THEN 1 ELSE 0 END) as broken,
      SUM(turn_count) as total_turns,
      SUM(input_tokens) as total_input,
      SUM(output_tokens) as total_output,
      SUM(cache_read_tokens) as total_cache_read
    FROM sessions
  `)).getRowObjects()
  const row = (rows[0] ?? {}) as any
  return {
    total: Number(row.total ?? 0),
    active: Number(row.active ?? 0),
    broken: Number(row.broken ?? 0),
    totalTurns: Number(row.total_turns ?? 0),
    totalInputTokens: Number(row.total_input ?? 0),
    totalOutputTokens: Number(row.total_output ?? 0),
    totalCacheReadTokens: Number(row.total_cache_read ?? 0),
  }
}

// ---------------------------------------------------------------------------
// Delete / Cleanup
// ---------------------------------------------------------------------------

export async function deleteSession(sessionKey: string): Promise<boolean> {
  const d = await getDb()
  const beforeRow = (await d.runAndReadAll(
    'SELECT COUNT(*) AS n FROM sessions WHERE session_key = ?',
    [sessionKey],
  )).getRowObjects()[0] as any
  if (Number(beforeRow?.n ?? 0) === 0) return false
  await d.run('DELETE FROM sessions WHERE session_key = ?', [sessionKey])
  return true
}

export async function deleteExpiredSessions(idleTimeoutMs: number, maxLifetimeMs: number): Promise<number> {
  const d = await getDb()
  const now = new Date()
  const idleCutoff = new Date(now.getTime() - idleTimeoutMs).toISOString()
  const lifetimeCutoff = new Date(now.getTime() - maxLifetimeMs).toISOString()
  const sql = `
    SELECT COUNT(*) AS n FROM sessions
    WHERE last_accessed_at < ? OR created_at < ? OR is_broken = 1
  `
  const cntRow = (await d.runAndReadAll(sql, [idleCutoff, lifetimeCutoff])).getRowObjects()[0] as any
  const cnt = Number(cntRow?.n ?? 0)
  if (cnt === 0) return 0
  await d.run(
    `DELETE FROM sessions WHERE last_accessed_at < ? OR created_at < ? OR is_broken = 1`,
    [idleCutoff, lifetimeCutoff],
  )
  return cnt
}

export async function clearAllSessions(): Promise<void> {
  const d = await getDb()
  await d.run('DELETE FROM sessions')
}

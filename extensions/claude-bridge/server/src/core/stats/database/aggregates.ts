// ============================================================================
// Database Aggregates — summary statistics queries
// ============================================================================

import type { Aggregates } from '../types'
import { getDb } from './lifecycle'
import { getCounter, getCounterString } from './crud'

export async function getAggregates(): Promise<Aggregates> {
  const d = await getDb()

  const totalsRows = (await d.runAndReadAll(`
    SELECT
      COUNT(*) as total,
      SUM(CASE WHEN endpoint = 'anthropic' THEN 1 ELSE 0 END) as anthropic,
      SUM(CASE WHEN endpoint = 'openai' THEN 1 ELSE 0 END) as openai,
      SUM(CASE WHEN status = 'error' THEN 1 ELSE 0 END) as errors,
      SUM(CASE WHEN is_user_request = 1 THEN 1 ELSE 0 END) as user_messages,
      SUM(CASE WHEN endpoint = 'anthropic' AND is_user_request = 1 THEN 1 ELSE 0 END) as anthropic_user_messages,
      SUM(CASE WHEN endpoint = 'openai' AND is_user_request = 1 THEN 1 ELSE 0 END) as openai_user_messages,
      MAX(timestamp) as total_last,
      MAX(CASE WHEN endpoint = 'anthropic' THEN timestamp END) as anthropic_last,
      MAX(CASE WHEN endpoint = 'openai' THEN timestamp END) as openai_last,
      MAX(CASE WHEN status = 'error' THEN timestamp END) as errors_last
    FROM requests
  `)).getRowObjects()
  const totals = (totalsRows[0] ?? {}) as any

  const pluginRows = (await d.runAndReadAll(`
    SELECT plugin_id, COUNT(*) as cnt,
      SUM(CASE WHEN is_user_request = 1 THEN 1 ELSE 0 END) as user_cnt,
      MAX(timestamp) as last_at
    FROM requests GROUP BY plugin_id
  `)).getRowObjects() as any[]

  const pluginRequests: Record<string, number> = {}
  const pluginMessageCounts: Record<string, number> = {}
  const lastRequestTimes: Record<string, string> = {}

  for (const row of pluginRows) {
    pluginRequests[row.plugin_id] = Number(row.cnt ?? 0)
    pluginMessageCounts[row.plugin_id] = Number(row.user_cnt ?? 0)
    if (row.last_at) lastRequestTimes[`plugin:${row.plugin_id}`] = String(row.last_at)
  }

  const userRows = (await d.runAndReadAll(`
    SELECT user_name, COUNT(*) as cnt,
      SUM(CASE WHEN is_user_request = 1 THEN 1 ELSE 0 END) as user_cnt,
      SUM(input_tokens + COALESCE(cache_read_tokens, 0) + COALESCE(cache_write_tokens, 0)) as total_input,
      SUM(output_tokens) as total_output,
      MAX(timestamp) as last_at
    FROM requests WHERE user_name IS NOT NULL GROUP BY user_name
  `)).getRowObjects() as any[]

  const userRequests: Record<string, number> = {}
  const userMessageCounts: Record<string, number> = {}
  const userTokens: Record<string, { input: number; output: number }> = {}
  for (const row of userRows) {
    userRequests[row.user_name] = Number(row.cnt ?? 0)
    userMessageCounts[row.user_name] = Number(row.user_cnt ?? 0)
    userTokens[row.user_name] = { input: Number(row.total_input ?? 0), output: Number(row.total_output ?? 0) }
    if (row.last_at) lastRequestTimes[`user:${row.user_name}`] = String(row.last_at)
  }

  // Per-user per-model token breakdown (for cost estimation)
  const userModelRows = (await d.runAndReadAll(`
    SELECT user_name, model,
      SUM(input_tokens) as raw_input,
      SUM(output_tokens) as total_output,
      SUM(COALESCE(cache_read_tokens, 0)) as cache_read,
      SUM(COALESCE(cache_write_tokens, 0)) as cache_write
    FROM requests WHERE user_name IS NOT NULL AND model IS NOT NULL
    GROUP BY user_name, model
  `)).getRowObjects() as any[]

  const userModelTokens: Record<string, Record<string, { input: number; output: number; cacheRead: number; cacheWrite: number }>> = {}
  for (const row of userModelRows) {
    if (!userModelTokens[row.user_name]) userModelTokens[row.user_name] = {}
    userModelTokens[row.user_name]![row.model] = {
      input: Number(row.raw_input ?? 0),
      output: Number(row.total_output ?? 0),
      cacheRead: Number(row.cache_read ?? 0),
      cacheWrite: Number(row.cache_write ?? 0),
    }
  }

  // Global timestamps
  if (totals.total_last) lastRequestTimes['total'] = String(totals.total_last)
  if (totals.anthropic_last) lastRequestTimes['anthropic'] = String(totals.anthropic_last)
  if (totals.openai_last) lastRequestTimes['openai'] = String(totals.openai_last)
  if (totals.errors_last) lastRequestTimes['errors'] = String(totals.errors_last)

  // WebSearch counter from persistent counters table
  const webSearches = await getCounter('webSearches')
  const wsLastAt = await getCounterString('webSearches_lastAt')
  if (wsLastAt) lastRequestTimes['webSearches'] = wsLastAt

  return {
    total: Number(totals.total ?? 0),

    anthropic: Number(totals.anthropic ?? 0),
    openai: Number(totals.openai ?? 0),
    errors: Number(totals.errors ?? 0),
    webSearches,
    userMessages: Number(totals.user_messages ?? 0),
    anthropicUserMessages: Number(totals.anthropic_user_messages ?? 0),
    openaiUserMessages: Number(totals.openai_user_messages ?? 0),
    pluginRequests,
    userRequests,
    userMessageCounts,
    pluginMessageCounts,
    userTokens,
    userModelTokens,
    lastRequestTimes,
  }
}

// ---------------------------------------------------------------------------
// Time-series aggregation for usage chart
// ---------------------------------------------------------------------------

export interface TimeSeriesPoint {
  userName: string
  period: string
  requests: number
  inputTokens: number
  outputTokens: number
}

/**
 * Per-user usage timeseries — drives the dashboard UsageChart.
 *
 * Source: `jsonl_events` JOINed with `session_tokens.deleted=0` so the
 * series matches the new Stats overview model exactly (same definition
 * of session, same effective-tokens formula). Grouping is by the event's
 * `day` column (already in local time) — no date math on timestamp here.
 *
 * `requests` column counts assistant API calls (one per message_id in
 * jsonl_events). Input/output tokens use the same effectiveInputTokens /
 * output split as the progress bar.
 */
export async function getUserTimeSeries(
  granularity: 'hour' | 'day' | 'week' | 'month' | 'quarter' | 'year' = 'week',
  days = 0,
): Promise<TimeSeriesPoint[]> {
  if (days > 36500) days = 36500
  const d = await getDb()

  const sinceIso = days > 0
    ? new Date(Date.now() - days * 86_400_000).toISOString()
    : null

  const filters: string[] = ["(e.model IS NULL OR e.model != '<synthetic>')"]
  if (sinceIso) filters.push(`e.timestamp >= '${sinceIso}'`)
  const where = `WHERE ${filters.join(' AND ')}`

  // Group by UTC day or UTC day+hour using strftime — robust to whatever
  // the legacy day/hour columns hold. UsageChart on /ui then re-buckets
  // to local timezone via Date.getHours()/getDate().
  // NB: DuckDB strftime takes (timestamp, format) — argument order is
  // swapped from SQLite's (format, timestamp). Cast TEXT timestamp to
  // TIMESTAMP first.
  const groupCols = granularity === 'hour'
    ? `strftime(e.timestamp::TIMESTAMP, '%Y-%m-%d') || ' ' || strftime(e.timestamp::TIMESTAMP, '%H')`
    : `strftime(e.timestamp::TIMESTAMP, '%Y-%m-%d')`

  // Real new tokens per period — what was sent through the API for the
  // FIRST time:
  //   input            (uncached new user input)
  // + cache_write      (system prompt / history written to cache this
  //                     turn — i.e. NOT yet cached when first sent)
  // + output           (assistant's generated reply)
  // We deliberately exclude cache_read — that's re-reading already-
  // cached tokens, infrastructure overhead, not new content this period.
  // Dedup by message_id because CLI streams each assistant turn as N
  // JSONL entries (one per content_block_stop) sharing message.id with
  // identical usage; summing without dedup multiplies by N.
  const sql = `
    SELECT
      user_name,
      day,
      hour,
      COUNT(*) as requests,
      SUM(input_tokens) as input_tokens,
      SUM(output_tokens) as output_tokens
    FROM (
      SELECT
        st.user_name as user_name,
        ${groupCols} as day,
        ${granularity === 'hour' ? "CAST(strftime(e.timestamp::TIMESTAMP, '%H') AS INTEGER) as hour," : 'NULL as hour,'}
        e.message_id as message_id,
        MAX(COALESCE(e.input_tokens,0) + COALESCE(e.cache_write_tokens,0)) as input_tokens,
        MAX(COALESCE(e.output_tokens,0)) as output_tokens
      FROM jsonl_events e
      INNER JOIN session_tokens st
        ON st.session_id = e.session_id AND st.deleted = 0
      ${where} AND e.type = 'assistant' AND e.message_id IS NOT NULL
      GROUP BY st.user_name, day${granularity === 'hour' ? ', hour' : ''}, e.message_id
    )
    GROUP BY user_name, day, hour
    ORDER BY day ASC, user_name
  `
  const rows = (await d.runAndReadAll(sql)).getRowObjects() as Array<{
    user_name: string
    day: string
    hour: number | null
    requests: number | bigint
    input_tokens: number | bigint
    output_tokens: number | bigint
  }>

  function periodOf(day: string, _hour: number | null): string {
    // For 'hour' the SQL `day` column already includes the hour
    // ('YYYY-MM-DD HH'), so just return it as-is.
    if (granularity === 'hour') return day
    if (granularity === 'day') return day
    const [y, m, d2] = day.split('-')
    if (granularity === 'month') return `${y}-${m}`
    if (granularity === 'year') return y!
    if (granularity === 'quarter') {
      const q = Math.floor((parseInt(m!, 10) - 1) / 3) + 1
      return `${y}-Q${q}`
    }
    const date = new Date(`${y}-${m}-${d2}T00:00:00`)
    const onejan = new Date(date.getFullYear(), 0, 1)
    const week = Math.ceil((((date.getTime() - onejan.getTime()) / 86_400_000) + onejan.getDay() + 1) / 7)
    return `${y}-${String(week).padStart(2, '0')}`
  }

  const bucket = new Map<string, TimeSeriesPoint>()
  for (const r of rows) {
    const hourNum = r.hour === null || r.hour === undefined ? null : Number(r.hour)
    const period = periodOf(String(r.day), hourNum)
    const key = `${r.user_name}|${period}`
    const existing = bucket.get(key)
    const reqN = Number(r.requests ?? 0)
    const inN = Number(r.input_tokens ?? 0)
    const outN = Number(r.output_tokens ?? 0)
    if (existing) {
      existing.requests += reqN
      existing.inputTokens += inN
      existing.outputTokens += outN
    } else {
      bucket.set(key, {
        userName: r.user_name,
        period,
        requests: reqN,
        inputTokens: inN,
        outputTokens: outN,
      })
    }
  }
  return Array.from(bucket.values()).sort((a, b) =>
    a.period === b.period ? a.userName.localeCompare(b.userName) : a.period.localeCompare(b.period),
  )
}

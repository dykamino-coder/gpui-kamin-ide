// ============================================================================
// Telemetry Store — DuckDB persistence for OTel metrics & events
// ============================================================================
//
// All operations are async (DuckDB Neo client). Schema lives in
// `stats/database/schema.ts` (otel_metrics + otel_events tables) so we
// don't need an `ensureTables` step here — the DuckDB lifecycle init
// guarantees they exist before getDb() returns.

import { getDb } from '../stats/database/lifecycle'
import { warnLog } from '../logging'

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface TelemetryRecord {
  id?: number
  sessionId: string
  userName: string
  metricName: string
  value: number
  model?: string
  attributes: Record<string, string>
  timestamp: string // ISO
}

export interface TelemetryEvent {
  id?: number
  sessionId: string
  userName: string
  eventName: string
  data: Record<string, unknown>
  timestamp: string // ISO
}

export interface TokenUsageEntry {
  input: number
  output: number
  cacheRead: number
  cacheCreation: number
}

export interface TokenUsageSummary {
  byUser: Record<string, TokenUsageEntry>
  byModel: Record<string, TokenUsageEntry>
  total: TokenUsageEntry
}

export interface CostSummary {
  byUser: Record<string, number>
  bySession: Record<string, number>
  byModel: Record<string, number>
  total: number
}

// ---------------------------------------------------------------------------
// Write operations
// ---------------------------------------------------------------------------

export async function recordMetric(record: TelemetryRecord): Promise<void> {
  try {
    const d = await getDb()
    await d.run(
      `INSERT INTO otel_metrics (session_id, user_name, metric_name, value, model, attributes, timestamp)
       VALUES (?, ?, ?, ?, ?, ?, ?)`,
      [
        record.sessionId || null,
        record.userName || null,
        record.metricName,
        record.value,
        record.model || null,
        JSON.stringify(record.attributes),
        record.timestamp,
      ],
    )
  } catch (err) {
    warnLog('[telemetry] Failed to record metric', { metric: record.metricName, err: String(err) })
  }
}

export async function recordEvent(event: TelemetryEvent): Promise<void> {
  try {
    const d = await getDb()
    await d.run(
      `INSERT INTO otel_events (session_id, user_name, event_name, data, timestamp)
       VALUES (?, ?, ?, ?, ?)`,
      [
        event.sessionId || null,
        event.userName || null,
        event.eventName,
        JSON.stringify(event.data),
        event.timestamp,
      ],
    )
  } catch (err) {
    warnLog('[telemetry] Failed to record event', { event: event.eventName, err: String(err) })
  }
}

// ---------------------------------------------------------------------------
// Read operations — Metrics
// ---------------------------------------------------------------------------

export async function getMetricsBySession(sessionId: string): Promise<TelemetryRecord[]> {
  const d = await getDb()
  const rows = (await d.runAndReadAll(
    'SELECT * FROM otel_metrics WHERE session_id = ? ORDER BY timestamp DESC',
    [sessionId],
  )).getRowObjects() as Array<Record<string, unknown>>
  return rows.map(rowToMetric)
}

export async function getMetricsByUser(userName: string): Promise<TelemetryRecord[]> {
  const d = await getDb()
  const rows = (await d.runAndReadAll(
    'SELECT * FROM otel_metrics WHERE user_name = ? ORDER BY timestamp DESC',
    [userName],
  )).getRowObjects() as Array<Record<string, unknown>>
  return rows.map(rowToMetric)
}

export async function getTokenUsageSummary(): Promise<TokenUsageSummary> {
  const d = await getDb()
  const rows = (await d.runAndReadAll(`
    SELECT user_name, model, attributes, SUM(value) as total_value
    FROM otel_metrics
    WHERE metric_name = 'claude_code.token.usage'
    GROUP BY user_name, model, attributes
  `)).getRowObjects() as Array<Record<string, unknown>>

  const byUser: Record<string, TokenUsageEntry> = {}
  const byModel: Record<string, TokenUsageEntry> = {}
  const total: TokenUsageEntry = { input: 0, output: 0, cacheRead: 0, cacheCreation: 0 }

  for (const row of rows) {
    const user = (row.user_name as string) || 'unknown'
    const model = (row.model as string) || 'unknown'
    const value = Number(row.total_value ?? 0)
    const attrs = safeParseJson(row.attributes as string)
    const tokenType = attrs.type || 'unknown'

    if (!byUser[user]) byUser[user] = { input: 0, output: 0, cacheRead: 0, cacheCreation: 0 }
    if (!byModel[model]) byModel[model] = { input: 0, output: 0, cacheRead: 0, cacheCreation: 0 }

    const field = tokenTypeToField(tokenType)
    if (field) {
      byUser[user][field] += value
      byModel[model][field] += value
      total[field] += value
    }
  }

  return { byUser, byModel, total }
}

export async function getCostSummary(): Promise<CostSummary> {
  const d = await getDb()
  const byUser: Record<string, number> = {}
  const bySession: Record<string, number> = {}
  const byModel: Record<string, number> = {}
  let total = 0

  const rows = (await d.runAndReadAll(`
    SELECT user_name, session_id, model, SUM(value) as total_value
    FROM otel_metrics
    WHERE metric_name = 'claude_code.cost.usage'
    GROUP BY user_name, session_id, model
  `)).getRowObjects() as Array<Record<string, unknown>>

  for (const row of rows) {
    const user = (row.user_name as string) || 'unknown'
    const session = (row.session_id as string) || 'unknown'
    const model = (row.model as string) || 'unknown'
    const value = Number(row.total_value ?? 0)

    byUser[user] = (byUser[user] || 0) + value
    bySession[session] = (bySession[session] || 0) + value
    byModel[model] = (byModel[model] || 0) + value
    total += value
  }

  return { byUser, bySession, byModel, total }
}

// ---------------------------------------------------------------------------
// Read operations — Events
// ---------------------------------------------------------------------------

export async function getRecentEvents(limit = 100): Promise<TelemetryEvent[]> {
  const d = await getDb()
  const rows = (await d.runAndReadAll(
    'SELECT * FROM otel_events ORDER BY timestamp DESC LIMIT ?',
    [limit],
  )).getRowObjects() as Array<Record<string, unknown>>
  return rows.map(rowToEvent)
}

export async function getEventsBySession(sessionId: string): Promise<TelemetryEvent[]> {
  const d = await getDb()
  const rows = (await d.runAndReadAll(
    'SELECT * FROM otel_events WHERE session_id = ? ORDER BY timestamp DESC',
    [sessionId],
  )).getRowObjects() as Array<Record<string, unknown>>
  return rows.map(rowToEvent)
}

export async function getEventById(id: number): Promise<TelemetryEvent | undefined> {
  const d = await getDb()
  const rows = (await d.runAndReadAll(
    'SELECT * FROM otel_events WHERE id = ?',
    [id],
  )).getRowObjects() as Array<Record<string, unknown>>
  const row = rows[0]
  return row ? rowToEvent(row) : undefined
}

// ---------------------------------------------------------------------------
// Aggregate queries
// ---------------------------------------------------------------------------

export async function getMetricsSummary(): Promise<{
  totalMetrics: number
  totalEvents: number
  uniqueSessions: number
  uniqueUsers: number
}> {
  const d = await getDb()

  const metricsRow = (await d.runAndReadAll('SELECT COUNT(*) as cnt FROM otel_metrics')).getRowObjects()[0] as Record<string, unknown> | undefined
  const eventsRow = (await d.runAndReadAll('SELECT COUNT(*) as cnt FROM otel_events')).getRowObjects()[0] as Record<string, unknown> | undefined
  const sessionsRow = (await d.runAndReadAll('SELECT COUNT(DISTINCT session_id) as cnt FROM otel_metrics WHERE session_id IS NOT NULL')).getRowObjects()[0] as Record<string, unknown> | undefined
  const usersRow = (await d.runAndReadAll('SELECT COUNT(DISTINCT user_name) as cnt FROM otel_metrics WHERE user_name IS NOT NULL')).getRowObjects()[0] as Record<string, unknown> | undefined

  return {
    totalMetrics: Number(metricsRow?.cnt ?? 0),
    totalEvents: Number(eventsRow?.cnt ?? 0),
    uniqueSessions: Number(sessionsRow?.cnt ?? 0),
    uniqueUsers: Number(usersRow?.cnt ?? 0),
  }
}

/** Clear all OTel telemetry data (metrics + events). */
export async function clearOtelData(): Promise<void> {
  const d = await getDb()
  await d.run('DELETE FROM otel_metrics')
  await d.run('DELETE FROM otel_events')
}

/** Clear OTel data for a specific user. */
export async function clearOtelDataForUser(userName: string): Promise<void> {
  const d = await getDb()
  await d.run('DELETE FROM otel_metrics WHERE user_name = ?', [userName])
  await d.run('DELETE FROM otel_events WHERE user_name = ?', [userName])
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function rowToMetric(row: Record<string, unknown>): TelemetryRecord {
  return {
    id: Number(row.id ?? 0),
    sessionId: (row.session_id as string) || '',
    userName: (row.user_name as string) || '',
    metricName: (row.metric_name as string) || '',
    value: Number(row.value ?? 0),
    model: (row.model as string) || undefined,
    attributes: safeParseJson(row.attributes as string),
    timestamp: (row.timestamp as string) || '',
  }
}

function rowToEvent(row: Record<string, unknown>): TelemetryEvent {
  return {
    id: Number(row.id ?? 0),
    sessionId: (row.session_id as string) || '',
    userName: (row.user_name as string) || '',
    eventName: (row.event_name as string) || '',
    data: safeParseJson(row.data as string),
    timestamp: (row.timestamp as string) || '',
  }
}

function safeParseJson(str: string | null | undefined): Record<string, string> {
  if (!str) return {}
  try {
    return JSON.parse(str)
  } catch {
    return {}
  }
}

function tokenTypeToField(type: string): keyof TokenUsageEntry | null {
  switch (type) {
    case 'input': return 'input'
    case 'output': return 'output'
    case 'cacheRead': case 'cache_read': return 'cacheRead'
    case 'cacheCreation': case 'cache_creation': return 'cacheCreation'
    default: return null
  }
}

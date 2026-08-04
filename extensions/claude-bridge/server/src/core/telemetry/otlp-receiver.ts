// ============================================================================
// OTLP HTTP/JSON Receiver — accepts metrics and logs from Claude Code CLI
// ============================================================================
//
// Routes:
//   POST /v1/metrics  — receives OTLP metrics (resourceMetrics)
//   POST /v1/logs     — receives OTLP logs/events (resourceLogs)
//
// Expected OTLP JSON payload structure:
//   https://opentelemetry.io/docs/specs/otlp/#otlphttp
// ============================================================================

import { Hono } from 'hono'
import { recordMetric, recordEvent } from './store'
import type { TelemetryRecord, TelemetryEvent } from './store'
import { debugLog, warnLog } from '../logging'

// ---------------------------------------------------------------------------
// OTLP JSON type definitions (subset we care about)
// ---------------------------------------------------------------------------

interface OtlpAnyValue {
  stringValue?: string
  intValue?: string | number
  doubleValue?: number
  boolValue?: boolean
  kvlistValue?: { values: OtlpKeyValue[] }
  arrayValue?: { values: OtlpAnyValue[] }
}

interface OtlpKeyValue {
  key: string
  value: OtlpAnyValue
}

interface OtlpResource {
  attributes?: OtlpKeyValue[]
}

// --- Metrics ---

interface OtlpNumberDataPoint {
  attributes?: OtlpKeyValue[]
  asInt?: string | number
  asDouble?: number
  timeUnixNano?: string
  startTimeUnixNano?: string
}

interface OtlpMetric {
  name: string
  description?: string
  unit?: string
  gauge?: { dataPoints: OtlpNumberDataPoint[] }
  sum?: { dataPoints: OtlpNumberDataPoint[]; isMonotonic?: boolean }
  histogram?: { dataPoints: unknown[] }
}

interface OtlpScopeMetrics {
  scope?: { name?: string; version?: string }
  metrics: OtlpMetric[]
}

interface OtlpResourceMetrics {
  resource?: OtlpResource
  scopeMetrics: OtlpScopeMetrics[]
}

interface OtlpMetricsPayload {
  resourceMetrics: OtlpResourceMetrics[]
}

// --- Logs ---

interface OtlpLogRecord {
  timeUnixNano?: string
  observedTimeUnixNano?: string
  severityNumber?: number
  severityText?: string
  body?: OtlpAnyValue
  attributes?: OtlpKeyValue[]
  traceId?: string
  spanId?: string
}

interface OtlpScopeLogs {
  scope?: { name?: string; version?: string }
  logRecords: OtlpLogRecord[]
}

interface OtlpResourceLogs {
  resource?: OtlpResource
  scopeLogs: OtlpScopeLogs[]
}

interface OtlpLogsPayload {
  resourceLogs: OtlpResourceLogs[]
}

// ---------------------------------------------------------------------------
// Route factory
// ---------------------------------------------------------------------------

export function createOtelRoutes(): Hono {
  const otel = new Hono()

  // POST /v1/metrics
  otel.post('/v1/metrics', async (c) => {
    try {
      const body = await c.req.json<OtlpMetricsPayload>()
      const count = await processMetrics(body)
      debugLog(`[otel] Received ${count} metric data points`)
      // OTLP expects empty JSON response on success
      return c.json({}, 200)
    } catch (err) {
      warnLog('[otel] Failed to process metrics', String(err))
      return c.json({ error: 'Invalid OTLP metrics payload' }, 400)
    }
  })

  // POST /v1/logs
  otel.post('/v1/logs', async (c) => {
    try {
      const body = await c.req.json<OtlpLogsPayload>()
      const count = await processLogs(body)
      debugLog(`[otel] Received ${count} log records`)
      return c.json({}, 200)
    } catch (err) {
      warnLog('[otel] Failed to process logs', String(err))
      return c.json({ error: 'Invalid OTLP logs payload' }, 400)
    }
  })

  return otel
}

// ---------------------------------------------------------------------------
// Metrics processing
// ---------------------------------------------------------------------------

async function processMetrics(payload: OtlpMetricsPayload): Promise<number> {
  let count = 0
  if (!payload.resourceMetrics) return count

  for (const rm of payload.resourceMetrics) {
    const { sessionId, userName } = extractResourceIdentifiers(rm.resource)

    for (const sm of rm.scopeMetrics || []) {
      for (const metric of sm.metrics || []) {
        const dataPoints = metric.gauge?.dataPoints || metric.sum?.dataPoints || []

        for (const dp of dataPoints) {
          const value = extractDataPointValue(dp)
          if (value === null) continue

          const attrs = kvListToRecord(dp.attributes)
          const model = attrs.model || attrs['gen_ai.request.model'] || undefined
          const timestamp = nanoToISO(dp.timeUnixNano) || new Date().toISOString()

          const record: TelemetryRecord = {
            sessionId,
            userName,
            metricName: metric.name,
            value,
            model,
            attributes: attrs,
            timestamp,
          }

          await recordMetric(record)
          count++
        }
      }
    }
  }

  return count
}

// ---------------------------------------------------------------------------
// Logs processing
// ---------------------------------------------------------------------------

async function processLogs(payload: OtlpLogsPayload): Promise<number> {
  let count = 0
  if (!payload.resourceLogs) return count

  for (const rl of payload.resourceLogs) {
    const { sessionId, userName } = extractResourceIdentifiers(rl.resource)

    for (const sl of rl.scopeLogs || []) {
      for (const lr of sl.logRecords || []) {
        const timestamp = nanoToISO(lr.timeUnixNano || lr.observedTimeUnixNano) || new Date().toISOString()

        // Extract event name from attributes or body
        const logAttrs = kvListToRecord(lr.attributes)
        let eventName = logAttrs['event.name'] || logAttrs.event || ''

        // Extract event data from body
        const data: Record<string, unknown> = {}

        if (lr.body) {
          if (lr.body.stringValue) {
            // Simple string body — use as event name if none found, else store as message
            if (!eventName) {
              eventName = lr.body.stringValue
            } else {
              data.message = lr.body.stringValue
            }
          } else if (lr.body.kvlistValue) {
            // Structured body — extract key-value pairs
            for (const kv of lr.body.kvlistValue.values) {
              data[kv.key] = extractAnyValue(kv.value)
            }
            // Try to get event name from body if not in attributes
            if (!eventName && data.event) {
              eventName = String(data.event)
            }
          }
        }

        // Merge log attributes into data
        for (const [k, v] of Object.entries(logAttrs)) {
          if (k !== 'event.name' && k !== 'event') {
            data[k] = v
          }
        }

        // Add severity info
        if (lr.severityText) data.severity = lr.severityText
        if (lr.severityNumber) data.severityNumber = lr.severityNumber

        if (!eventName) eventName = 'unknown_event'

        const event: TelemetryEvent = {
          sessionId,
          userName,
          eventName,
          data,
          timestamp,
        }

        await recordEvent(event)
        count++
      }
    }
  }

  return count
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/**
 * Extract session ID and user name from OTLP resource attributes.
 * These are set via OTEL_RESOURCE_ATTRIBUTES in session-manager.ts:
 *   user.name=<userName>,session.custom_id=<sessionId>
 */
function extractResourceIdentifiers(resource?: OtlpResource): {
  sessionId: string
  userName: string
} {
  const attrs = kvListToRecord(resource?.attributes)
  return {
    sessionId: attrs['session.custom_id'] || attrs['session.id'] || '',
    userName: attrs['user.name'] || '',
  }
}

/**
 * Extract numeric value from an OTLP data point.
 */
function extractDataPointValue(dp: OtlpNumberDataPoint): number | null {
  if (dp.asDouble !== undefined) return dp.asDouble
  if (dp.asInt !== undefined) return typeof dp.asInt === 'string' ? parseInt(dp.asInt, 10) : dp.asInt
  return null
}

/**
 * Convert OTLP key-value list to a flat string record.
 */
function kvListToRecord(attrs?: OtlpKeyValue[]): Record<string, string> {
  const result: Record<string, string> = {}
  if (!attrs) return result

  for (const kv of attrs) {
    const val = extractAnyValue(kv.value)
    if (val !== undefined && val !== null) {
      result[kv.key] = String(val)
    }
  }

  return result
}

/**
 * Extract a primitive value from an OtlpAnyValue.
 */
function extractAnyValue(v: OtlpAnyValue): unknown {
  if (v.stringValue !== undefined) return v.stringValue
  if (v.intValue !== undefined) return typeof v.intValue === 'string' ? parseInt(v.intValue, 10) : v.intValue
  if (v.doubleValue !== undefined) return v.doubleValue
  if (v.boolValue !== undefined) return v.boolValue
  if (v.kvlistValue) {
    const obj: Record<string, unknown> = {}
    for (const kv of v.kvlistValue.values) {
      obj[kv.key] = extractAnyValue(kv.value)
    }
    return obj
  }
  if (v.arrayValue) {
    return v.arrayValue.values.map(extractAnyValue)
  }
  return undefined
}

/**
 * Convert nanosecond Unix timestamp to ISO string.
 */
function nanoToISO(nanos?: string): string | null {
  if (!nanos) return null
  try {
    const ms = Math.floor(Number(BigInt(nanos) / BigInt(1_000_000)))
    return new Date(ms).toISOString()
  } catch {
    // Fallback: try parsing as milliseconds
    try {
      const num = Number(nanos)
      if (num > 1e15) {
        // Likely nanoseconds
        return new Date(Math.floor(num / 1_000_000)).toISOString()
      }
      if (num > 1e12) {
        // Likely microseconds
        return new Date(Math.floor(num / 1_000)).toISOString()
      }
      return new Date(num).toISOString()
    } catch {
      return null
    }
  }
}

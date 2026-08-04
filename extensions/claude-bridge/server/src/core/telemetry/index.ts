// ============================================================================
// Telemetry — barrel export
// ============================================================================

export { createOtelRoutes } from './otlp-receiver'

export {
  recordMetric,
  recordEvent,
  getMetricsBySession,
  getMetricsByUser,
  getTokenUsageSummary,
  getCostSummary,
  getRecentEvents,
  getEventsBySession,
  getEventById,
  getMetricsSummary,
  clearOtelData,
  clearOtelDataForUser,
} from './store'

export type {
  TelemetryRecord,
  TelemetryEvent,
  TokenUsageEntry,
  TokenUsageSummary,
  CostSummary,
} from './store'

// ============================================================================
// Database — re-exports from focused sub-modules
// ============================================================================

// Lifecycle
export { getDb, closeDb } from './lifecycle'

// Schema
export { initSchema } from './schema'

// CRUD + counters + pruning
export {
  insertRequest,
  updateRequest,
  getRequestById,
  getRequests,
  getCounter,
  getCounterString,
  incrementCounter,
  setCounter,
  deleteRequest,
  deleteRequestsBefore,
  deleteRequestsByFilter,
  pruneOldRequests,
  clearAll,
} from './crud'

// Aggregates
export { getAggregates, getUserTimeSeries } from './aggregates'

// Sessions
export {
  upsertSession,
  updateSessionFields,
  getSessionByKey,
  getAllSessions,
  getActiveSessions,
  getSessionStats,
  deleteSession as deleteDbSession,
  deleteExpiredSessions,
  clearAllSessions,
} from './sessions'
export type { SessionRow } from './sessions'

// Re-export types from the shared types module
export type { RequestFilter, Aggregates } from '../types'

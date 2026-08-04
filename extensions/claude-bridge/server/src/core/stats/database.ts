// ============================================================================
// Re-export shim — all database functionality has moved to database/
// Existing imports from 'stats/database' continue to work.
// ============================================================================

export {
  getDb,
  closeDb,
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
  pruneOldRequests,
  clearAll,
  getAggregates,
} from './database/index'

export type { RequestFilter, Aggregates } from './types'

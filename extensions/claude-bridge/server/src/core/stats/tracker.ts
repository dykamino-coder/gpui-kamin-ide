// ============================================================================
// Stats Tracker — minimal stub for PTY mode
// Full telemetry will come from OTel in a later phase.
// ============================================================================

import type { Stats } from "../types"
import type { RequestLogEntry, ExtendedStats, RequestFilter } from "./types"

export type { RequestLogEntry, ExtendedStats, RequestFilter }

class StatsTracker {
  private stats: Stats = {
    requests: 0,
    errors: 0,
    webSearches: 0,
    startedAt: new Date(),
    lastRequestAt: null,
    lastResponse: null,
  }

  get(): Stats {
    return { ...this.stats }
  }

  getExtended(): ExtendedStats {
    return {
      ...this.stats,
      anthropicRequests: 0,
      openaiRequests: 0,
      anthropicUserMessages: 0,
      openaiUserMessages: 0,
      pluginRequests: {},
      userRequests: {},
      userMessageRequests: 0,
      userMessageCounts: {},
      pluginMessageCounts: {},
      userTokens: {},
      userModelTokens: {},
      activePlugin: 'none',
      queueActive: 0,
      queueDepth: 0,
      requestLog: [],
      lastRequestTimes: {},
    }
  }

  resetAll(): void {
    this.stats = {
      requests: 0,
      errors: 0,
      webSearches: 0,
      startedAt: new Date(),
      lastRequestAt: null,
      lastResponse: null,
    }
  }

  getFilteredRequests(_filter: RequestFilter = {}, _limit = 50, _offset = 0): RequestLogEntry[] {
    return []
  }

  getRequestById(_id: string): RequestLogEntry | undefined {
    return undefined
  }

  deleteRequest(_id: string): boolean {
    return false
  }

  deleteByFilter(_filter: RequestFilter): number {
    return 0
  }
}

export const stats = new StatsTracker()

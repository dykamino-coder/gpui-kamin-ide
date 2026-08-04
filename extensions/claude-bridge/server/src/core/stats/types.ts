// ============================================================================
// Stats Types — shared interfaces for the stats module
// ============================================================================

import type { Stats } from "../types"

// ---------------------------------------------------------------------------
// Request log entry — represents a single proxied request
// ---------------------------------------------------------------------------

export interface RequestLogEntry {
  id: string
  timestamp: Date
  endpoint: 'anthropic' | 'openai' | 'pty'
  method: string
  model: string
  pluginId: string
  userName?: string
  durationMs: number
  inputTokens: number
  outputTokens: number
  /** Tokens read from prompt cache (Anthropic cache_read_input_tokens) */
  cacheReadTokens: number
  /** Tokens written to prompt cache (Anthropic cache_creation_input_tokens) */
  cacheWriteTokens: number
  toolsUsed: string[]
  status: 'success' | 'error' | 'streaming' | 'cancelled'
  statusCode: number
  error?: string
  /** Full request body (messages, system, tools) — only for captured requests */
  requestBody?: unknown
  /** Full response text */
  responseText?: string
  /** true = real user message, false = tool follow-up from agent */
  isUserRequest?: boolean
  /** Extracted user message text (only for isUserRequest=true) */
  userMessage?: string
  /** Session key linking this request to a reusable SDK session */
  sessionKey?: string
  /** Client task/session ID from request headers (X-Roo-Task-ID, etc.) */
  clientTaskId?: string
  /** Raw request headers (JSON) for future analysis */
  rawHeaders?: Record<string, string>
}

// ---------------------------------------------------------------------------
// Request filter — used for querying the request log
// ---------------------------------------------------------------------------

export interface RequestFilter {
  endpoint?: 'anthropic' | 'openai' | 'pty'
  pluginId?: string
  userName?: string
  status?: 'success' | 'error' | 'streaming' | 'cancelled'
  isUserRequest?: boolean
  sessionKey?: string
}

// ---------------------------------------------------------------------------
// Aggregates — summary statistics from the database
// ---------------------------------------------------------------------------

export interface Aggregates {
  total: number
  anthropic: number
  openai: number
  errors: number
  webSearches: number
  userMessages: number
  anthropicUserMessages: number
  openaiUserMessages: number
  pluginRequests: Record<string, number>
  userRequests: Record<string, number>
  userMessageCounts: Record<string, number>
  pluginMessageCounts: Record<string, number>
  userTokens: Record<string, { input: number; output: number }>
  userModelTokens: Record<string, Record<string, { input: number; output: number; cacheRead: number; cacheWrite: number }>>
  lastRequestTimes: Record<string, string>
}

// ---------------------------------------------------------------------------
// Extended stats — full dashboard snapshot
// ---------------------------------------------------------------------------

export interface ExtendedStats extends Stats {
  anthropicRequests: number
  openaiRequests: number
  anthropicUserMessages: number
  openaiUserMessages: number
  pluginRequests: Record<string, number>
  userRequests: Record<string, number>
  userMessageRequests: number
  userMessageCounts: Record<string, number>
  pluginMessageCounts: Record<string, number>
  userTokens: Record<string, { input: number; output: number }>
  userModelTokens: Record<string, Record<string, { input: number; output: number; cacheRead: number; cacheWrite: number }>>
  activePlugin: string
  queueActive: number
  queueDepth: number
  modelQueues?: Record<string, { active: number; depth: number; max: number }>
  requestLog: readonly RequestLogEntry[]
  lastRequestTimes: Record<string, string>
}

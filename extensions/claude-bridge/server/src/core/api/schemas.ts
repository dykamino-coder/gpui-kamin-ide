import { z } from 'zod'

export const str = z.string()
export const num = z.number()
export const bool = z.boolean()
export const nullableString = str.nullable()
// Values arrive through JSON.parse (or Response.json), so {} is precisely the
// JSON-valued extension contract, without recursive local $refs in OpenAPI.
export const json = z.unknown()
export const object = z.record(str, json)
export const numbers = z.record(str, num)
export const ok = z.object({ ok: z.literal(true) })
export const error = z.looseObject({ error: z.union([str, object]) })
export const sessionCounts = z.object({ total: num, active: num })
export const tokenIdentity = z.object({ id: str, name: str, createdAt: str })
export const token = tokenIdentity.extend({
  token: str.describe('Masked on reads/rename; issued on create/recover only.'),
})
export const tokenName = z.object({ name: str.trim().min(1) })
export const streaming = z.object({
  enabled: bool,
  showSideQuests: bool,
  showThinkingStream: bool,
  captureRequestBodies: bool,
})
export const proxy = z.object({
  httpProxy: nullableString,
  httpsProxy: nullableString,
  noProxy: nullableString,
  caCert: nullableString,
})
export const settings = proxy.extend({
  maxConcurrent: num,
  maxConcurrentOpus: num,
  maxConcurrentSonnet: num,
  maxConcurrentHaiku: num,
  logLevel: str,
  loopDetection: bool,
  loopMinRepeats: num,
  keepToolResults: num,
  toolResultMax: num,
  compression: bool,
  maxRequests: num,
  sessionCache: bool,
})
export const settingsPatch = z.object({
  httpProxy: nullableString.optional(),
  httpsProxy: nullableString.optional(),
  noProxy: nullableString.optional(),
  caCert: nullableString.optional(),
  logLevel: nullableString.optional(),
  maxRequests: num.int().positive().optional(),
})
export const account = z.object({
  email: nullableString,
  plan: nullableString,
  organization: nullableString,
  expiresAt: num.nullable(),
  loginExpiresAt: nullableString,
  displayName: nullableString,
  error: str.optional(),
})
export const deviceLogin = z.object({ loginId: str, verificationUrl: str, userCode: str, startedAt: num })
export const codexError = z.looseObject({ message: str })
export const codexAccount = z.object({
  loggedIn: bool,
  email: nullableString,
  planType: nullableString,
  accountType: nullableString,
  pendingLogin: deviceLogin.nullable(),
  error: codexError.nullable(),
  checkFailed: bool.optional(),
})
const limitWindow = z.object({ usedPercent: num, windowMinutes: num.nullable(), resetsAt: num.nullable() }).nullable()
export const codexLimits = z.object({
  limits: z
    .object({
      buckets: z.array(
        z.object({
          id: nullableString,
          name: nullableString,
          primary: limitWindow,
          secondary: limitWindow,
          reached: nullableString,
        }),
      ),
      credits: z.object({ hasCredits: bool, unlimited: bool, balance: nullableString }).nullable(),
      planType: nullableString,
      usageAllowed: bool.nullable(),
      readAt: num,
    })
    .nullable(),
})
const projection = z.object({ percent: num, resets: str }).nullable()
export const usage = z.object({
  windows: z.array(
    z.object({
      id: str,
      label: str,
      model: str.optional(),
      percent: num,
      resetsAt: nullableString,
      resetText: nullableString,
      observedAt: str,
      source: z.enum(['statusline', 'tui-compatibility']),
      freshness: z.enum(['fresh', 'stale']),
      reason: nullableString,
    }),
  ),
  observedAt: str,
  attemptedAt: str,
  source: z.enum(['statusline', 'tui-compatibility', 'unavailable']),
  claudeCodeVersion: nullableString,
  state: z.enum(['complete', 'partial', 'unavailable']),
  reason: nullableString,
  diagnostics: z.object({ missing: z.array(str), finalScreenLines: num, outputLimited: bool }),
  extra: nullableString,
  session: projection,
  weekAll: projection,
  weekSonnet: projection,
  timestamp: str,
})
export const requestLog = z.looseObject({
  id: str,
  timestamp: str,
  endpoint: str,
  method: str,
  pluginId: str.optional(),
  model: str,
  userName: str.optional(),
  durationMs: num,
  inputTokens: num,
  outputTokens: num,
  cacheReadTokens: num,
  cacheWriteTokens: num,
  toolsUsed: z.array(str),
  status: z.enum(['success', 'error', 'streaming', 'cancelled']),
  statusCode: num,
  error: str.optional(),
  requestBody: json.optional(),
  responseText: str.optional(),
  isUserRequest: bool.optional(),
  userMessage: str.optional(),
  sessionKey: str.optional(),
})
const counts = { userMessages: num, assistantMessages: num, contextTokens: num, totalTokens: num }
export const session = z.object({
  id: str,
  userName: str,
  tokenId: str,
  engine: z.enum(['claude', 'codex']),
  cwd: str,
  state: str,
  createdAt: str,
  lastActivityAt: str,
  mcpCallCount: num,
  inputCount: num,
  ...counts,
  mcpInitialized: bool,
  mcpLastError: nullableString,
  mcpLog: z.array(object),
  cpuPercent: num.nullable(),
  memBytes: num.nullable(),
  durationSec: num,
  idleSec: num,
  attached: bool,
  detachedForSec: num.nullable(),
})
export const archivedSession = z.object({
  sessionId: str,
  title: nullableString,
  folder: nullableString,
  startedAt: nullableString,
  lastActivityAt: nullableString,
  engines: z.array(str),
  userMessages: num,
  assistantMessages: num,
  compactCount: num,
  inTokens: num,
  cacheReadTokens: num,
  cacheWriteTokens: num,
  outTokens: num,
  contextTokens: num,
  model: nullableString,
  jsonlPath: nullableString,
  jsonlAvailable: bool,
})
export const models = z.array(z.object({ name: str, engine: str.optional(), in: num, out: num, pct: num }))
export const card = z.object({
  sessions: num,
  userMessages: num,
  assistantMessages: num,
  totalTokens: num,
  favoriteModel: nullableString,
  models,
  engines: object.optional(),
  activeDays: num,
  longestStreak: num,
  peakHour: num.nullable(),
  days: z.array(z.tuple([str, num, num, num, num, numbers])),
})
export const overview = z.object({
  range: str,
  engine: str,
  engines: object,
  sessions: num,
  userMessages: num,
  assistantMessages: num,
  totalTokens: num,
  favoriteModel: nullableString,
  models,
  hourly: z.array(
    z.object({
      utcHour: str,
      model: nullableString,
      sessionId: str.optional(),
      userMessages: num,
      assistantMessages: num,
      tokens: num,
    }),
  ),
  dailySessions: z.array(z.object({ date: str, sessions: num })).optional(),
})
const mcpMetrics = {
  schemaBytes: num,
  observedSessions: num,
  unusedSessions: num,
  incompleteSessions: num,
  calls: num,
  lastCall: nullableString,
  wastedBytes: num,
}
const breakdown = z.array(z.object({ ...mcpMetrics, userName: str, project: str }))
export const mcpReport = z.object({
  rows: z.array(z.object({ ...mcpMetrics, toolName: str, source: str, breakdown })),
  servers: z.array(z.object({ ...mcpMetrics, source: str, toolCount: num, breakdown })),
  sessionLimit: num,
  observedSessions: num,
  wastedBytes: num,
})
export const user = z.object({
  userName: str,
  activeSessions: num,
  totalInputs: num,
  tokens: z.object({ input: num, output: num, cacheRead: num, cacheCreation: num }),
  contextTokens: num,
  cost: num,
  codex: object,
  engines: object,
  firstSeen: nullableString,
  lastSeen: nullableString,
  periodTokens: z.object({ d7: num, d30: num }),
})

export const forceQuery = z.object({ force: z.enum(['true', 'false', '1', '0']).optional() })
export const engineQuery = z.object({ engine: z.enum(['all', 'claude', 'codex']).optional() })
export const integerQuery = str.regex(/^-?\d+$/).describe('Integer encoded as a query string')
export const pageQuery = z.object({ limit: integerQuery.optional(), offset: integerQuery.optional() })
export const statsQuery = engineQuery.extend({
  range: z.enum(['all', '30d', '7d']).optional(),
  token: str.optional(),
  agg: z.literal('hm').optional(),
  tz: integerQuery.optional(),
})
export const timeseriesQuery = engineQuery.extend({
  period: z.enum(['hour', 'day', 'week', 'month', 'quarter', 'year']).optional(),
  days: integerQuery.optional(),
})

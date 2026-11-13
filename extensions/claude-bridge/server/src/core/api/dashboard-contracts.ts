import { z } from 'zod'
import { operation as op, type Contract } from './contract'
import * as s from './schemas'

const p = '/api/dashboard'
const d = (method: string, path: string, summary: string, response?: z.ZodType, extra: Partial<Contract> = {}) =>
  op(method, p + path, summary, response, extra)
const admin = { security: ['basicAuth', 'adminBearer'] }
const building = z.looseObject({ ready: s.bool, pct: s.num })
export const dashboardContracts: Contract[] = [
  d(
    'GET',
    '/terminal',
    'Terminal count and quick-action API equivalents',
    z.object({
      sessions: s.num,
      actions: z.array(
        z.object({ engine: z.enum(['claude', 'codex']), action: s.str, method: z.enum(['GET', 'POST']), url: s.str }),
      ),
      interactive: z.object({ url: s.str, claude: z.array(s.str), codex: z.array(s.str) }),
    }),
    {
      description:
        'Status, usage and Codex account actions use these HTTP endpoints. CLI TUI launch/resume and Claude /login,/logout use the documented /ws/terminal transport.',
    },
  ),
  d(
    'GET',
    '/auth/session',
    'Check dashboard sign-in',
    z.object({ authEnabled: s.bool, authenticated: s.bool, expiresAt: s.str.optional() }),
    { security: [] },
  ),
  d('POST', '/auth/login', 'Create an administrator session', z.object({ token: s.str, expiresAt: s.str }), {
    security: [],
    body: z.object({ username: s.str, password: s.str }),
    description: 'Issues a session credential once, as the dashboard login does. Passwords are never returned.',
  }),
  d('POST', '/auth/logout', 'Revoke the current dashboard session', s.ok),
  d(
    'GET',
    '/status',
    'Server, database and system status',
    z.object({
      service: s.str,
      version: s.str,
      mode: s.str,
      host: s.str,
      port: s.num,
      debug: s.bool,
      verbose: s.bool,
      sessions: s.sessionCounts,
      os: s.str,
      nodeVersion: s.str,
      codex: s.object,
      db: z.object({ type: s.str, version: s.nullableString, sizeBytes: s.num }),
      system: s.object,
    }),
  ),
  d(
    'GET',
    '/config',
    'Configuration shown in the dashboard',
    s.settings.extend({ port: s.num, host: s.str, mode: s.str }),
  ),
  d('GET', '/proxy-settings', 'Read editable configuration', s.settings),
  d('PUT', '/proxy-settings', 'Update proxy, logging and request-history settings', s.settings, {
    ...admin,
    body: s.settingsPatch,
    description:
      'Partial update. null or an empty string clears a proxy/logging override. maxRequests is a positive integer. Other configuration fields are read-only legacy projections.',
  }),
  d('DELETE', '/proxy-settings', 'Reset configuration overrides', s.settings, admin),
  d('POST', '/proxy-settings/cert', 'Upload a corporate CA certificate', s.proxy.extend({ path: s.str }), {
    ...admin,
    media: 'multipart/form-data',
    body: z.object({ cert: z.file() }),
  }),
  d(
    'GET',
    '/db/diag',
    'Database table sizes',
    z.object({
      fileMB: s.num,
      tables: z.array(z.object({ table: s.str, rows: s.num, textMB: s.num })),
      sumTextMB: s.num,
      freeBlockRatio: s.num.nullable(),
    }),
  ),
  d(
    'POST',
    '/db/compact',
    'Compact the statistics database',
    z.object({ ok: s.bool, beforeMB: s.num, afterMB: s.num, error: s.str.optional() }),
    { ...admin, statuses: [200, 409] },
  ),
  d(
    'POST',
    '/db/retention',
    'Apply statistics retention',
    z.object({ otelEventsDeleted: s.num, apiErrorsDeleted: s.num }),
    admin,
  ),
  d('GET', '/account', 'Claude account summary', s.account, { query: s.forceQuery }),
  d(
    'GET',
    '/auth-status',
    'Claude authentication status',
    z.object({
      loggedIn: s.bool,
      authMethod: s.str.optional(),
      apiProvider: s.str.optional(),
      error: s.str.optional(),
    }),
    { query: s.forceQuery },
  ),
  d(
    'POST',
    '/health/refresh',
    'Refresh CLI health without sending a paid prompt',
    z.object({
      sdk: s.bool,
      account: z.object({ email: s.str.optional(), plan: s.str.optional() }).nullable(),
      apiPing: s.num.nullable(),
      cliVersion: s.str.optional(),
      sessions: s.sessionCounts.optional(),
      error: s.str.optional(),
    }),
    { query: s.forceQuery.extend({ prompt: s.str.optional() }), statuses: [200, 503] },
  ),
  d('GET', '/usage', 'Claude plan usage windows', s.usage, { query: s.forceQuery }),
  d('GET', '/codex/account', 'Codex account and device login state', s.codexAccount, { query: s.forceQuery }),
  d('GET', '/codex/limits', 'Codex plan limits', s.codexLimits, { query: s.forceQuery }),
  d('POST', '/codex/login', 'Begin Codex device login', s.deviceLogin, {
    description:
      'Returns the short-lived user code required for the login flow. Does not return auth.json or account credentials.',
  }),
  d('POST', '/codex/login/cancel', 'Cancel Codex device login', s.ok),
  d('POST', '/codex/logout', 'Sign out of Codex', s.ok),
  d('GET', '/codex/models', 'Available Codex models', z.object({ models: z.array(s.object) })),
  d('GET', '/tokens', 'List tokens (full values for administrator sessions)', z.array(s.token), {
    security: [{ adminBearer: [] }, {}],
    description:
      'Preserves the existing dashboard contract: administrator session Bearer receives full token values; other callers receive masked values.',
  }),
  d('POST', '/tokens', 'Create or recover a user token', s.token, {
    body: s.tokenName,
    statuses: [200, 201],
    security: [],
    description: 'Existing first-install contract: idempotent by name, returns the issued credential for client setup.',
  }),
  d('POST', '/tokens/resolve', 'Resolve a token already held by the caller', s.tokenIdentity, {
    body: z.object({ token: s.str }),
    security: [],
  }),
  d('PATCH', '/tokens/:id', 'Rename a user token', s.token, {
    body: s.tokenName,
    security: [{ adminBearer: [] }, {}],
    description:
      'Returns the same token representation as GET /tokens: full for administrator session Bearer, masked otherwise.',
  }),
  d('DELETE', '/tokens/:id', 'Delete a user token', s.ok, { security: [] }),
  d(
    'GET',
    '/tokens/:tokenId/streaming-settings',
    'Per-token streaming settings',
    s.streaming.extend({ error: s.str.optional() }),
    { security: [] },
  ),
  d('PUT', '/tokens/:tokenId/streaming-settings', 'Update per-token streaming settings', s.streaming, {
    body: s.streaming.partial(),
    security: [],
  }),
  d('GET', '/sessions', 'Live PTY sessions and counters', z.object({ sessions: z.array(s.session) }), admin),
  d('POST', '/sessions/:id/kill', 'Terminate a live session', s.ok.extend({ id: s.str })),
  d(
    'GET',
    '/sessions/:id/mcp-log',
    'Recent MCP calls for a session',
    z.object({
      sessionId: s.str,
      mcpInitialized: s.bool,
      mcpLastError: s.nullableString,
      mcpCallCount: s.num,
      log: z.array(s.object),
    }),
  ),
  d('GET', '/sessions/tree', 'Agent hierarchy for a token', z.object({ tree: z.array(s.object) }), {
    query: z.object({ tokenId: s.str.min(1) }),
  }),
  d('GET', '/sessions/stats', 'Live session counts by user', s.sessionCounts.extend({ byUser: s.numbers }), admin),
  d(
    'GET',
    '/tokens/:tokenId/sessions',
    'Historical sessions for a user',
    z.object({
      tokenId: s.str,
      sessions: z.array(s.archivedSession),
      total: s.num,
      next: s.nullableString.optional(),
      building: building.optional(),
    }),
    { query: s.engineQuery.extend({ limit: s.integerQuery.optional(), before: s.str.optional() }) },
  ),
  d('GET', '/sessions/:sessionId/jsonl', 'Stream a session transcript', undefined, {
    responseMedia: 'application/x-ndjson',
    description: 'Bounded raw byte stream, not a JSON array; does not buffer the full file.',
  }),
  d('GET', '/sessions/:sessionId/jsonl-download', 'Consume a one-use transcript download grant', undefined, {
    responseMedia: 'application/x-ndjson',
    description:
      'First POST to this path; retain the HttpOnly bridge_jsonl_download cookie and GET the returned URL within 30 seconds. The grant is one-use and scoped to this session.',
  }),
  d(
    'POST',
    '/sessions/:sessionId/jsonl-download',
    'Issue a one-use transcript download URL',
    z.looseObject({ url: s.str }),
  ),
  d(
    'DELETE',
    '/sessions/:sessionId',
    'Delete historical session data',
    s.ok.extend({ softDeleted: s.bool, fileRemoved: s.bool }),
    {
      query: z.object({ rmFile: z.enum(['0', '1']).optional() }),
      description: 'Soft-deletes statistics/transcript data. Does not terminate a live process; use /kill.',
    },
  ),
  d(
    'GET',
    '/stats',
    'Telemetry statistics and live sessions',
    z.object({
      requests: s.num,
      errors: s.num,
      webSearches: s.num,
      startedAt: s.str,
      lastRequestAt: s.nullableString,
      lastResponse: s.nullableString,
      userRequests: s.numbers,
      userTokens: z.record(s.str, z.object({ input: s.num, output: s.num })),
      userModelTokens: s.object,
      requestLog: z.array(s.requestLog),
      lastRequestTimes: z.record(s.str, s.str),
      tokenUsage: s.object,
      cost: s.object,
      metrics: s.object,
      sessions: z.array(s.object),
    }),
    admin,
  ),
  d(
    'GET',
    '/stats/timeseries',
    'User usage over time',
    z.array(
      z.object({
        userName: s.str,
        engine: s.str.optional(),
        period: s.str,
        requests: s.num,
        inputTokens: s.num,
        outputTokens: s.num,
      }),
    ),
    { ...admin, query: s.timeseriesQuery },
  ),
  d('GET', '/stats/overview', 'Rollup statistics and heatmap buckets', s.overview, { query: s.statsQuery }),
  d(
    'GET',
    '/stats/users-overview',
    'Batched user cards or revision acknowledgement',
    z.union([
      z.object({ unchanged: z.literal(true), revision: s.str }),
      z.object({
        revision: s.str,
        ready: s.bool,
        range: s.str,
        cards: z.record(s.str, s.card).optional(),
        building: building.optional(),
      }),
    ]),
    {
      query: s.engineQuery.extend({
        range: z.enum(['all', '30d', '7d']).optional(),
        tz: s.str.optional(),
        tzOffset: s.integerQuery.optional(),
        subject: s.str.optional(),
        rev: s.str.optional(),
      }),
    },
  ),
  d('GET', '/users', 'Users and token/cost totals', z.array(s.user), { ...admin, query: s.engineQuery }),
  d('GET', '/users/:userName/requests', 'Recent requests for one user', z.array(s.requestLog), {
    query: z.object({ limit: s.integerQuery.optional() }),
  }),
  d('GET', '/user-chat/:userName', 'Export user messages', z.array(s.requestLog), {
    query: z.object({ limit: s.integerQuery.optional() }),
  }),
  d('GET', '/requests', 'Recent telemetry request log', z.array(s.requestLog), {
    ...admin,
    query: s.pageQuery.extend({ userName: s.str.optional(), status: s.str.optional(), endpoint: s.str.optional() }),
    description: 'Unsupported filters are rejected by the handler rather than silently ignored.',
  }),
  d('DELETE', '/requests', 'Clear all request history', s.ok, admin),
  d(
    'DELETE',
    '/requests/filter',
    'Delete history for one user',
    s.ok.extend({ deleted: s.num, events: s.num, metrics: s.num, legacy: s.num }),
    { query: z.object({ userName: s.str, status: s.str.optional(), endpoint: s.str.optional() }) },
  ),
  d('GET', '/requests/:id', 'Request log detail', s.requestLog, admin),
  d('DELETE', '/requests/:id', 'Delete one request log entry', s.ok, admin),
  d(
    'GET',
    '/errors/count',
    'Error counts',
    z.object({ total: s.num, last24h: s.num, lastAt: s.nullableString, byEngine: s.numbers }),
    { ...admin, query: s.engineQuery },
  ),
  d(
    'GET',
    '/errors/timeseries',
    'Errors over time',
    z.array(z.object({ period: s.str, count: s.num, connectionRefused: s.num, timeouts: s.num, rateLimited: s.num })),
    { ...admin, query: s.timeseriesQuery },
  ),
  d(
    'GET',
    '/errors/recent',
    'Recent API errors',
    z.array(
      z.looseObject({
        timestamp: s.str,
        sessionId: s.nullableString,
        userName: s.nullableString,
        engine: s.str,
        errorType: s.nullableString,
        url: s.nullableString,
        attempt: s.num,
        maxAttempts: s.num,
        retryMs: s.num,
        raw: s.nullableString,
      }),
    ),
    { ...admin, query: s.engineQuery.extend({ limit: s.integerQuery.optional() }) },
  ),
  d('GET', '/mcp-report', 'MCP schema usage and wasted bytes', s.mcpReport, admin),
  d(
    'GET',
    '/transcripts',
    'Transcripts owned by the bearer token',
    z.object({ transcripts: z.array(z.object({ conversationId: s.str, size: s.num })) }),
    {
      security: ['apiBearer'],
      description: 'Uses a user API token, including when invoked by an administrator. Ownership is unchanged.',
    },
  ),
  d('GET', '/transcripts/:conversationId', 'Read a bounded transcript byte range', undefined, {
    security: ['apiBearer'],
    responseMedia: 'application/x-ndjson',
    query: z.object({ offset: s.integerQuery.optional(), len: s.integerQuery.optional() }),
  }),
]

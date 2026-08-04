// ============================================================================
// Dashboard stats & request log endpoints — powered by OTel telemetry store
// ============================================================================

import type { Hono } from 'hono'
import path from 'path'
import {
  getTokenUsageSummary,
  getCostSummary,
  getMetricsSummary,
  getRecentEvents,
  getEventById,
  clearOtelData,
  clearOtelDataForUser,
} from '../../telemetry/store'
import { getUserTimeSeries } from '../../stats/database/aggregates'
import { clearAll, deleteRequest, deleteRequestsByFilter } from '../../stats/database/crud'
import { getAllSessions } from '../../pty/session-manager'
import { denyApiToken, denyForeignUser, denyForeignSubject } from '../authz'

/** Resolve the owning user_name for a sessionId (session_tokens is the
 *  authoritative owner map). Returns undefined if unknown. */
async function sessionOwner(sessionId: string): Promise<string | undefined> {
  const { getDb } = await import('../../stats/database/lifecycle')
  const db = await getDb()
  const row = (await db.runAndReadAll(
    `SELECT user_name FROM session_tokens WHERE session_id = ? LIMIT 1`,
    [sessionId],
  )).getRowObjects()[0] as { user_name?: string } | undefined
  return row?.user_name
}

// In-memory cache for the Usage chart timeseries — see /timeseries
// handler. Key = `${period}|${days}`, TTL ~30s. Avoids re-running the
// DuckDB scan on every dashboard refresh.
const timeseriesCache: Map<string, { at: number; data: unknown }> = new Map()

// Same idea for /stats/overview — with many users each panel mount fired
// 4 DuckDB scans; concurrent dashboards saturated the connection pool and
// the panel sat on "Loading…" forever. Key = range|token|agg|tz.
const overviewCache: Map<string, { at: number; data: unknown }> = new Map()
const OVERVIEW_CACHE_TTL_MS = 30_000

// Request-triggered catch-up sweeps used to run on EVERY stats request.
// The background sweeper already runs every 5 min — throttle the extra
// per-request kick so a burst of dashboard opens can't stack sweeps on
// top of the heavy scans they're waiting for.
let lastRequestSweepAt = 0
function sweepThrottled(sweepAll: () => Promise<unknown>): void {
  const now = Date.now()
  if (now - lastRequestSweepAt < 60_000) return
  lastRequestSweepAt = now
  sweepAll().catch(() => { /* fire-and-forget catch-up */ })
}

export function registerStatsRoutes(api: Hono): void {
  // GET /api/dashboard/stats -- OTel-based statistics. Admin-only: returns every
  // user's session cwd (absolute paths) + per-user token/model breakdown.
  api.get('/api/dashboard/stats', async (c) => {
    const denied = denyApiToken(c); if (denied) return denied
    const tokenUsage = await getTokenUsageSummary()
    const cost = await getCostSummary()
    const metrics = await getMetricsSummary()

    // Build session summary from PTY session manager
    const ptySessions = getAllSessions()
    const sessions = ptySessions.map((s) => ({
      id: s.id,
      userName: s.userName,
      cwd: s.cwd,
      state: s.state,
      createdAt: s.createdAt.toISOString(),
      lastActivityAt: s.lastActivityAt.toISOString(),
      mcpCallCount: s.mcpCallCount,
      inputCount: s.inputCount,
    }))

    // Compute aggregated totals compatible with the ExtendedStats shape
    const totalInput = tokenUsage.total.input + tokenUsage.total.cacheRead + tokenUsage.total.cacheCreation
    const totalOutput = tokenUsage.total.output

    // Build per-user token maps in the shape the frontend expects
    const userTokens: Record<string, { input: number; output: number }> = {}
    const userModelTokens: Record<string, Record<string, {
      input: number; output: number; cacheRead: number; cacheWrite: number
    }>> = {}

    for (const [user, entry] of Object.entries(tokenUsage.byUser)) {
      userTokens[user] = {
        input: entry.input + entry.cacheRead + entry.cacheCreation,
        output: entry.output,
      }
    }

    // Build per-user per-model tokens from cost summary + token usage
    for (const [model, entry] of Object.entries(tokenUsage.byModel)) {
      // Associate model tokens with each user that has usage
      for (const [user] of Object.entries(tokenUsage.byUser)) {
        if (!userModelTokens[user]) userModelTokens[user] = {}
        userModelTokens[user]![model] = {
          input: entry.input,
          output: entry.output,
          cacheRead: entry.cacheRead,
          cacheWrite: entry.cacheCreation,
        }
      }
    }

    // Build per-user request counts from metrics
    const userRequests: Record<string, number> = {}
    for (const [user] of Object.entries(tokenUsage.byUser)) {
      userRequests[user] = 1 // at least 1 request if they have token usage
    }

    return c.json({
      // Base Stats fields
      requests: metrics.totalEvents,
      errors: 0,
      webSearches: 0,
      startedAt: new Date().toISOString(),
      lastRequestAt: null,
      lastResponse: null,

      // ExtendedStats fields
      userRequests,
      userTokens,
      userModelTokens,
      requestLog: [],
      lastRequestTimes: {},

      // OTel-specific data
      tokenUsage,
      cost,
      metrics,
      sessions,
    })
  })

  // GET /api/dashboard/requests -- recent OTel events
  api.get('/api/dashboard/requests', async (c) => {
    const denied = denyApiToken(c); if (denied) return denied
    const limit = parseInt(c.req.query('limit') || '50')
    const events = await getRecentEvents(limit)

    // Map OTel events to a shape compatible with RequestLogEntry where possible
    const entries = events.map((ev) => ({
      id: String(ev.id ?? ''),
      timestamp: ev.timestamp,
      endpoint: 'anthropic' as const,
      method: 'POST',
      model: (ev.data.model as string) || 'unknown',
      userName: ev.userName || undefined,
      durationMs: (ev.data.durationMs as number) || 0,
      inputTokens: (ev.data.inputTokens as number) || 0,
      outputTokens: (ev.data.outputTokens as number) || 0,
      cacheReadTokens: (ev.data.cacheReadTokens as number) || 0,
      cacheWriteTokens: (ev.data.cacheWriteTokens as number) || 0,
      toolsUsed: [] as string[],
      status: 'success' as const,
      statusCode: 200,
      error: (ev.data.error as string) || undefined,
      isUserRequest: false,
      userMessage: undefined,
      sessionKey: ev.sessionId || undefined,
      // Preserve the original event for richer detail
      eventName: ev.eventName,
      eventData: ev.data,
    }))

    return c.json(entries)
  })

  // DELETE /api/dashboard/requests -- clear ALL history (requests + OTel) for
  // every user. Admin-only destructive op.
  api.delete('/api/dashboard/requests', async (c) => {
    const denied = denyApiToken(c); if (denied) return denied
    await clearAll()
    await clearOtelData()
    return c.json({ ok: true })
  })

  // DELETE /api/dashboard/requests/filter -- delete by filter. A per-user token
  // may only clear its OWN userName; admin may target anyone.
  api.delete('/api/dashboard/requests/filter', async (c) => {
    const endpoint = c.req.query('endpoint') || undefined
    const userName = c.req.query('userName') || undefined
    const status = c.req.query('status') || undefined
    const denied = denyForeignUser(c, userName); if (denied) return denied
    const deleted = await deleteRequestsByFilter({ endpoint: endpoint as any, userName, status: status as any })
    // Also clear OTel data for user if filtering by userName
    if (userName) await clearOtelDataForUser(userName)
    return c.json({ ok: true, deleted })
  })

  // GET /api/dashboard/requests/:id -- single event detail
  api.get('/api/dashboard/requests/:id', async (c) => {
    const denied = denyApiToken(c); if (denied) return denied
    const idParam = c.req.param('id')
    const numericId = parseInt(idParam, 10)
    if (isNaN(numericId)) return c.json({ error: 'Invalid ID' }, 400)

    const event = await getEventById(numericId)
    if (!event) return c.json({ error: 'Request not found' }, 404)

    return c.json({
      id: String(event.id ?? ''),
      timestamp: event.timestamp,
      endpoint: 'anthropic',
      method: 'POST',
      model: (event.data.model as string) || 'unknown',
      userName: event.userName || undefined,
      durationMs: (event.data.durationMs as number) || 0,
      inputTokens: (event.data.inputTokens as number) || 0,
      outputTokens: (event.data.outputTokens as number) || 0,
      cacheReadTokens: (event.data.cacheReadTokens as number) || 0,
      cacheWriteTokens: (event.data.cacheWriteTokens as number) || 0,
      toolsUsed: [],
      status: 'success',
      statusCode: 200,
      error: (event.data.error as string) || undefined,
      isUserRequest: false,
      sessionKey: event.sessionId || undefined,
      eventName: event.eventName,
      eventData: event.data,
    })
  })

  // DELETE /api/dashboard/requests/:id -- delete a single request
  api.delete('/api/dashboard/requests/:id', async (c) => {
    const denied = denyApiToken(c); if (denied) return denied // was unguarded: cross-user row delete
    const id = c.req.param('id')
    const ok = await deleteRequest(id)
    return ok ? c.json({ ok: true }) : c.json({ error: 'Not found' }, 404)
  })

  // GET /api/dashboard/stats/timeseries -- user usage over time
  // (jsonl_events × session_tokens, deleted=0). The background sweeper
  // (`startJsonlSweeper`) keeps DuckDB hot every 5 min; we fire one
  // extra catch-up sweep in the background on each request but do NOT
  // block the response on it. A small in-memory result cache (~30s)
  // smooths out tab refreshes — the chart never needs to be second-by-
  // second precise.
  api.get('/api/dashboard/stats/timeseries', async (c) => {
    const denied = denyApiToken(c); if (denied) return denied
    const { sweepAll } = await import('../../stats/jsonl-sweeper')
    sweepThrottled(sweepAll)
    const period = (c.req.query('period') || 'day') as 'hour' | 'day' | 'week' | 'month' | 'quarter' | 'year'
    const days = parseInt(c.req.query('days') || '0', 10)
    const cacheKey = `${period}|${days}`
    const cached = timeseriesCache.get(cacheKey)
    const now = Date.now()
    if (cached && now - cached.at < 30_000) return c.json(cached.data)
    const data = await getUserTimeSeries(period, days)
    timeseriesCache.set(cacheKey, { at: now, data })
    return c.json(data)
  })

  // GET /api/dashboard/stats/overview?range=all|30d|7d&token=<userName>
  //
  // Returns raw, UTC-honest data; the client groups by local timezone to
  // compute heatmap, peakHour, streaks, daily timeseries.
  //
  // Two aggregation modes:
  //  - legacy (no `agg` param): hourly buckets per (hour × model × session).
  //    Kept for the shipped Electron client. On busy multi-user servers the
  //    per-session split multiplied the payload into the megabytes and the
  //    Stats panel never finished loading.
  //  - `agg=hm&tz=<getTimezoneOffset() minutes>`: buckets per (hour × model)
  //    only, plus a tiny `dailySessions` array with the EXACT per-local-day
  //    distinct session count (the sole thing the client used sessionId for).
  //    Payload shrinks by the session factor; tz is used ONLY for that one
  //    day-bucketed distinct count.
  //
  // Token formula matches the per-session progress bar:
  //   effectiveInputTokens = input + cache_read + cache_write  (no output)
  api.get('/api/dashboard/stats/overview', async (c) => {
    const { getDb } = await import('../../stats/database/lifecycle')
    const { sweepAll } = await import('../../stats/jsonl-sweeper')
    sweepThrottled(sweepAll)

    const db = await getDb()
    const range = (c.req.query('range') || 'all') as 'all' | '30d' | '7d'
    // API-token callers are pinned to their own userName by the auth
    // middleware; admin/session callers may pass ?token to slice by user.
    const apiUserName = (c as any).get('apiUserName') as string | undefined
    const tokenId = apiUserName || c.req.query('token') || null
    const compact = c.req.query('agg') === 'hm'
    // JS getTimezoneOffset() semantics: UTC − local, minutes. Clamped to
    // the real-world range so the interpolated INTERVAL stays a plain int.
    const tzRaw = parseInt(c.req.query('tz') || '0', 10)
    const tzOffsetMin = Math.max(-840, Math.min(840, Number.isFinite(tzRaw) ? tzRaw : 0))
    const sinceDays = range === '7d' ? 7 : range === '30d' ? 30 : null
    // Range cutoff is timestamp-based (UTC ms), client decides what
    // "today" means for streaks.
    const sinceIso = sinceDays
      ? new Date(Date.now() - sinceDays * 86_400_000).toISOString()
      : null

    const cacheKey = `${range}|${tokenId ?? ''}|${compact ? 'hm' : 'v1'}|${tzOffsetMin}`
    const cachedOverview = overviewCache.get(cacheKey)
    if (cachedOverview && Date.now() - cachedOverview.at < OVERVIEW_CACHE_TTL_MS) {
      return c.json(cachedOverview.data)
    }

    const joinClause = `
      FROM jsonl_events e
      INNER JOIN session_tokens st
        ON st.session_id = e.session_id AND st.deleted = 0
    `
    const filters: string[] = ["(e.model IS NULL OR e.model != '<synthetic>')"]
    if (sinceIso) filters.push(`e.timestamp >= '${sinceIso}'`)
    if (tokenId) filters.push(`st.user_name = '${tokenId.replace(/'/g, "''")}'`)
    const where = `WHERE ${filters.join(' AND ')}`

    const TOKEN_SUM = `(COALESCE(e.input_tokens,0) + COALESCE(e.cache_read_tokens,0) + COALESCE(e.cache_write_tokens,0))`

    // Skalar aggregates — timezone-independent. We split user / assistant
    // message counts so the UI can show both ("12 user / 47 assistant").
    const counters = (await db.runAndReadAll(`
      SELECT
        COUNT(DISTINCT e.session_id) as sessions,
        SUM(CASE WHEN e.type = 'user' THEN 1 ELSE 0 END) as user_messages,
        SUM(CASE WHEN e.type = 'assistant' THEN 1 ELSE 0 END) as assistant_messages
      ${joinClause} ${where}
    `)).getRowObjects()[0] as any

    // "Session tokens" = sum of each session's last-assistant effective
    // input (matches the per-session Context column in the sessions
    // modal). Doesn't double-count cache_read flowing through every turn.
    // arg_max = "value at max(timestamp)" — one scan, instead of the old
    // correlated ORDER-BY-LIMIT-1 subquery that re-probed jsonl_events
    // once per session and dominated the endpoint on busy servers.
    const sessionTokensRow = (await db.runAndReadAll(`
      SELECT SUM(last_ctx) as total
      FROM (
        SELECT arg_max(
          COALESCE(e.input_tokens,0) + COALESCE(e.cache_read_tokens,0) + COALESCE(e.cache_write_tokens,0),
          e.timestamp
        ) as last_ctx
        FROM jsonl_events e
        INNER JOIN session_tokens st
          ON st.session_id = e.session_id AND st.deleted = 0
        WHERE e.type = 'assistant' AND (e.model IS NULL OR e.model != '<synthetic>')
          ${tokenId ? `AND st.user_name = '${tokenId.replace(/'/g, "''")}'` : ''}
        GROUP BY e.session_id
      )
    `)).getRowObjects()[0] as { total: bigint | number | null } | undefined
    const sessionTokens = Number(sessionTokensRow?.total ?? 0)

    // Per-model totals — order matters for "favorite", but no time bucketing.
    const modelTotalsRaw = (await db.runAndReadAll(`
      SELECT e.model as model,
        SUM(${TOKEN_SUM}) as in_tokens,
        SUM(COALESCE(e.output_tokens,0)) as out_tokens
      ${joinClause} ${where} AND e.type = 'assistant' AND e.model IS NOT NULL AND e.model != '' AND e.model != '<synthetic>'
      GROUP BY e.model
      ORDER BY (in_tokens + out_tokens) DESC
    `)).getRowObjects() as Array<{ model: string; in_tokens: bigint | number; out_tokens: bigint | number }>
    const modelTotals = modelTotalsRaw.map(r => ({
      model: r.model,
      in_tokens: Number(r.in_tokens ?? 0),
      out_tokens: Number(r.out_tokens ?? 0),
    }))
    const modelsTotalAll = modelTotals.reduce((s, r) => s + r.in_tokens + r.out_tokens, 0)
    const models = modelTotals.map(r => ({
      name: r.model,
      in: r.in_tokens,
      out: r.out_tokens,
      pct: modelsTotalAll > 0 ? ((r.in_tokens + r.out_tokens) / modelsTotalAll) * 100 : 0,
    }))
    const favoriteModel = models[0]?.name ?? null

    // Hourly buckets — UTC ISO hour key like '2026-04-25T19'. Client
    // re-buckets by local TZ for heatmap, peakHour, streaks, daily
    // model timeseries. DuckDB strftime takes (timestamp, format) — cast
    // TEXT timestamp to TIMESTAMP first.
    //
    // compact mode groups by (hour, model) only — the per-session split
    // exists solely so the LEGACY client can count distinct sessions per
    // local day; compact clients get that from `dailySessions` below.
    const hourly = (await db.runAndReadAll(`
      SELECT
        strftime(e.timestamp::TIMESTAMP, '%Y-%m-%dT%H') as utc_hour,
        e.model as model,
        ${compact ? `'' as session_id,` : `e.session_id as session_id,`}
        SUM(CASE WHEN e.type = 'user' THEN 1 ELSE 0 END) as user_msgs,
        SUM(CASE WHEN e.type = 'assistant' THEN 1 ELSE 0 END) as assistant_msgs,
        SUM(CASE WHEN e.type = 'assistant' THEN ${TOKEN_SUM} ELSE 0 END) as model_tokens
      ${joinClause} ${where}
      GROUP BY utc_hour, e.model${compact ? '' : ', e.session_id'}
      ORDER BY utc_hour ASC
    `)).getRowObjects() as Array<{
      utc_hour: string
      model: string | null
      session_id: string
      user_msgs: bigint | number
      assistant_msgs: bigint | number
      model_tokens: bigint | number
    }>

    // compact mode: exact distinct-session count per LOCAL day (tz shift
    // applied in SQL). ≤ a few hundred tiny rows even for range=all.
    // getTimezoneOffset() is UTC−local, so local = timestamp − offset.
    let dailySessions: Array<{ date: string; sessions: number }> | undefined
    if (compact) {
      const dailyRows = (await db.runAndReadAll(`
        SELECT
          strftime(e.timestamp::TIMESTAMP - INTERVAL '${tzOffsetMin} minutes', '%Y-%m-%d') as local_day,
          COUNT(DISTINCT e.session_id) as sessions
        ${joinClause} ${where}
        GROUP BY local_day
        ORDER BY local_day ASC
      `)).getRowObjects() as Array<{ local_day: string; sessions: bigint | number }>
      dailySessions = dailyRows.map(r => ({ date: r.local_day, sessions: Number(r.sessions ?? 0) }))
    }

    const payload = {
      range,
      sessions: Number(counters?.sessions ?? 0),
      userMessages: Number(counters?.user_messages ?? 0),
      assistantMessages: Number(counters?.assistant_messages ?? 0),
      // Renamed: was "totalTokens" (cumulative API throughput), now per-
      // session Context sum to match the sessions modal column.
      totalTokens: sessionTokens,
      favoriteModel,
      models,
      // Raw hourly buckets — client groups locally for heatmap (with rich
      // per-day tooltip), peak hour, streaks, daily models timeseries.
      hourly: hourly.map(r => ({
        utcHour: r.utc_hour,
        model: r.model,
        ...(compact ? {} : { sessionId: r.session_id }),
        userMessages: Number(r.user_msgs ?? 0),
        assistantMessages: Number(r.assistant_msgs ?? 0),
        tokens: Number(r.model_tokens ?? 0),
      })),
      ...(dailySessions ? { dailySessions } : {}),
    }
    overviewCache.set(cacheKey, { at: Date.now(), data: payload })
    return c.json(payload)
  })

  // ── GET /api/dashboard/tokens/:tokenId/sessions
  //    Per-token sessions list — JOINed against session_tokens.deleted=0
  //    so the list stays consistent with the Sessions counter on the
  //    overview endpoint and with the client's savedSessions sidebar.
  //    Each row carries enough metadata to render a card: human title
  //    (CLI's `summary` entry, or first user message), counts of user /
  //    assistant / compact (summary) entries, total tokens, model.
  api.get('/api/dashboard/tokens/:tokenId/sessions', async (c) => {
    const denied = denyForeignSubject(c, c.req.param('tokenId')); if (denied) return denied
    const { getDb } = await import('../../stats/database/lifecycle')
    const { sweepAll } = await import('../../stats/jsonl-sweeper')
    sweepThrottled(sweepAll)
    const db = await getDb()
    const tokenId = c.req.param('tokenId')
    // Live PTY sessions hold the real cwd in memory — use them as a
    // fallback for historical session_tokens rows that pre-date the
    // `cwd` column migration.
    const liveCwdByConvId = new Map<string, string>()
    for (const ps of getAllSessions()) {
      if (ps.cliConversationId && ps.cwd) liveCwdByConvId.set(ps.cliConversationId, ps.cwd)
    }
    const rows = (await db.runAndReadAll(`
      SELECT
        e.session_id,
        st.cwd as session_cwd,
        st.title as session_title,
        st.last_context as session_last_context,
        st.compact_boundaries as session_compact_boundaries,
        st.context_tokens as session_context_tokens,
        st.pre_boundary_snapshots as session_pre_boundary_snapshots,
        MIN(e.timestamp) as started_at,
        MAX(e.timestamp) as last_activity_at,
        SUM(CASE WHEN e.type = 'user' THEN 1 ELSE 0 END) as user_messages,
        -- One assistant TURN can be split across multiple JSONL rows that
        -- share message.id (e.g. one row for the thinking block and another
        -- for the text). Count distinct message_ids so the dashboard
        -- counter matches what the chat viewer actually renders (which
        -- merges these via mergeConsecutiveCliEntries).
        COUNT(DISTINCT CASE WHEN e.type = 'assistant' THEN e.message_id END) as assistant_messages,
        -- Token breakdown so the table can split a misleading "total"
        -- (cumulative input + cache flowing through API) into in / cache / out.
        SUM(CASE WHEN e.type = 'assistant' THEN COALESCE(e.input_tokens,0) ELSE 0 END) as in_tokens,
        SUM(CASE WHEN e.type = 'assistant' THEN COALESCE(e.cache_read_tokens,0) ELSE 0 END) as cache_read_tokens,
        SUM(CASE WHEN e.type = 'assistant' THEN COALESCE(e.cache_write_tokens,0) ELSE 0 END) as cache_write_tokens,
        SUM(CASE WHEN e.type = 'assistant' THEN COALESCE(e.output_tokens,0) ELSE 0 END) as out_tokens,
        -- Final context window pressure (matches the per-session progress bar
        -- in the client). This is a snapshot, NOT cumulative.
        (
          SELECT COALESCE(a.input_tokens,0) + COALESCE(a.cache_read_tokens,0) + COALESCE(a.cache_write_tokens,0)
          FROM jsonl_events a
          WHERE a.session_id = e.session_id AND a.type = 'assistant'
          ORDER BY a.timestamp DESC LIMIT 1
        ) as last_context,
        (
          SELECT a.model FROM jsonl_events a
          WHERE a.session_id = e.session_id AND a.type = 'assistant'
            AND a.model IS NOT NULL AND a.model != ''
          ORDER BY a.timestamp DESC LIMIT 1
        ) as model,
        (
          SELECT o.file_path FROM jsonl_offsets o WHERE o.file_path LIKE '%/' || e.session_id || '.jsonl' LIMIT 1
        ) as file_path
      FROM jsonl_events e
      INNER JOIN session_tokens st
        ON st.session_id = e.session_id AND st.deleted = 0
      WHERE st.user_name = ?
      GROUP BY e.session_id, st.cwd, st.title, st.last_context,
               st.compact_boundaries, st.context_tokens, st.pre_boundary_snapshots
      ORDER BY last_activity_at DESC
    `, [tokenId])).getRowObjects() as Array<any>

    // All metadata (title, compact_boundaries, last_context,
    // pre_boundary_snapshots, context_tokens) is persisted in
    // session_tokens by the sweeper as JSONL is read incrementally.
    // Endpoint is PURE SQL — no file IO at request time. Download
    // endpoint (`/sessions/:id/jsonl`) handles missing-file 404 itself.

    function folderFromPath(filePath: string | null): string | null {
      if (!filePath) return null
      const slug = path.basename(path.dirname(filePath))
      const uuidRe = /[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}/g
      const matches = slug.match(uuidRe) ?? []
      if (matches.length === 0) return null
      const idx = slug.indexOf(matches[0]!) + matches[0]!.length
      let tail = slug.slice(idx).replace(/^-/, '')
      if (matches.length >= 2) {
        const sessionUuid = matches[matches.length - 1]!
        const sIdx = tail.indexOf(sessionUuid)
        if (sIdx >= 0) tail = tail.slice(0, sIdx).replace(/-+$/, '')
      }
      return tail || null
    }

    const sessions = rows.map(r => ({
      sessionId: r.session_id,
      title: r.session_title,
      folder: r.session_cwd || liveCwdByConvId.get(r.session_id) || folderFromPath(r.file_path),
      startedAt: r.started_at,
      lastActivityAt: r.last_activity_at,
      userMessages: Number(r.user_messages ?? 0),
      assistantMessages: Number(r.assistant_messages ?? 0),
      compactCount: Number(r.session_compact_boundaries ?? 0),
      inTokens: Number(r.in_tokens ?? 0),
      cacheReadTokens: Number(r.cache_read_tokens ?? 0),
      cacheWriteTokens: Number(r.cache_write_tokens ?? 0),
      outTokens: Number(r.out_tokens ?? 0),
      contextTokens: Number(r.session_context_tokens ?? r.session_last_context ?? 0),
      model: r.model,
      jsonlPath: r.file_path,
      // We optimistically claim availability when the offset table has
      // a path entry. If the file was rm'd externally, the download
      // endpoint returns 404 — UI will gracefully handle that.
      jsonlAvailable: !!r.file_path,
    }))
    return c.json({ tokenId, sessions })
  })

  // ── GET /api/dashboard/sessions/:sessionId/jsonl
  //    Stream the raw JSONL for a session if the file is still on disk.
  api.get('/api/dashboard/sessions/:sessionId/jsonl', async (c) => {
    const sessionId = c.req.param('sessionId')
    const denied = denyForeignUser(c, await sessionOwner(sessionId)); if (denied) return denied
    const { getDb } = await import('../../stats/database/lifecycle')
    const fs = await import('fs')
    const db = await getDb()
    const row = (await db.runAndReadAll(
      `SELECT file_path FROM jsonl_offsets WHERE file_path LIKE '%/' || ? || '.jsonl' LIMIT 1`,
      [sessionId],
    )).getRowObjects()[0] as { file_path: string } | undefined
    if (!row?.file_path || !fs.existsSync(row.file_path)) {
      return c.json({ error: 'JSONL not available — file deleted' }, 404)
    }
    const content = fs.readFileSync(row.file_path, 'utf-8')
    c.header('Content-Type', 'application/x-ndjson')
    c.header('Content-Disposition', `attachment; filename="${sessionId}.jsonl"`)
    return c.body(content)
  })

  // ── DELETE /api/dashboard/sessions/:sessionId?rmFile=1
  //    Soft-delete the session (drops it from Stats grid, sessions list,
  //    and Sessions counter without losing jsonl_events history). With
  //    `?rmFile=1` also unlinks the underlying JSONL file from disk.
  api.delete('/api/dashboard/sessions/:sessionId', async (c) => {
    const sessionId = c.req.param('sessionId')
    const denied = denyForeignUser(c, await sessionOwner(sessionId)); if (denied) return denied
    const { getDb } = await import('../../stats/database/lifecycle')
    const fs = await import('fs')
    const db = await getDb()
    const rmFile = c.req.query('rmFile') === '1'

    // DuckDB run() doesn't return row counts — count first to detect
    // whether the soft-delete actually flipped anything.
    const beforeRow = (await db.runAndReadAll(
      `SELECT COUNT(*) AS n FROM session_tokens WHERE session_id = ? AND deleted = 0`,
      [sessionId],
    )).getRowObjects()[0] as { n: bigint | number } | undefined
    const matched = Number(beforeRow?.n ?? 0)
    if (matched > 0) {
      await db.run(`UPDATE session_tokens SET deleted = 1 WHERE session_id = ?`, [sessionId])
    }

    let fileRemoved = false
    if (rmFile) {
      const row = (await db.runAndReadAll(
        `SELECT file_path FROM jsonl_offsets WHERE file_path LIKE '%/' || ? || '.jsonl' LIMIT 1`,
        [sessionId],
      )).getRowObjects()[0] as { file_path: string } | undefined
      if (row?.file_path && fs.existsSync(row.file_path)) {
        try { fs.unlinkSync(row.file_path); fileRemoved = true } catch { /* ignore */ }
      }
    }
    return c.json({ ok: true, softDeleted: matched > 0, fileRemoved })
  })

  // ── GET /api/dashboard/errors/count — total + last 24h count
  api.get('/api/dashboard/errors/count', async (c) => {
    const denied = denyApiToken(c); if (denied) return denied
    const { getDb } = await import('../../stats/database/lifecycle')
    const db = await getDb()
    const since = new Date(Date.now() - 86_400_000).toISOString()
    const row = (await db.runAndReadAll(
      `SELECT
         COUNT(*) AS total,
         SUM(CASE WHEN timestamp >= ? THEN 1 ELSE 0 END) AS last24h,
         MAX(timestamp) AS last_at
       FROM api_errors`,
      [since],
    )).getRowObjects()[0] as any
    return c.json({
      total: Number(row?.total ?? 0),
      last24h: Number(row?.last24h ?? 0),
      lastAt: row?.last_at ?? null,
    })
  })

  // ── GET /api/dashboard/errors/timeseries?period=hour|day&days=N
  // Returns per-period error counts for the chart in the errors modal.
  // Same shape as /stats/timeseries but no per-user split — just count
  // bucketed by UTC hour/day so the client can re-bucket into local TZ.
  api.get('/api/dashboard/errors/timeseries', async (c) => {
    const denied = denyApiToken(c); if (denied) return denied
    const { getDb } = await import('../../stats/database/lifecycle')
    const db = await getDb()
    const period = (c.req.query('period') || 'day') as 'hour' | 'day'
    const days = parseInt(c.req.query('days') || '7', 10)
    const sinceIso = new Date(Date.now() - days * 86_400_000).toISOString()
    const groupCol = period === 'hour'
      ? `strftime(timestamp::TIMESTAMP, '%Y-%m-%d') || ' ' || strftime(timestamp::TIMESTAMP, '%H')`
      : `strftime(timestamp::TIMESTAMP, '%Y-%m-%d')`
    const rows = (await db.runAndReadAll(
      `SELECT ${groupCol} AS period, COUNT(*) AS cnt,
        SUM(CASE WHEN error_type = 'ConnectionRefused' THEN 1 ELSE 0 END) AS connection_refused,
        SUM(CASE WHEN error_type LIKE '%timeout%' OR error_type LIKE '%Timeout%' THEN 1 ELSE 0 END) AS timeouts,
        SUM(CASE WHEN error_type IN ('429') OR error_type LIKE '%rate%' THEN 1 ELSE 0 END) AS rate_limited
       FROM api_errors
       WHERE timestamp >= ?
       GROUP BY period
       ORDER BY period ASC`,
      [sinceIso],
    )).getRowObjects() as any[]
    return c.json(rows.map(r => ({
      period: String(r.period),
      count: Number(r.cnt ?? 0),
      connectionRefused: Number(r.connection_refused ?? 0),
      timeouts: Number(r.timeouts ?? 0),
      rateLimited: Number(r.rate_limited ?? 0),
    })))
  })

  // ── GET /api/dashboard/errors/recent?limit=50 — most recent errors
  api.get('/api/dashboard/errors/recent', async (c) => {
    const denied = denyApiToken(c); if (denied) return denied
    const { getDb } = await import('../../stats/database/lifecycle')
    const db = await getDb()
    const limit = Math.min(500, parseInt(c.req.query('limit') || '50', 10))
    const rows = (await db.runAndReadAll(
      `SELECT timestamp, session_id, user_name, error_type, url, attempt, max_attempts, retry_ms, raw
       FROM api_errors
       ORDER BY timestamp DESC LIMIT ?`,
      [limit],
    )).getRowObjects() as any[]
    return c.json(rows.map(r => ({
      timestamp: String(r.timestamp),
      sessionId: r.session_id,
      userName: r.user_name,
      errorType: r.error_type,
      url: r.url,
      attempt: Number(r.attempt ?? 0),
      maxAttempts: Number(r.max_attempts ?? 0),
      retryMs: Number(r.retry_ms ?? 0),
      raw: r.raw,
    })))
  })
}

// ============================================================================
// Dashboard users endpoint — per-user aggregated statistics
// ============================================================================

import type { Hono } from 'hono'
import { getDb } from '../../stats/database/lifecycle'
import { getAllSessions } from '../../pty/session-manager'
import { denyApiToken, denyForeignUser } from '../authz'

// Anthropic per-1M-token rates by model family. Mirrors
// electron/renderer/utils/session-cost.ts so server-side cost ≈ client.
interface CostTier { input: number; output: number; cacheWrite: number; cacheRead: number }
const TIER_3_15: CostTier = { input: 3, output: 15, cacheWrite: 3.75, cacheRead: 0.3 }
const TIER_15_75: CostTier = { input: 15, output: 75, cacheWrite: 18.75, cacheRead: 1.5 }
const TIER_5_25: CostTier = { input: 5, output: 25, cacheWrite: 6.25, cacheRead: 0.5 }
const TIER_HAIKU: CostTier = { input: 1, output: 5, cacheWrite: 1.25, cacheRead: 0.1 }
function tierForModel(model: string | null | undefined): CostTier {
  const m = (model || '').toLowerCase()
  if (m.includes('haiku')) return TIER_HAIKU
  // Opus 5 — текущий дефолт (тариф линейки 4.5+).
  if (m.includes('opus-5')) return TIER_5_25
  if (m.includes('opus-4')) {
    // 4.x остаётся ТОЛЬКО для стоимости старых логов (из пикера выпилен).
    if (m.includes('opus-4-5') || m.includes('opus-4-6') || m.includes('opus-4-7') || m.includes('opus-4-8')) return TIER_5_25
    return TIER_15_75
  }
  if (m.includes('sonnet')) return TIER_3_15
  return TIER_3_15
}

export function registerUserRoutes(api: Hono): void {
  // GET /api/dashboard/users — aggregated per-user summary.
  //
  // Source: jsonl_events JOIN session_tokens(deleted=0) — same model as
  // Stats overview. Active PTY session count is layered in from the live
  // session manager. No legacy `requests` / OTel reads — those are
  // unpopulated in PTY mode and were producing inconsistent numbers.
  api.get('/api/dashboard/users', async (c) => {
    // Cross-user roster — admin only; a per-user token can't enumerate everyone.
    const denied = denyApiToken(c); if (denied) return denied
    const db = await getDb()

    // Per-user totals (no model split needed for these sums).
    const aggRowsRaw = (await db.runAndReadAll(`
      SELECT
        st.user_name as user_name,
        COUNT(DISTINCT e.session_id) as sessions,
        SUM(CASE WHEN e.type = 'user' THEN 1 ELSE 0 END) as user_inputs,
        SUM(CASE WHEN e.type = 'assistant'
              THEN COALESCE(e.input_tokens,0) ELSE 0 END) as input_uncached,
        SUM(CASE WHEN e.type = 'assistant'
              THEN COALESCE(e.output_tokens,0) ELSE 0 END) as output_tokens,
        SUM(CASE WHEN e.type = 'assistant'
              THEN COALESCE(e.cache_read_tokens,0) ELSE 0 END) as cache_read,
        SUM(CASE WHEN e.type = 'assistant'
              THEN COALESCE(e.cache_write_tokens,0) ELSE 0 END) as cache_creation,
        MIN(e.timestamp) as first_seen,
        MAX(e.timestamp) as last_seen
      FROM jsonl_events e
      INNER JOIN session_tokens st
        ON st.session_id = e.session_id AND st.deleted = 0
      WHERE st.user_name IS NOT NULL AND st.user_name != ''
        AND (e.model IS NULL OR e.model != '<synthetic>')
      GROUP BY st.user_name
    `)).getRowObjects() as Array<{
      user_name: string
      sessions: bigint | number
      user_inputs: bigint | number
      input_uncached: bigint | number
      output_tokens: bigint | number
      cache_read: bigint | number
      cache_creation: bigint | number
      first_seen: string | null
      last_seen: string | null
    }>
    const aggRows = aggRowsRaw.map(r => ({
      user_name: r.user_name,
      sessions: Number(r.sessions ?? 0),
      user_inputs: Number(r.user_inputs ?? 0),
      input_uncached: Number(r.input_uncached ?? 0),
      output_tokens: Number(r.output_tokens ?? 0),
      cache_read: Number(r.cache_read ?? 0),
      cache_creation: Number(r.cache_creation ?? 0),
      first_seen: r.first_seen,
      last_seen: r.last_seen,
    }))

    const aggMap = new Map(aggRows.map(r => [r.user_name, r]))

    // Per-(user, model) breakdown for cost computation — every model
    // family has its own per-1M-token rates.
    const costRows = (await db.runAndReadAll(`
      SELECT
        st.user_name as user_name,
        e.model as model,
        SUM(COALESCE(e.input_tokens,0)) as input_tokens,
        SUM(COALESCE(e.output_tokens,0)) as output_tokens,
        SUM(COALESCE(e.cache_read_tokens,0)) as cache_read_tokens,
        SUM(COALESCE(e.cache_write_tokens,0)) as cache_write_tokens
      FROM jsonl_events e
      INNER JOIN session_tokens st
        ON st.session_id = e.session_id AND st.deleted = 0
      WHERE st.user_name IS NOT NULL AND st.user_name != ''
        AND e.type = 'assistant' AND e.model IS NOT NULL AND e.model != '' AND e.model != '<synthetic>'
      GROUP BY st.user_name, e.model
    `)).getRowObjects() as Array<{
      user_name: string
      model: string
      input_tokens: bigint | number
      output_tokens: bigint | number
      cache_read_tokens: bigint | number
      cache_write_tokens: bigint | number
    }>
    const costByUser = new Map<string, number>()
    for (const r of costRows) {
      const tier = tierForModel(r.model)
      const usd = (
        Number(r.input_tokens ?? 0) * tier.input
        + Number(r.output_tokens ?? 0) * tier.output
        + Number(r.cache_read_tokens ?? 0) * tier.cacheRead
        + Number(r.cache_write_tokens ?? 0) * tier.cacheWrite
      ) / 1_000_000
      costByUser.set(r.user_name, (costByUser.get(r.user_name) ?? 0) + usd)
    }

    // Sum of per-session real-text Context (last-assistant effective
    // input). Matches the per-session Context column in Sessions modal.
    const ctxRows = (await db.runAndReadAll(`
      SELECT st.user_name as user_name, SUM(last_ctx) as ctx
      FROM (
        SELECT
          e.session_id,
          (
            SELECT COALESCE(a.input_tokens,0) + COALESCE(a.cache_read_tokens,0) + COALESCE(a.cache_write_tokens,0)
            FROM jsonl_events a
            WHERE a.session_id = e.session_id AND a.type = 'assistant'
              AND (a.model IS NULL OR a.model != '<synthetic>')
            ORDER BY a.timestamp DESC LIMIT 1
          ) as last_ctx
        FROM (SELECT DISTINCT session_id FROM jsonl_events) e
      ) sessCtx
      INNER JOIN session_tokens st ON st.session_id = sessCtx.session_id AND st.deleted = 0
      WHERE st.user_name IS NOT NULL AND st.user_name != ''
      GROUP BY st.user_name
    `)).getRowObjects() as Array<{ user_name: string; ctx: bigint | number | null }>
    const ctxByUser = new Map(ctxRows.map(r => [r.user_name, Number(r.ctx ?? 0)]))

    const ptySessions = getAllSessions()
    const activeSessionsByUser = new Map<string, number>()
    for (const s of ptySessions) {
      activeSessionsByUser.set(s.userName, (activeSessionsByUser.get(s.userName) || 0) + 1)
    }

    const allUsers = new Set<string>()
    for (const r of aggRows) allUsers.add(r.user_name)
    for (const s of ptySessions) if (s.userName) allUsers.add(s.userName)

    const users = [...allUsers].map((userName) => {
      const r = aggMap.get(userName)
      return {
        userName,
        activeSessions: activeSessionsByUser.get(userName) || 0,
        totalInputs: r?.user_inputs ?? 0,
        tokens: {
          input: r?.input_uncached ?? 0,
          output: r?.output_tokens ?? 0,
          cacheRead: r?.cache_read ?? 0,
          cacheCreation: r?.cache_creation ?? 0,
        },
        // Sum of per-session real-text Context (matches Sessions modal).
        contextTokens: ctxByUser.get(userName) ?? 0,
        // USD cost summed across model tiers.
        cost: costByUser.get(userName) ?? 0,
        firstSeen: r?.first_seen ?? null,
        lastSeen: r?.last_seen ?? null,
      }
    }).sort((a, b) => {
      if (a.activeSessions !== b.activeSessions) return b.activeSessions - a.activeSessions
      if (a.lastSeen && b.lastSeen) return b.lastSeen.localeCompare(a.lastSeen)
      return 0
    })

    return c.json(users)
  })

  // GET /api/dashboard/users/:userName/requests — recent requests for a user
  api.get('/api/dashboard/users/:userName/requests', async (c) => {
    const userName = c.req.param('userName')
    const denied = denyForeignUser(c, userName); if (denied) return denied
    const limit = parseInt(c.req.query('limit') || '50', 10)
    const db = await getDb()

    const rows = (await db.runAndReadAll(`
      SELECT * FROM requests
      WHERE user_name = ?
      ORDER BY timestamp DESC
      LIMIT ?
    `, [userName, limit])).getRowObjects() as Array<Record<string, unknown>>

    const entries = rows.map((row) => ({
      id: row.id,
      timestamp: row.timestamp,
      endpoint: row.endpoint,
      method: row.method || 'POST',
      model: row.model || '',
      userName: row.user_name,
      durationMs: Number(row.duration_ms ?? 0),
      inputTokens: Number(row.input_tokens ?? 0),
      outputTokens: Number(row.output_tokens ?? 0),
      cacheReadTokens: Number(row.cache_read_tokens ?? 0),
      cacheWriteTokens: Number(row.cache_write_tokens ?? 0),
      toolsUsed: safeParseArray(row.tools_used as string),
      status: row.status || 'success',
      statusCode: Number(row.status_code ?? 200),
      error: row.error || undefined,
      isUserRequest: row.is_user_request === 1 || row.is_user_request === 1n,
      userMessage: row.user_message || undefined,
      sessionKey: row.session_key || undefined,
    }))

    return c.json(entries)
  })
}

function safeParseArray(str: string | null | undefined): string[] {
  if (!str) return []
  try { return JSON.parse(str) } catch { return [] }
}

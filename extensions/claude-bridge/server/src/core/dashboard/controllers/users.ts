// ============================================================================
// Dashboard users endpoint — per-user aggregated statistics
// ============================================================================

import type { Hono } from 'hono'
import { getDb } from '../../stats/database/lifecycle'
import { userContextTokens, userCostRows, userTotals, type UserTotalsRow } from '../../stats/database/stats-queries'
import { createRevisionCache } from '../../stats/database/revision-cache'
import { getAllSessions } from '../../pty/session-manager'
import { denyApiToken, denyForeignUser } from '../authz'

// Anthropic published base rates per 1M tokens. Keep aligned with the
// KaminIDE webview's session-cost helper so dashboard cost ≈ client.
interface CostTier {
  input: number
  output: number
  cacheWrite: number
  cacheRead: number
}
const TIER_3_15: CostTier = { input: 3, output: 15, cacheWrite: 3.75, cacheRead: 0.3 }
const TIER_2_10: CostTier = { input: 2, output: 10, cacheWrite: 2.5, cacheRead: 0.2 }
const TIER_15_75: CostTier = { input: 15, output: 75, cacheWrite: 18.75, cacheRead: 1.5 }
const TIER_5_25: CostTier = { input: 5, output: 25, cacheWrite: 6.25, cacheRead: 0.5 }
const TIER_4_20: CostTier = { input: 4, output: 20, cacheWrite: 5, cacheRead: 0.2 }
const TIER_FABLE_5_1: CostTier = { input: 10, output: 50, cacheWrite: 12.5, cacheRead: 0.25 }
// Standard speed. Fast-mode turns cost 2× but the accounting rows carry no
// speed, so the dashboard keeps the standard rate (the webview applies 2×).
const TIER_FABLE_5_2: CostTier = { input: 8, output: 40, cacheWrite: 10, cacheRead: 0.2 }
const TIER_HAIKU: CostTier = { input: 1, output: 5, cacheWrite: 1.25, cacheRead: 0.1 }
// Haiku 5.5 up to a 100K-token prompt. Prompts above 100K cost 5× more, but the
// dashboard sums tokens per model, not per request, so it uses the base rate
// (the webview estimate applies the long-prompt rate per request).
const TIER_HAIKU_5_5: CostTier = { input: 0.1, output: 0.5, cacheWrite: 0.125, cacheRead: 0.01 }
const usersCache = createRevisionCache<{
  aggRows: UserTotalsRow[]
  costByUser: Map<string, number>
  ctxByUser: Map<string, number>
}>({ max: 1, maxStaleMs: 60_000 })

async function loadUserStats() {
  const db = await getDb()
  // Same formulas as the former raw jsonl_events scans.
  const aggRows = await userTotals(db)
  // Per-(user, model) breakdown for cost computation — every model
  // family has its own per-1M-token rates.
  const costByUser = new Map<string, number>()
  for (const r of await userCostRows(db)) {
    const base = tierForModel(r.model)
    const multiplier = r.model.includes('haiku-5') && r.long_prompt ? 5 : 1
    const tier = {
      input: base.input * multiplier,
      output: base.output * multiplier,
      cacheRead: base.cacheRead * multiplier,
      cacheWrite: base.cacheWrite * multiplier,
    }
    const usd =
      (r.input_tokens * tier.input +
        r.output_tokens * tier.output +
        r.cache_read_tokens * tier.cacheRead +
        r.cache_write_tokens * tier.cacheWrite +
        r.cache_write_1h_tokens * (2 * tier.input - tier.cacheWrite)) /
      1_000_000
    costByUser.set(r.user_name, (costByUser.get(r.user_name) ?? 0) + usd)
  }
  // Sum of per-session real-text Context (last-assistant effective
  // input). Matches the per-session Context column in Sessions modal.
  const ctxByUser = await userContextTokens(db)
  return { aggRows, costByUser, ctxByUser }
}

export function tierForModel(model: string | null | undefined): CostTier {
  const m = (model || '').toLowerCase()
  if (m.includes('haiku-5')) return TIER_HAIKU_5_5
  if (m.includes('haiku')) return TIER_HAIKU
  if (m.includes('fable-5-2')) return TIER_FABLE_5_2
  if (m.includes('fable-5-1')) return TIER_FABLE_5_1
  if (m.includes('opus-5-5')) return TIER_4_20
  // Opus 5 and legacy 4.5+ use the previous rate.
  if (m.includes('opus-5')) return TIER_5_25
  if (m.includes('opus-4')) {
    // 4.x остаётся ТОЛЬКО для стоимости старых логов (из пикера выпилен).
    if (m.includes('opus-4-5') || m.includes('opus-4-6') || m.includes('opus-4-7') || m.includes('opus-4-8'))
      return TIER_5_25
    return TIER_15_75
  }
  if (m.includes('sonnet-5-5')) return { ...TIER_2_10, cacheRead: 0.1 }
  if (m.includes('sonnet-5')) return TIER_2_10
  return TIER_3_15
}

export function registerUserRoutes(api: Hono): void {
  // GET /api/dashboard/users — aggregated per-user summary.
  //
  // Source: the stats rollup of live sessions (session_tokens.deleted = 0),
  // same model as Stats overview. Active PTY session count is layered in from the live
  // session manager. No legacy `requests` / OTel reads — those are
  // unpopulated in PTY mode and were producing inconsistent numbers.
  api.get('/api/dashboard/users', async (c) => {
    // Cross-user roster — admin only; a per-user token can't enumerate everyone.
    const denied = denyApiToken(c)
    if (denied) return denied
    // Rollup read model (stats-queries.ts), served stale-while-revalidate
    // across rollup revisions; live PTY counts are layered on per request.
    const { rollupRevision } = await import('../../stats/database/rollup')
    const { aggRows, costByUser, ctxByUser } = (await usersCache.get('users', rollupRevision(), loadUserStats)).data
    const aggMap = new Map(aggRows.map((r) => [r.user_name, r]))
    const ptySessions = getAllSessions()
    const activeSessionsByUser = new Map<string, number>()
    for (const s of ptySessions) {
      activeSessionsByUser.set(s.userName, (activeSessionsByUser.get(s.userName) || 0) + 1)
    }

    const allUsers = new Set<string>()
    for (const r of aggRows) allUsers.add(r.user_name)
    for (const s of ptySessions) if (s.userName) allUsers.add(s.userName)

    const users = [...allUsers]
      .map((userName) => {
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
          // Token flow (in + out + cache) per rolling window, for sidebar sorting.
          periodTokens: { d7: r?.tokens_7d ?? 0, d30: r?.tokens_30d ?? 0 },
        }
      })
      .sort((a, b) => {
        if (a.activeSessions !== b.activeSessions) return b.activeSessions - a.activeSessions
        if (a.lastSeen && b.lastSeen) return b.lastSeen.localeCompare(a.lastSeen)
        return 0
      })

    return c.json(users)
  })

  // GET /api/dashboard/users/:userName/requests — recent requests for a user
  api.get('/api/dashboard/users/:userName/requests', async (c) => {
    const userName = c.req.param('userName')
    const denied = denyForeignUser(c, userName)
    if (denied) return denied
    const limit = parseInt(c.req.query('limit') || '50', 10)
    const db = await getDb()

    const rows = (
      await db.runAndReadAll(
        `
      SELECT * FROM requests
      WHERE user_name = ?
      ORDER BY timestamp DESC
      LIMIT ?
    `,
        [userName, limit],
      )
    ).getRowObjects() as Array<Record<string, unknown>>

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
  try {
    return JSON.parse(str)
  } catch {
    return []
  }
}

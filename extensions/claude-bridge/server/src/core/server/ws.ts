import { WebSocketServer, WebSocket as WS } from 'ws'
import type { Server as HttpServer } from 'http'
import type { IncomingMessage } from 'http'
import type { Duplex } from 'stream'
import { eventBus } from '../events/bus'
import { getTerminalSessionCount } from './terminal'
import { getAllSessions } from '../pty/session-manager'
import { getDb } from '../stats/database/lifecycle'

/** Per-session live aggregates from jsonl_events — counts ONLY entries
 *  produced after each PTY session's `createdAt`. That way "Active PTY
 *  Sessions" table reflects what the user did THIS session (not the
 *  full conversation history of a /resume'd chat). Context column is
 *  still the absolute last-turn pressure (it's a snapshot, not a delta).
 */
async function getPtyAggregates(sessions: Array<{ id: string; conversationId: string; createdAt: string }>): Promise<Map<string, {
  userMessages: number
  assistantMessages: number
  totalTokens: number
  contextTokens: number
  model: string | null
}>> {
  const out = new Map<string, { userMessages: number; assistantMessages: number; totalTokens: number; contextTokens: number; model: string | null }>()
  if (sessions.length === 0) return out
  const db = await getDb()
  // Context = PEAK effective input (input + cache_read + cache_write)
  // across all assistant turns in this session — same metric as the
  // client's SessionStats.tsx progress bar. Skips <synthetic> rows.
  // Note: doesn't slice by compact_boundary (system entries aren't
  // stored in jsonl_events) — peak is across the whole session.
  // Context query walks the resume chain via session_tokens.parent_session_id.
  // After a session is resumed (`--resume`), CLI writes to a NEW jsonl
  // file, so a direct MAX over `jsonl_events WHERE session_id = current`
  // returns 0 until the user actually types something — the dashboard
  // would then show "Context 0" for any idle resumed session.
  // Walking the parent chain via session_tokens.context_tokens (which
  // sweeper persists per-session) lets us surface the LAST known
  // context from the previous segment until a fresh assistant turn
  // overwrites it. Falls back to a direct jsonl_events MAX so brand-
  // new sessions still report their live peak before sweeper has run.
  const sql = `
    SELECT
      SUM(CASE WHEN type = 'user' AND timestamp >= ? THEN 1 ELSE 0 END) as user_messages,
      SUM(CASE WHEN type = 'assistant' AND timestamp >= ? THEN 1 ELSE 0 END) as assistant_messages,
      SUM(CASE WHEN timestamp >= ? THEN COALESCE(input_tokens,0) + COALESCE(cache_read_tokens,0) + COALESCE(cache_write_tokens,0) + COALESCE(output_tokens,0) ELSE 0 END) as total_tokens,
      GREATEST(
        COALESCE((
          SELECT MAX(COALESCE(a.input_tokens,0) + COALESCE(a.cache_read_tokens,0) + COALESCE(a.cache_write_tokens,0))
          FROM jsonl_events a
          WHERE a.session_id = ? AND a.type = 'assistant'
            AND (a.model IS NULL OR a.model != '<synthetic>')
        ), 0),
        COALESCE((
          WITH RECURSIVE chain AS (
            SELECT session_id, parent_session_id, context_tokens
            FROM session_tokens WHERE session_id = ?
            UNION ALL
            SELECT st.session_id, st.parent_session_id, st.context_tokens
            FROM session_tokens st
            JOIN chain c ON c.parent_session_id = st.session_id
          )
          SELECT MAX(context_tokens) FROM chain
        ), 0)
      ) as context_tokens,
      (
        SELECT a.model FROM jsonl_events a
        WHERE a.session_id = ? AND a.type = 'assistant'
          AND a.model IS NOT NULL AND a.model != ''
        ORDER BY a.timestamp DESC LIMIT 1
      ) as model
    FROM jsonl_events
    WHERE session_id = ?
  `
  for (const s of sessions) {
    try {
      const rows = (await db.runAndReadAll(sql, [
        s.createdAt, s.createdAt, s.createdAt,
        s.conversationId, s.conversationId, s.conversationId, s.conversationId,
      ])).getRowObjects() as Array<{
        user_messages: bigint | number | null
        assistant_messages: bigint | number | null
        total_tokens: bigint | number | null
        context_tokens: bigint | number | null
        model: string | null
      }>
      const r = rows[0]
      if (r) {
        out.set(s.conversationId, {
          userMessages: Number(r.user_messages ?? 0),
          assistantMessages: Number(r.assistant_messages ?? 0),
          totalTokens: Number(r.total_tokens ?? 0),
          contextTokens: Number(r.context_tokens ?? 0),
          model: r.model,
        })
      }
    } catch { /* skip session on error */ }
  }
  return out
}

// Server boot timestamp — populated once on module load. Surfaced to the
// dashboard as `stats.startedAt` so the Server status card can show
// Started + live Uptime.
const SERVER_STARTED_AT = new Date().toISOString()

// Cache latest health check result so new WS connections get it immediately
let lastHealth: unknown = null
let lastHealthAt: number | null = null
let lastAccount: unknown = null
let lastAccountAt: number | null = null
let lastUsage: unknown = null
eventBus.on('health:updated', (event) => {
  lastHealth = event.data
  lastHealthAt = Date.now()
  // Also cache account data from health updates
  const h = event.data as { account?: unknown }
  if (h?.account) { lastAccount = h.account; lastAccountAt = Date.now() }
})

eventBus.on('account:updated', (event) => {
  lastAccount = event.data
  lastAccountAt = Date.now()
})

eventBus.on('usage:updated', (event) => {
  lastUsage = event.data
})

// ---------------------------------------------------------------------------
// WebSocket route registry — multiple WSS on a single HTTP server
// ---------------------------------------------------------------------------

interface WsRoute {
  path: string
  wss: WebSocketServer
}

const wsRoutes: WsRoute[] = []

/** Register a noServer WSS for a given path. Called by attachWebSocket / attachTerminalWebSocket. */
export function registerWsRoute(path: string, wss: WebSocketServer): void {
  wsRoutes.push({ path, wss })
}

/**
 * Install the central `upgrade` handler on the HTTP server.
 * Must be called AFTER all WSS routes are registered.
 */
export function installUpgradeHandler(server: HttpServer): void {
  server.on('upgrade', (request: IncomingMessage, socket: Duplex, head: Buffer) => {
    const pathname = request.url?.split('?')[0]
    for (const route of wsRoutes) {
      if (pathname === route.path) {
        route.wss.handleUpgrade(request, socket, head, (ws) => {
          route.wss.emit('connection', ws, request)
        })
        return
      }
    }
    // No matching route — reject
    socket.destroy()
  })
}

/**
 * Attach WebSocket server for dashboard real-time updates.
 * Path: /ws/dashboard
 *
 * Events are forwarded via eventBus.setBroadcast (single JSON.stringify per event).
 * `stats:updated` is throttled to max 2 sends/sec to avoid flooding WS with
 * 50-100KB payloads on every recordX call.
 */
export function attachWebSocket(_server: HttpServer): void {
  const wss = new WebSocketServer({ noServer: true })

  // Register route
  registerWsRoute('/ws/dashboard', wss)

  wss.on('connection', (ws: WS) => {
    // Pull live aggregates (msgs/tokens/model) from jsonl_events for any
    // active PTY session whose conversation_id is already known so the
    // dashboard's Active PTY Sessions table matches the per-token list.
    // DuckDB queries are async, so wrap the init send in a self-invoking
    // async IIFE — first message goes out as soon as the aggregates resolve.
    const allSessions = getAllSessions()
    void (async () => {
      try {
        const aggregates = await getPtyAggregates(
          allSessions
            .filter(s => !!s.cliConversationId)
            .map(s => ({ id: s.id, conversationId: s.cliConversationId!, createdAt: s.createdAt.toISOString() }))
        )

        const initData = {
          stats: {
            startedAt: SERVER_STARTED_AT,
          },
          health: lastHealth,
          healthCheckedAt: lastHealthAt,
          account: lastAccount,
          accountCheckedAt: lastAccountAt,
          usage: lastUsage,
          terminalSessions: getTerminalSessionCount(),
          ptySessions: allSessions.map(s => {
            const agg = s.cliConversationId ? aggregates.get(s.cliConversationId) : undefined
            return {
              id: s.id,
              userName: s.userName,
              cwd: s.cwd,
              state: s.state,
              createdAt: s.createdAt.toISOString(),
              lastActivityAt: s.lastActivityAt.toISOString(),
              mcpCallCount: s.mcpCallCount,
              inputCount: s.inputCount,
              mcpInitialized: s.mcpInitialized,
              mcpLastError: s.mcpLastError,
              sessionTitle: s.sessionTitle ?? null,
              cliConversationId: s.cliConversationId ?? null,
              userMessages: agg?.userMessages ?? 0,
              assistantMessages: agg?.assistantMessages ?? 0,
              totalTokens: agg?.totalTokens ?? 0,
              contextTokens: agg?.contextTokens ?? 0,
              model: agg?.model ?? null,
            }
          }),
        }
        if (ws.readyState === WS.OPEN) {
          ws.send(JSON.stringify({ type: 'init', data: initData, timestamp: Date.now() }))
        }
      } catch {
        /* best-effort init payload — client survives without it */
      }
    })()

    // No per-client wildcard listener needed — broadcast handles all forwarding
    ws.on('error', () => {})
  })

  // Drop dashboard broadcasts to a client whose send buffer is already this far
  // behind — a stalled/slow dashboard socket would otherwise let bufferedAmount
  // grow unbounded (event firehose) until the server OOMs. 16 MiB is generous
  // for the small JSON events; a client that far behind gets stale data anyway.
  const MAX_DASH_BUFFERED = 16 * 1024 * 1024
  function broadcastToAll(json: string): void {
    for (const client of wss.clients) {
      if (client.readyState !== WS.OPEN) continue
      if (client.bufferedAmount > MAX_DASH_BUFFERED) continue
      try { client.send(json) } catch { /* client gone mid-iteration */ }
    }
  }

  // Set broadcast function on eventBus
  eventBus.setBroadcast((event) => {
    // No dashboard connected (the common case) → skip the JSON.stringify
    // entirely; otherwise every session/turn event serializes for nobody.
    if (wss.clients.size === 0) return
    broadcastToAll(JSON.stringify(event))
  })
}

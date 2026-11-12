import { WebSocketServer, WebSocket as WS } from 'ws'
import type { Server as HttpServer } from 'http'
import type { IncomingMessage } from 'http'
import type { Duplex } from 'stream'
import { isDashboardAuthEnabled, validateDashboardSession } from '../auth/dashboard-auth'
import { commandSnapshot, commandOutput, stopCommand } from '../pty/commands'
import { eventBus } from '../events/bus'
import { getTerminalSessionCount } from './terminal'
import { getAllSessions } from '../pty/session-manager'
import { persistedContextTokens, ptySessionCounters } from '../dashboard/pty-session-counters'
import { warnLog } from '../logging'

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
  if (h?.account) {
    lastAccount = h.account
    lastAccountAt = Date.now()
  }
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
    // The Active PTY Sessions counters are the sessions' live counters, the
    // same the polled REST list returns (dashboard/pty-session-counters.ts).
    // The persisted-context lookup is async, so the init send waits for it.
    const allSessions = getAllSessions()
    void (async () => {
      try {
        const persisted = await persistedContextTokens(allSessions)

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
          ptySessions: allSessions.map((s) => {
            return {
              id: s.id,
              userName: s.userName,
              engine: s.engine ?? 'claude',
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
              ...ptySessionCounters(s, persisted),
              model: s.model ?? null,
            }
          }),
        }
        if (ws.readyState === WS.OPEN) {
          ws.send(JSON.stringify({ type: 'init', data: initData, timestamp: Date.now() }))
        }
      } catch (err) {
        // Best effort: the client survives without the init payload.
        warnLog('dashboard ws: init payload failed', { error: String(err) })
      }
    })()

    let commandToken: string | undefined
    let subscribed = false
    let sendingCommands = false
    const allowed = async (token?: string): Promise<boolean> =>
      !isDashboardAuthEnabled() || (!!token && !!(await validateDashboardSession(token)))
    const sendCommands = async (): Promise<void> => {
      if (!subscribed || sendingCommands || ws.readyState !== WS.OPEN || ws.bufferedAmount > 1024 * 1024) return
      sendingCommands = true
      try {
        if (!(await allowed(commandToken))) {
          subscribed = false
          return
        }
        const snapshots = []
        for (const session of getAllSessions()) snapshots.push(await commandSnapshot(session))
        if (ws.readyState === WS.OPEN)
          ws.send(JSON.stringify({ type: 'commands:snapshots', data: snapshots, timestamp: Date.now() }))
      } finally {
        sendingCommands = false
      }
    }
    const commandTimer = setInterval(() => {
      void sendCommands().catch(() => {})
    }, 1000)
    commandTimer.unref?.()
    ws.once('close', () => clearInterval(commandTimer))
    ws.on('message', (raw) => {
      void (async () => {
        if (Buffer.byteLength(raw.toString()) > 8192) return
        const msg = JSON.parse(raw.toString()) as { type: string; token?: string; sessionId?: string; id?: string }
        if (!msg.type.startsWith('commands:') || !(await allowed(msg.token))) return
        if (msg.type === 'commands:subscribe') {
          commandToken = msg.token
          subscribed = true
          await sendCommands()
          return
        }
        const session = getAllSessions().find((s) => s.id === msg.sessionId)
        if (!session || typeof msg.id !== 'string' || msg.id.length > 100) return
        if (msg.type === 'commands:stop') await stopCommand(session, msg.id)
        if (msg.type === 'commands:output' && ws.readyState === WS.OPEN)
          ws.send(
            JSON.stringify({
              type: 'commands:output',
              data: { sessionId: session.id, id: msg.id, output: await commandOutput(session, msg.id) },
              timestamp: Date.now(),
            }),
          )
      })().catch(() => {})
    })

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
      try {
        client.send(json)
      } catch {
        /* client gone mid-iteration */
      }
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

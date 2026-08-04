// ============================================================================
// Open Claude Bridge — Main Entry Point (PTY mode)
// ============================================================================

import { serve } from "@hono/node-server"
import {
  createApp,
  config,
  args,
  attachWebSocket,
  installUpgradeHandler,
  attachTerminalWebSocket,
  cleanupTerminal,
  attachSessionWebSocket,
  shutdownAll,
  startSessionReaper,
  stopSessionReaper,
  startConsoleShrinkSweep,
  eventBus,
  VERSION,
  SERVICE_NAME,
} from "./core"
import type { HealthCheck } from "./core"
import { errorLog } from "./core/logging"
import { injectProxyEnv } from "./core/config/settings"

// --help
if (args.help) {
  console.log(`
${SERVICE_NAME} v${VERSION}

Usage: open-claude-bridge [options]

Options:
  -p, --port <port>       Server port (default: 3456)
  -H, --host <host>       Server host (default: 127.0.0.1)
  -d, --debug             Enable debug logging to logs/
  -v, --verbose           Enable verbose logging (all events)
  --log-level <level>     Log level: error, warn, info, debug, verbose (default: error)
  -h, --help              Show this help

Architecture:
  Claude CLI runs as PTY sessions on the server.
  MCP tools are executed on the user's machine via Electron app.
  Dashboard: http://HOST:PORT/ui

Config priority: CLI args > env vars > opencode.json > defaults

Environment variables (fallback):
  CLAUDE_PROXY_PORT           Server port
  CLAUDE_PROXY_HOST           Server host
  CLAUDE_PROXY_LOG_LEVEL      Log level (error/warn/info/debug/verbose)
  CLAUDE_PROXY_HTTP_PROXY     HTTP proxy for Claude Code
  CLAUDE_PROXY_HTTPS_PROXY    HTTPS proxy for Claude Code
  CLAUDE_PROXY_CA_CERT        Path to custom CA cert (→ NODE_EXTRA_CA_CERTS)
  DEBUG=1                     Debug logging
  VERBOSE=1                   Verbose logging

Dashboard: http://HOST:PORT/ui
`)
  process.exit(0)
}

// node-pty adds process exit listeners per PTY; raise limit to accommodate
process.setMaxListeners(50)

let server: ReturnType<typeof serve> | null = null

async function startup(): Promise<void> {
  // Create HTTP server
  const app = createApp()

  const nodeServer = serve({
    fetch: app.fetch,
    port: config.port,
    hostname: config.host,
  })

  // MCP elicitation/tool calls can legitimately hang for hours while a user
  // decides. Node's default requestTimeout (300s in 18+) kills the connection
  // mid-wait. We're a local bridge — not exposed — so disabling is safe.
  try {
    const raw = nodeServer as unknown as { requestTimeout?: number; keepAliveTimeout?: number; headersTimeout?: number; timeout?: number }
    raw.requestTimeout = 0            // no per-request timeout
    raw.keepAliveTimeout = 7_200_000  // 2h idle between requests on a socket
    raw.headersTimeout = 60_000       // headers must still arrive in 60s
    raw.timeout = 0                   // legacy socket timeout off
  } catch { /* older node-server — ignore */ }

  server = nodeServer

  // Attach WebSocket servers (noServer mode — register routes)
  attachWebSocket(nodeServer as any)           // /ws/dashboard
  attachTerminalWebSocket(nodeServer as any)    // /ws/terminal
  attachSessionWebSocket(nodeServer as any)     // /ws/session (PTY + MCP)
  // Install central upgrade handler AFTER all WS routes are registered
  installUpgradeHandler(nodeServer as any)

  // Initialize DuckDB (migrates from SQLite proxy.db on first boot).
  try {
    const { initStatsDb } = await import("./core/stats/database/lifecycle")
    await initStatsDb()
  } catch (e) {
    console.warn(`[bridge] DuckDB unavailable (${e instanceof Error ? e.message : e}), running without persistence`)
  }
  // Prime caches that need an initial DB read so sync hot paths
  // (Bearer auth, settings injection) work without awaiting.
  try {
    const { primeTokenCache } = await import("./core/auth/tokens")
    await primeTokenCache()
  } catch { /* cache will lazy-load on first call */ }
  try {
    const { primeSettingsCache } = await import("./core/config/settings")
    await primeSettingsCache()
  } catch { /* cache will lazy-load on first call */ }

  // Start the JSONL → SQLite sweeper. Catches up missed entries from
  // ~/.claude/projects/ on boot and runs every 5 min thereafter.
  try {
    const { startJsonlSweeper } = await import("./core/stats/jsonl-sweeper")
    startJsonlSweeper()
  } catch (e) {
    console.warn(`[bridge] jsonl-sweeper unavailable: ${e instanceof Error ? e.message : e}`)
  }

  // Start automated DB maintenance: daily retention (age out unread otel/error
  // rows + CHECKPOINT) and threshold-triggered compaction (reclaims the file's
  // high-water mark that historic churn left behind).
  try {
    const { startDbMaintenance } = await import("./core/stats/database/maintenance")
    startDbMaintenance()
  } catch (e) {
    console.warn(`[bridge] db-maintenance unavailable: ${e instanceof Error ? e.message : e}`)
  }

  // Pin the global ~/.claude/CLAUDE.md to the bridge default so even non-PTY
  // CLI invocations (slash commands, /init) see the right "you're in a
  // container, tools are remote" preamble instead of stale per-tenant content
  // left over by older bridge builds.
  try {
    const { ensureGlobalBridgeClaudeMd } = await import("./core/pty/session-settings")
    ensureGlobalBridgeClaudeMd()
  } catch (e) {
    console.warn(`[bridge] global CLAUDE.md init failed: ${e instanceof Error ? e.message : e}`)
  }

  const base = `http://${config.host}:${config.port}`
  console.log(`[bridge] Server running at ${base}`)
  console.log(`[bridge] Dashboard: ${base}/ui`)
  console.log(`[bridge] Mode: PTY sessions (Electron MCP)`)
  console.log(`[bridge] Tokens: Create user tokens at ${base}/ui#/tokens`)

  // Start session reaper (cleanup idle/expired sessions)
  startSessionReaper()

  // Opt-in: shrink the PTY of sessions whose console nobody is watching
  // (BRIDGE_IDLE_CONSOLE_SHRINK=1) — cuts the Ink-TUI repaint volume our output
  // hot-path processes. No-op unless the env flag is set.
  startConsoleShrinkSweep()

  // Start background refresh jobs (health, account, usage)
  import("./core/dashboard/background-jobs").then(({ startBackgroundJobs }) => {
    startBackgroundJobs()
  }).catch(() => {})

  // Check auth status with retries for slow CLI startup
  const startupHealthCheck = async (attempt: number): Promise<void> => {
    try {
      const { execFile } = await import("child_process")
      const { promisify } = await import("util")
      const execFileAsync = promisify(execFile)
      const env = { ...process.env, HOME: process.env.HOME || "/home/bridge" } as Record<string, string>
      injectProxyEnv(env)
      const { stdout } = await execFileAsync("claude", ["--dangerously-skip-permissions", "auth", "status"], {
        timeout: 10_000,
        env,
      })
      const parsed = JSON.parse(stdout.trim())
      if (!parsed.loggedIn) {
        if (attempt < 3) {
          const delay = attempt === 1 ? 10_000 : 30_000
          console.log(`[bridge] Not authenticated yet — retrying in ${delay / 1000}s (attempt ${attempt}/3)`)
          setTimeout(() => startupHealthCheck(attempt + 1), delay)
          return
        }
        console.log("[bridge] Not authenticated after 3 attempts")
        eventBus.emit("health:updated", { sdk: false, account: null, apiPing: null, error: "Not authenticated" } as HealthCheck)
        return
      }

      // Auth OK
      const account = {
        email: parsed.email || parsed.accountEmail,
        plan: parsed.plan || parsed.accountPlan,
      }
      eventBus.emit("health:updated", { sdk: true, account, apiPing: null })
      console.log(`[bridge] Claude authenticated${account.email ? ` (${account.email})` : ""}`)
    } catch (err) {
      if (attempt < 3) {
        const delay = attempt === 1 ? 10_000 : 30_000
        console.log(`[bridge] Health check failed — retrying in ${delay / 1000}s (attempt ${attempt}/3)`)
        setTimeout(() => startupHealthCheck(attempt + 1), delay)
        return
      }
      eventBus.emit("health:updated", { sdk: false, account: null, apiPing: null, error: "Health check failed" } as HealthCheck)
      console.error(`[bridge] Health check failed: ${err instanceof Error ? err.message : err}`)
    }
  }
  startupHealthCheck(1)
}

// Hard deadline for a graceful shutdown before we force-exit — see shutdown().
const SHUTDOWN_WATCHDOG_MS = 8_000
let shuttingDown = false
async function shutdown(): Promise<void> {
  // Bound to SIGINT + SIGTERM AND called from the fatal-exception path — a second
  // trigger (double signal, or a signal during an exception shutdown) must not
  // re-run the teardown (double closeDb pool drain, etc.).
  if (shuttingDown) return
  shuttingDown = true
  console.log("\n[bridge] Shutting down...")
  // Hard deadline: keepAliveTimeout is 2h, so server.close() can hang on a live
  // keep-alive socket, and a dynamic import() could stall — never let a graceful
  // shutdown wedge the process forever. .unref() so it can't keep us alive.
  setTimeout(() => { console.error("[bridge] Shutdown watchdog fired — forcing exit"); process.exit(1) }, SHUTDOWN_WATCHDOG_MS).unref()

  // Stop background jobs and session reaper
  stopSessionReaper()
  try {
    const { stopBackgroundJobs } = await import("./core/dashboard/background-jobs")
    stopBackgroundJobs()
  } catch {}

  // Kill all PTY sessions and reject pending MCP calls
  shutdownAll()

  // Clean up standalone terminal
  cleanupTerminal()

  // Close SQLite database
  try {
    const { closeDb } = await import("./core/stats/database")
    closeDb()
  } catch {}

  if (server) {
    server.close()
  }

  process.exit(0)
}

// Graceful shutdown handlers
process.on("SIGINT", shutdown)
process.on("SIGTERM", shutdown)

// Crash-loop breaker for the survive-by-default uncaughtException policy: if we
// keep catching exceptions this fast, the process is thrashing on corrupt state —
// stop surviving and exit so the supervisor restarts clean.
const CRASH_WINDOW_MS = 10_000
const CRASH_LOOP_MAX = 20
const crashTimes: number[] = []

// Handle uncaught errors
process.on("uncaughtException", (error) => {
  errorLog("Uncaught exception", { message: error.message, stack: error.stack })

  // node-pty on Windows produces benign "write EOF" and "AttachConsole failed" errors
  // when PTY subprocesses exit. These are NOT fatal — do not crash the server.
  // undici raises a bare `Error("terminated")` when a streaming WebSocket /
  // mockttp tunnel closes mid-flight (HTTP/2 stream RST or peer abort).
  // It happens on Linux prod under intermittent network blips and must NOT
  // tear down the whole bridge — every PTY session would die with it.
  const benign = [
    "write EOF",
    "write EPIPE",
    "AttachConsole failed",
    "read ECONNRESET",
    "terminated",
    "ERR_STREAM_PREMATURE_CLOSE",
    "UND_ERR_SOCKET",
    "UND_ERR_CONNECT_TIMEOUT",
    // undici timeouts on a slow/black-holed upstream, and OS-level connect
    // failures — all transient network conditions, never a corruption of our
    // own logic, so they must NOT tear down every PTY session on the box.
    "Headers Timeout Error",
    "Body Timeout Error",
    "Headers Overflow Error",
    "other side closed",
    "Secure Proxy Connection failed",
    // Hono/undici body stream that has already been fully consumed (e.g.
    // long-lived SSE turn whose client tab navigated away) — node tries
    // to push one final chunk into the already-closed ReadableStream and
    // throws asynchronously. Cosmetic on Linux prod, fatal on the
    // default uncaughtException policy. Tearing the whole bridge down
    // for it kills every PTY session.
    "ReadableStream is already closed",
    "Invalid state: ReadableStream is already closed",
  ]
  // Match by error.code / cause.code / name too — far more robust than a
  // message substring, since the SAME reset can surface with wildly different
  // message text ("read ECONNRESET" vs "ECONNRESET: Connection reset by peer"
  // vs "socket hang up"). These are all transient connection breaks that the
  // per-request handlers already clean up; the process must survive them.
  const benignCodes = new Set([
    "ECONNRESET", "EPIPE", "ECONNREFUSED", "ENOTFOUND", "ENETUNREACH",
    "EHOSTUNREACH", "ETIMEDOUT", "ECONNABORTED", "ERR_STREAM_PREMATURE_CLOSE",
    "ERR_STREAM_DESTROYED", "ABORT_ERR",
    "UND_ERR_SOCKET", "UND_ERR_CONNECT_TIMEOUT", "UND_ERR_HEADERS_TIMEOUT",
    "UND_ERR_BODY_TIMEOUT", "UND_ERR_ABORTED", "UND_ERR_CLOSED", "UND_ERR_DESTROYED",
  ])
  const anyErr = error as { code?: string; name?: string; cause?: { code?: string } }
  const code = anyErr.code ?? anyErr.cause?.code
  const isBenign = (code != null && benignCodes.has(code))
    || anyErr.name === "AbortError"
    || (typeof error.message === "string" && benign.some(b => error.message.includes(b)))
  if (isBenign) {
    console.error(`[bridge] Non-fatal exception (ignored): ${error.message}${code ? ` [${code}]` : ""}`)
    return
  }

  // SURVIVE-BY-DEFAULT. A stray throw reaching this handler has already unwound
  // the offending request/timer task; the V8 heap is intact and every OTHER live
  // PTY session + the HTTP/WS servers are still fully functional. Tearing them all
  // down to "escape an unknown state" is strictly worse than logging + continuing.
  // (Mirrors the always-survive `unhandledRejection` policy below.) Two genuinely
  // unrecoverable conditions still exit for a clean supervised restart:
  const msg = typeof error.message === "string" ? error.message : ""

  // (a) Best-effort out-of-memory catch. NOTE: real V8 heap exhaustion almost
  // always aborts the process via V8's own native fatal handler BEFORE this
  // JS handler runs, so this branch rarely fires — it's a backstop, not the
  // primary OOM defence. A stack-overflow RangeError is deliberately NOT matched
  // (its frame is already unwound → recoverable, so we survive that one).
  const isOom = error instanceof RangeError && /heap out of memory|out of memory/i.test(msg)
  if (isOom) {
    console.error(`[bridge] Fatal: out of memory — exiting for restart: ${msg}`)
    void shutdown()
    return
  }

  // (b) Crash-loop: too many uncaught exceptions in a short window means we're
  // spinning on corrupt state — exit so the supervisor restarts clean.
  const nowTs = Date.now()
  crashTimes.push(nowTs)
  while (crashTimes.length > 0 && nowTs - crashTimes[0]! > CRASH_WINDOW_MS) crashTimes.shift()
  if (crashTimes.length > CRASH_LOOP_MAX) {
    console.error(`[bridge] Crash-loop: ${crashTimes.length} uncaught exceptions in ${CRASH_WINDOW_MS}ms — exiting for restart`)
    void shutdown()
    return
  }

  console.error(`[bridge] Uncaught exception SURVIVED (logged, continuing): ${error.message}${code ? ` [${code}]` : ""}`)
})

process.on("unhandledRejection", (reason) => {
  errorLog("Unhandled rejection", { reason: reason instanceof Error ? { message: reason.message, stack: reason.stack } : String(reason) })
  console.error(`[bridge] Unhandled rejection: ${reason}`)
})

// Start
startup().catch((error) => {
  errorLog("Startup failed", { message: error.message, stack: error.stack })
  console.error(`[bridge] Startup failed: ${error.message}`)
  process.exit(1)
})

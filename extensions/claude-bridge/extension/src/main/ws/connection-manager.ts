import WebSocket from 'ws'
import * as vscode from 'vscode'
import type { BrowserWindow } from 'electron'
import type { ConnectionConfig, ConnectionState, PermissionDecision } from '../../shared/types'
import type { ClientMessage, ServerMessage } from '../../shared/mcp-protocol'
import { PermissionManager } from '../mcp/permission-manager'
import { JsonlBatcher } from './jsonl-batcher'
import { TranscriptMirror } from './transcript-mirror'

/** How much of the local copy is painted before the network answers. The chat
 *  windows ~150 rows and grows on scroll, so a couple of thousand records is
 *  already more than the first screens need. */
const MIRROR_PAINT_RECORDS = 2000
// Cap on how many cached records a reload replays into the renderer. The store
// is windowed there (STORE_WINDOW in JsonlViewer); older pages come back from the
// mirror on scroll-up. Kept a touch above the renderer window so the tail is
// fully covered after dedup.
const REPLAY_TAIL_MAX = 4500
import fs from 'fs'
import path from 'path'
import { SessionIdleTracker } from './session-idle-tracker'
import { handleServerMessage } from './handle-server-message'
import {
  handleHookExecute as runHookExecute,
  handleMcpCall as runMcpCall,
  type McpCallCtx,
} from './mcp-call-pipeline'

export type PermissionMode = 'ask' | 'acceptEdits' | 'plan' | 'bypassPermissions'

/** Streaming protocol this client speaks. >=1 tells the server to send
 *  incremental `streaming:delta` frames between boundary snapshots instead of a
 *  full snapshot every tick — O(n) bytes over a turn instead of O(n²). */
const CLIENT_STREAM_PROTOCOL = 1

/**
 * Connection states:
 * - disconnected: no active connection
 * - connecting: WebSocket opening
 * - connected: WebSocket open, session:create sent, waiting for session:created
 * - authenticated: session:created received, sessionId stored — fully operational
 * - error: connection failed or session error
 */
export type ExtendedStatus = 'disconnected' | 'connecting' | 'connected' | 'authenticated' | 'error'

export interface ExtendedConnectionState {
  status: ExtendedStatus
  sessionId?: string
  error?: string
  /** Epoch ms of the scheduled reconnect + which attempt it will be. Forwarded
   *  to the renderer so the status dot can count down to it instead of showing
   *  a delay that was already stale when it was written. */
  nextRetryAt?: number
  retryAttempt?: number
}

// Reconnect backoff constants
const RECONNECT_BASE_MS = 1000
const RECONNECT_MAX_MS = 30000
const RECONNECT_MULTIPLIER = 2
// The connection must stay authenticated this long before the backoff counter
// resets — dampens a close-right-after-auth flap into a graceful backoff.
const STABLE_RESET_MS = 10_000
// A reconnect only reloads the webview if the socket had been open at least this
// long before dropping. Below this, a re-open is treated as startup churn (would
// otherwise reload onto the user's first turn and kill its live streaming).
const RECONNECT_RELOAD_GRACE_MS = 30000
// Minimum gap between two webview reloads. On an UNSTABLE connection the socket
// flaps repeatedly; without this each flap fired another full `location.reload()`
// (re-parse the ~2MB webview bundle + re-render the whole JSONL backlog, all
// synchronous on the shared renderer thread) → a reload STORM that froze the
// window ("Not Responding"). One reload re-establishes the subs; coalesce the
// rest so a flapping link degrades to at most one reload per window, keeping the
// app responsive between blips instead of wedged.
const RECONNECT_RELOAD_DEBOUNCE_MS = 60000
// The socket can be OPEN + healthy (pings/pongs flow, heartbeat happy) yet never
// get a `session:created`/`session:restarted` back — e.g. the server restarted
// and the `session:resume` response was lost, or the CLI is slow to spawn under a
// reconnect storm. The external status maps this to "connecting" forever ("stuck
// Reconnecting to bridge"). If the session isn't ESTABLISHED within this window,
// tear the socket down so `close` fires and scheduleReconnect retries.
const SESSION_ESTABLISH_TIMEOUT_MS = 25000
// NOT raised for size: a 36MB installer downloads from this same box in ~2s, so
// 20MB of transcript is nowhere near 30s. A download that "never arrives" is
// therefore a FAILURE, not a slow transfer — and stretching the window only made
// the user wait longer for the error. Keep it short so the failure surfaces fast.
const DOWNLOAD_TIMEOUT_MS = 30_000
// After this many consecutive establish-timeouts while RESUMING, drop the resume
// and start a FRESH session so the user isn't wedged forever (the old conversation
// still persists server-side in its JSONL; the tab just rebinds to a new one).
const MAX_RESUME_FAILURES_BEFORE_FRESH = 3
// A resume that AUTHENTICATES (session:created) and then the CLI exits within this
// window is a "ghost resume" loop: the server passed `--resume <id>` to a CLI that
// can't find the conversation, so it exits 1 immediately. The establish-timer only
// counts sessions that NEVER authenticated, and `authenticated` zeroes its counter
// — so this fast created→exit blink slips through forever ("chat dies, doesn't
// recover"). A separate counter that authenticated does NOT reset breaks it.
const RESUME_LOOP_ALIVE_MS = 8000

export class ConnectionManager {
  // Shared cache of the last tree update (all connections share one tree per token)
  static lastTree: unknown[] | null = null
  // Optional callback to enrich tree with tab metadata before sending to renderer
  static enrichTree: ((tree: any[]) => void) | null = null
  // Optional callback when session info (effort/model) arrives from server
  static onSessionInfo: ((tabId: string, effort?: string, model?: string) => void) | null = null
  // Optional callback when session title arrives from server
  static onSessionTitle: ((tabId: string, title: string) => void) | null = null
  // Optional callback when conversationId is discovered (for session saving)
  static onConversationId: ((tabId: string, conversationId: string) => void) | null = null
  // Optional callback when session is authenticated — used to send external tool schemas
  static onAuthenticated: ((connection: ConnectionManager) => void) | null = null
  // Optional callback to get external MCP tool schemas for session:create
  static getExternalToolSchemas: (() => Array<{ name: string; description: string; inputSchema: Record<string, unknown> }>) | null = null
  // The user's standing instructions for the current token. Read at session
  // start (create AND resume — a resume respawns the CLI, so it must carry them
  // too) and APPENDED server-side to the technical system prompt. A hook rather
  // than a config field so an edit applies to the next session without
  // rebuilding the connection.
  static getBasePrompt: (() => string) | null = null
  // Optional gate resolved once the external MCP servers have settled (connected
  // or failed), bounded so a hung server can't stall session start. Awaited
  // BEFORE session:create/resume so the CLI's initial context already carries the
  // full external tool set — otherwise stdio servers finish connecting a few
  // seconds AFTER the session prompt is built, push the tool count past the CLI's
  // deferred-tools threshold, and the CLI yanks already-active tools out of the
  // set mid-turn (a `deferred_tools_delta` the model reads as "Read disconnected").
  // Resolves instantly once the initial discovery is done, so only the first
  // session after a cold launch pays the wait.
  static whenExternalToolsReady: (() => Promise<void>) | null = null
  // Notification hooks — fired so main/index.ts can spawn background-toast
  // mini-windows when the user is not looking at the main window.
  static onElicitationRequest: ((tabId: string, payload: { requestId: string; toolName: string; message: string }) => void) | null = null
  static onElicitationResolved: ((tabId: string, requestId: string) => void) | null = null
  static onSessionIdle: ((tabId: string, title: string) => void) | null = null
  // VSIX: per-tab connection status + working state → mirrored onto the owning
  // KaminIDE session's metadata (status dot) by SessionsBridge.
  static onStatus: ((tabId: string, status: ConnectionState['status']) => void) | null = null
  static onActivity: ((tabId: string, working: boolean, hookDriven: boolean) => void) | null = null

  private ws: WebSocket | null = null
  private state: ExtendedConnectionState = { status: 'disconnected' }
  private config: ConnectionConfig
  private window: BrowserWindow

  // Reconnect
  private reconnectTimer: ReturnType<typeof setTimeout> | null = null
  // Heartbeat watchdog. Server sends ping every 15s; we listen for the implicit
  // pong responses through ws.on('pong') AND for any inbound traffic. If 30s
  // pass with neither, we tear the socket down ourselves — half-open
  // connections (laptop suspend, VPN drop, NAT timeout) never surface a
  // 'close' event from the OS otherwise.
  private heartbeatTimer: ReturnType<typeof setInterval> | null = null
  private lastHeartbeatAt: number = 0
  // Session-establishment watchdog: armed after session:create/resume is sent,
  // cleared when the server confirms (state → authenticated). See close-storm note.
  private establishTimer: ReturnType<typeof setTimeout> | null = null
  private establishFailures = 0
  // Ghost-resume loop breaker (see RESUME_LOOP_ALIVE_MS). Tracks when the last
  // session:created landed and how many resumes authenticated-then-died fast.
  private lastSessionCreatedAt = 0
  private rapidResumeExits = 0
  private reconnectAttempt = 0
  // Stability gate for the backoff reset. The server confirming the session
  // (authenticated) does NOT immediately zero reconnectAttempt — a socket that
  // authenticates then drops seconds later (flaky prod link, a server that
  // closes right after resume) would otherwise reset the backoff on every cycle
  // and reconnect every base-delay (~1s), showing as a fast "Reconnecting…"
  // blink. We only reset once the connection has STAYED authenticated for
  // STABLE_RESET_MS; a close before then keeps the backoff escalating so a
  // flapping link settles instead of hammering. A genuine one-off blip still
  // recovers fast (its prior stable period already reset the counter).
  private stabilityTimer: ReturnType<typeof setTimeout> | null = null
  private intentionalDisconnect = false
  // True once the socket has opened at least once. A SECOND+ open is a
  // reconnect (server/container restart, network blip): we send `bridge:reconnected`
  // so the webview cleanly RE-SUBSCRIBES its event handlers (bridge-shim bumps a
  // signal → useBridgeListeners re-runs) and re-pulls missed entries — the subs
  // can otherwise go stale and silently swallow streaming+JSONL. NB: the webview
  // no longer reloads the iframe on this — it re-subscribes in place
  // (fix_webview_subs_drop_on_reconnect / bridge-shim `bridge:reconnected`).
  private hasOpenedBefore = false
  // Timestamp of the previous ws 'open'. A re-open that follows the previous one
  // quickly is STARTUP CHURN (initial connect retry / early re-resolve), not a
  // real mid-session reconnect. Resyncing then would land on the user's first
  // turn, so we only resync when the socket had been up for a while — a genuine
  // long-lived session that dropped and came back.
  private lastOpenAt = 0
  // Timestamp of the last `bridge:reconnected` we sent — the storm guard
  // (RECONNECT_RELOAD_DEBOUNCE_MS; the webview debounces resubscribe via this).
  private lastReloadAt = 0

  // Pending MCP calls (requestId → { toolName, input })
  private pendingMcpCalls = new Map<string, { toolName: string; input: Record<string, unknown> }>()

  // Permission responses waiting to be resolved
  private pendingPermissions = new Map<string, (decision: PermissionDecision) => void>()

  private idleTracker: SessionIdleTracker

  // Cached JSONL data for replay after renderer reload
  private cachedJsonlEntries: any[] = []
  // По cacheKey (agentId — хэш файла) храним И имя агента: реплей из кэша
  // обязан слать НАСТОЯЩЕЕ agentName — под ключом-хэшем вид агента по имени
  // записи не находил («No messages from this agent yet» после рестарта).
  private cachedSubagentEntries = new Map<string, { agentName: string; agentId?: string; entries: any[] }>()
  // Предел записей на одного сабагента в кэше хоста (см. pushSubagentCache).
  private static readonly SUBAGENT_CACHE_CAP = 5000
  private cachedJsonlStatus: any = null
  // Server's authoritative compact-segment index (delivered with replayComplete).
  // Held so an on-demand `loadBoundaries` re-request answers instantly; nulled on
  // compaction. Replaces the old mirror-computed index, which was blind to
  // out-of-window segments and inflated by resume-duplicated records.
  private lastSegmentIndex: { boundaries: { ts: string }[]; counts: number[] } | null = null
  // The latest in-flight streaming assistant entry (proxy stub). JSONL replay
  // only recovers CANONICAL entries; a turn still generating has no canonical
  // yet, so a webview reload mid-turn (startup re-resolve, reconnect, settings
  // reload) would lose its live word-by-word render until the canonical lands.
  // We cache the newest streaming stub and re-emit it on replay so the animation
  // resumes. Cleared when the turn finalizes (non-streaming entry) or the tab
  // resets. A stale re-emit is harmless — the client drops it once the canonical
  // for that message.id exists (applyStreamingEntry guard).
  private cachedStreamingEntry: any = null

  private batcher: JsonlBatcher

  // Permission mode (controls which tools need user approval)
  private permissionMode: PermissionMode = 'bypassPermissions'
  private permissionManager = new PermissionManager()

  private tabId: string
  /** Читается claude-bridge.getSessionInfo (тест-команды) — потому public. */
  lastModel: string | null = null
  lastEffort: string | null = null
  private conversationId: string | null = null
  private settingsDir: string | null = null
  private _downloadResolve: ((result: { content: string | null; fileName: string | null; error?: string }) => void) | null = null
  private _downloadTimeout: NodeJS.Timeout | null = null
  /** Chunks of an in-flight transcript download, joined once at the end. */
  private _downloadParts: string[] | null = null

  /** Local copy of this session's transcript, so a restart doesn't re-pull it
   *  over the network. Created once the conversation id is known — that is the
   *  file's identity. Absent (no id yet, or no cache dir) simply means no
   *  mirroring this run; the session works exactly as before. */
  private mirror: TranscriptMirror | null = null
  /** Fingerprint the server last reported for the transcript file. Stored with
   *  the mirror so a later run can prove its copy is still a prefix. */
  private mirrorHead: string | null = null

  constructor(config: ConnectionConfig, window: BrowserWindow, tabId: string, conversationId?: string) {
    this.config = config
    this.window = window
    this.tabId = tabId
    if (conversationId) this.conversationId = conversationId
    this.batcher = new JsonlBatcher(window, tabId)
    this.idleTracker = new SessionIdleTracker((rawTitle) => {
      ConnectionManager.onSessionIdle?.(this.tabId, rawTitle)
    })
  }

  // Background (warm-pool) tab: live transcript/streaming forwarding to the
  // webview is off until the tab is first shown (see JsonlBatcher.muted).
  private rendererMuted = false

  setRendererMuted(muted: boolean): void {
    this.rendererMuted = muted
    this.batcher.setMuted(muted)
  }

  getLastModel(): string | null { return this.lastModel }
  getTabId(): string { return this.tabId }
  getSettingsDir(): string | null { return this.settingsDir }
  setPermissionMode(mode: PermissionMode): void { this.permissionMode = mode }
  getSessionId(): string | undefined { return this.state.sessionId }

  /** The CLI conversation id — also the JSONL's basename on the server, so a
   *  download can name its file WITHOUT first fetching the transcript. */
  getConversationId(): string | null { return this.conversationId }

  /**
   * Initiate WebSocket connection to server. On open, sends session:create
   * with auth token (or session:resume when we have a previous conversationId).
   */
  connect(): void {
    if (this.ws) this.disconnect()

    this.intentionalDisconnect = false
    this.setState({ status: 'connecting' })

    // Ensure WS protocol + idempotent `/ws/session` suffix. Stored config can
    // already include the path (older gate modal used to write it), and
    // appending a second copy made the socket hang in "Connecting" forever.
    // Force `localhost` → `127.0.0.1`: on Windows `localhost` resolves to IPv6
    // `::1` first, but a WSL/podman port-forward (wslrelay) listens on IPv4 only,
    // so every connect wastes a dead `::1` attempt before falling back — which
    // showed as slow connects / reconnect churn. IPv4 is direct and stable.
    const base = this.config.serverUrl
      .replace(/^http/, 'ws')
      .replace(/\/+$/, '')
      .replace(/\/ws\/session$/, '')
      .replace(/\/\/localhost(?=[:/]|$)/i, '//127.0.0.1')
    const wsUrl = `${base}/ws/session`

    try {
      this.ws = new WebSocket(wsUrl)
    } catch (err) {
      const errMsg = err instanceof Error ? err.message : String(err)
      this.setState({ status: 'error', error: `Failed to create WebSocket: ${errMsg}` })
      this.scheduleReconnect()
      return
    }

    this.ws.on('open', () => {
      this.setState({ status: 'connected' })
      this.startHeartbeat()
      // Suppress the "Session finished" toast for the attach-replay window: the
      // server resends cached status (possibly a stale isWorking:true) right after
      // open, whose working→idle would otherwise fire a bogus toast on session open.
      this.idleTracker.armSettle()
      // A reconnect (not the first open): the server is resuming this session,
      // but the webview's event subs may have gone stale during the drop. Send
      // `bridge:reconnected` so it re-subscribes in place (bridge-shim's module-
      // level handler survives the wedge; NO iframe reload). Guard on uptime:
      // only a socket that had been open a while (a real mid-session reconnect)
      // triggers it — a quick startup re-open is churn that would resync onto the
      // first turn needlessly.
      const now = Date.now()
      const sinceLastOpen = this.lastOpenAt ? now - this.lastOpenAt : Infinity
      this.lastOpenAt = now
      const isReconnect = this.hasOpenedBefore
      this.hasOpenedBefore = true
      // Resync only a genuine mid-session reconnect (uptime past the grace), and
      // at most once per debounce window so a flapping link can't storm re-subs.
      // A suppressed resync is safe: the socket is back and streaming resumes;
      // the last resync already re-established the subs.
      if (isReconnect && sinceLastOpen > RECONNECT_RELOAD_GRACE_MS && now - this.lastReloadAt > RECONNECT_RELOAD_DEBOUNCE_MS) {
        this.lastReloadAt = now
        try { this.window.webContents.send('bridge:reconnected') } catch { /* window gone */ }
      }
      // Defer session:create/resume until the external MCP servers have settled
      // so the CLI's initial tool set is complete (see whenExternalToolsReady).
      // The gate is bounded and resolves instantly on a warm start / reconnect,
      // so this never actually waits except on the first cold-launch session.
      const sock = this.ws
      if (sock) void this.startSession(sock)
    })

    this.ws.on('message', (raw) => {
      // Any inbound traffic counts as proof-of-life — refresh the heartbeat
      // clock so a busy stream doesn't trigger a false-positive termination
      // just because the server happens not to ping in a 30s window.
      this.lastHeartbeatAt = Date.now()
      try {
        const msg = JSON.parse(raw.toString()) as ServerMessage
        this.dispatchMessage(msg)
      } catch {
        // Ignore unparsable messages
      }
    })

    this.ws.on('pong', () => { this.lastHeartbeatAt = Date.now() })
    this.ws.on('ping', () => { this.lastHeartbeatAt = Date.now() })

    this.ws.on('close', (code, reason) => {
      this.stopHeartbeat()
      this.clearEstablishTimer() // socket gone; the next open re-arms it
      // Dropped before proving stable → keep the escalating backoff (don't let a
      // pending stability reset fire and restart the blink at the base delay).
      this.clearStabilityTimer()
      this.ws = null
      if (!this.intentionalDisconnect) {
        const reasonStr = reason?.toString() || ''
        this.setState({
          status: 'disconnected',
          error: reasonStr ? `Connection closed: ${code} ${reasonStr}` : undefined,
        })
        this.scheduleReconnect()
      } else {
        this.setState({ status: 'disconnected' })
      }
    })

    this.ws.on('error', (err) => {
      this.setState({ status: 'error', error: err.message })
      // The 'close' event will fire after 'error', which handles reconnect
    })
  }

  /** Send the initial session:create/resume once external MCP tools are ready,
   *  so the CLI builds its context with the complete tool set (no mid-turn
   *  deferred-tools churn). Bounded wait — instant on warm start / reconnect. */
  private async startSession(sock: WebSocket): Promise<void> {
    try { await ConnectionManager.whenExternalToolsReady?.() } catch { /* proceed with whatever connected */ }
    // A reconnect may have swapped the socket while we waited — never write to a
    // stale or closed pipe (the current socket runs its own startSession).
    if (this.ws !== sock || sock.readyState !== WebSocket.OPEN) return
    const externalTools = ConnectionManager.getExternalToolSchemas?.() ?? []
    const basePrompt = ConnectionManager.getBasePrompt?.() ?? ''
    // Where THIS machine mirrors the full transcript (uncapped, append-only). The
    // server embeds it in the CLI's system prompt so Claude — whose file tools run
    // on the user's machine (user-tools MCP), the same machine — can read back its
    // own history (incl. turns a /compact dropped from context). Absolute so it
    // resolves regardless of the tool shell's cwd/env.
    const cacheDir = process.env.KAMIN_CACHE_DIR
    const transcriptMirrorDir = cacheDir ? path.join(cacheDir, 'transcripts') : undefined
    if (this.conversationId) {
      this.sendRaw({
        type: 'session:resume',
        token: this.config.token,
        conversationId: this.conversationId,
        cwd: this.config.cwd,
        cols: 120,
        rows: 40,
        protocolVersion: CLIENT_STREAM_PROTOCOL,
        ...(basePrompt ? { basePrompt } : {}),
        ...(transcriptMirrorDir ? { transcriptMirrorDir } : {}),
        ...(externalTools.length > 0 ? { externalTools } : {}),
      })
    } else {
      this.sendRaw({
        type: 'session:create',
        token: this.config.token,
        cwd: this.config.cwd,
        cols: 120,
        rows: 40,
        protocolVersion: CLIENT_STREAM_PROTOCOL,
        ...(basePrompt ? { basePrompt } : {}),
        ...(transcriptMirrorDir ? { transcriptMirrorDir } : {}),
        ...(externalTools.length > 0 ? { externalTools } : {}),
      })
    }
    this.armEstablishTimer()
  }

  /** Watchdog for a socket that opened but never got its session confirmed.
   *  Tears the socket down (→ close → scheduleReconnect); after repeated resume
   *  failures, drops the resume so the next attempt starts a fresh session. */
  private armEstablishTimer(): void {
    this.clearEstablishTimer()
    this.establishTimer = setTimeout(() => {
      this.establishTimer = null
      if (this.state.status === 'authenticated') return // already established
      this.establishFailures += 1
      if (this.conversationId && this.establishFailures >= MAX_RESUME_FAILURES_BEFORE_FRESH) {
        this.conversationId = null // next startSession will session:create fresh
      }
      try { this.ws?.terminate() } catch { /* already gone */ }
    }, SESSION_ESTABLISH_TIMEOUT_MS)
  }

  private clearEstablishTimer(): void {
    if (this.establishTimer) { clearTimeout(this.establishTimer); this.establishTimer = null }
  }

  /** session:created arrived — mark the moment so a fast following session:exit is
   *  recognisable as a ghost-resume blink. */
  private noteSessionCreated(): void {
    this.lastSessionCreatedAt = Date.now()
  }

  /** session:exit arrived — if this session authenticated then died within
   *  RESUME_LOOP_ALIVE_MS while RESUMING, it's a ghost-resume loop the establish
   *  watchdog can't see (it authenticated). Count it; after enough, drop the
   *  resume so the NEXT startSession creates a fresh conversation instead of
   *  re-resuming the same dead id forever. A session that lived longer resets it. */
  private noteSessionExit(): void {
    const alive = this.lastSessionCreatedAt > 0 ? Date.now() - this.lastSessionCreatedAt : Infinity
    if (this.conversationId && alive < RESUME_LOOP_ALIVE_MS) {
      this.rapidResumeExits += 1
      if (this.rapidResumeExits >= MAX_RESUME_FAILURES_BEFORE_FRESH) {
        this.conversationId = null // next startSession session:creates fresh
        this.rapidResumeExits = 0
      }
    } else {
      this.rapidResumeExits = 0 // a session that ran a while → not a resume loop
    }
  }

  /** Arm the stability-gated backoff reset: only zero reconnectAttempt after the
   *  connection has stayed up STABLE_RESET_MS. Cleared on close, so a socket that
   *  drops before then keeps the escalating backoff (no fast reconnect blink). */
  private armStabilityReset(): void {
    this.clearStabilityTimer()
    this.stabilityTimer = setTimeout(() => {
      this.stabilityTimer = null
      this.reconnectAttempt = 0
    }, STABLE_RESET_MS)
  }

  private clearStabilityTimer(): void {
    if (this.stabilityTimer) { clearTimeout(this.stabilityTimer); this.stabilityTimer = null }
  }

  private startHeartbeat(): void {
    this.stopHeartbeat()
    this.lastHeartbeatAt = Date.now()
    this.heartbeatTimer = setInterval(() => {
      if (!this.ws || this.ws.readyState !== WebSocket.OPEN) return
      const elapsed = Date.now() - this.lastHeartbeatAt
      if (elapsed > 30_000) {
        // Two server-side ping cycles missed — socket is dead. Pull the plug
        // ourselves so 'close' fires and the existing reconnect logic kicks
        // in (otherwise we'd sit in OPEN-state purgatory).
        try { this.ws.terminate() } catch { /* ignore */ }
        return
      }
      // Spontaneous ping from the client side too — server may have suspended
      // its ping timer (idle path) but the OS still routes pings/pongs that
      // prove liveness. Cheap insurance.
      try { this.ws.ping() } catch { /* ignore */ }
    }, 15_000)
  }

  private stopHeartbeat(): void {
    if (this.heartbeatTimer) {
      clearInterval(this.heartbeatTimer)
      this.heartbeatTimer = null
    }
  }

  /**
   * Intentionally disconnect. Cancels any pending reconnect.
   *
   * `endSession: true` (explicit user Disconnect / tab close) also tells the
   * server to destroy the session NOW. Without it — including the internal
   * call from connect() — the server only detaches and keeps the CLI alive
   * for its reattach grace window, so a quick reconnect resumes the same
   * process instead of killing + respawning on the same JSONL.
   */
  disconnect(opts: { endSession?: boolean } = {}): void {
    this.intentionalDisconnect = true
    this.cancelReconnect()
    this.stopHeartbeat()
    this.clearEstablishTimer()
    this.clearStabilityTimer()

    if (this.ws) {
      if (opts.endSession && this.ws.readyState === WebSocket.OPEN) {
        try { this.ws.send(JSON.stringify({ type: 'session:end' })) } catch { /* closing anyway */ }
      }
      // Remove listeners to avoid triggering reconnect on close
      this.ws.removeAllListeners()
      // close() на CONNECTING-сокете эмитит 'error' («WebSocket was closed
      // before the connection was established»); после removeAllListeners его
      // некому ловить — uncaughtException и тост «Extension crashed» на
      // старте приложения (гонка тёплого пула). Глушим осознанно.
      this.ws.on('error', () => { /* teardown: сокет и так закрываем */ })
      if (this.ws.readyState === WebSocket.OPEN || this.ws.readyState === WebSocket.CONNECTING) {
        this.ws.close()
      }
      this.ws = null
    }

    // Flush any in-flight JSONL batches so the renderer doesn't miss the
    // final entries.
    this.batcher.flushMain()
    this.batcher.flushSubagent()

    // Close the mirror's write stream when the session is truly ending (tab
    // close / free-from-memory) — a transient disconnect keeps it so the reopened
    // socket keeps appending. Without this, its file descriptor leaked for every
    // opened-then-closed tab over a long app session.
    if (opts.endSession) void this.mirror?.close()
    // Висящий idle-debounce умирает вместе с сессией: иначе тост «Session
    // finished» стрелял для уже закрытого таба (аудит #70 C14).
    if (opts.endSession) this.idleTracker.dispose()

    this.pendingMcpCalls.clear()
    // Осиротевшие permission-промисы резолвим отказом, иначе handleMcpCall
    // висит на await навсегда (утечка замыкания; сервер отваливается по
    // своему таймауту без ответа).
    for (const resolve of this.pendingPermissions.values()) {
      try { resolve('deny') } catch { /* ignore */ }
    }
    this.pendingPermissions.clear()
    // reconnectAttempt здесь НЕ сбрасывается: connect() при живом сокете
    // зовёт disconnect(), и сброс превращал backoff в вечный 1-секундный
    // цикл при «CLI умирает сразу после спавна». Сброс делают stability-
    // таймер (armStabilityReset) и явный пользовательский disconnect
    // (endSession).
    if (opts.endSession) this.reconnectAttempt = 0
    this.setState({ status: 'disconnected' })
  }

  /** Returns current connection state. Maps ExtendedConnectionState to
   *  ConnectionState for IPC compatibility. */
  getState(): ConnectionState {
    return {
      status: this.state.status === 'authenticated' ? 'connected'
        : this.state.status === 'connecting' ? 'connecting'
        : this.state.status === 'connected' ? 'connecting'
        : this.state.status as ConnectionState['status'],
      sessionId: this.state.sessionId,
      error: this.state.error,
    }
  }

  /** Returns internal extended state (includes 'authenticated'). */
  getExtendedState(): ExtendedConnectionState { return { ...this.state } }

  sendInput(data: string): void { this.send({ type: 'session:input', data }) }
  /** Stop: abort the in-flight upstream stream on the server AND SIGINT the CLI. */
  interruptSession(): void { this.send({ type: 'session:interrupt' }) }

  /** Submit a full user message — server wraps it in bracketed paste and
   *  times the Enter via a quiet-window gate. Replaces the renderer's manual
   *  paste+retry dance. */
  submitText(text: string): void { this.send({ type: 'session:submitText', data: text }) }

  resize(cols: number, rows: number): void { this.send({ type: 'session:resize', cols, rows }) }

  /** Respond to a permission request from the renderer. */
  respondPermission(requestId: string, decision: PermissionDecision): void {
    const resolve = this.pendingPermissions.get(requestId)
    if (resolve) {
      resolve(decision)
      this.pendingPermissions.delete(requestId)
    }
    // handleMcpCall() sends mcp:denied if !allowed, mcp:response if allowed
  }

  /** Владеет ли ЭТО соединение ожидающим MCP-вызовом (адресация ответа
   *  вместо броадкаста во все табы — аудит #70 C15). */
  hasPendingMcp(requestId: string): boolean {
    return this.pendingMcpCalls.has(requestId)
  }

  /** Send an MCP tool result back to the server. */
  respondMcp(requestId: string, result: unknown): void {
    this.send({ type: 'mcp:response', requestId, result })
    this.pendingMcpCalls.delete(requestId)
    this.window.webContents.send('mcp-activity', this.tabId, {
      requestId,
      toolName: '',
      input: {},
      timestamp: Date.now(),
      status: 'completed',
      result,
    })
  }

  /** Send an MCP tool denial back to the server. */
  denyMcp(requestId: string, reason: string): void {
    this.send({ type: 'mcp:denied', requestId, reason })
    this.pendingMcpCalls.delete(requestId)
    this.window.webContents.send('mcp-activity', this.tabId, {
      requestId,
      toolName: '',
      input: {},
      timestamp: Date.now(),
      status: 'denied',
    })
  }

  /** Replay all cached JSONL entries/status/subagent data to the renderer.
   *  Called after renderer reload (HMR or manual page refresh). */
  /** Paint the chat from the LOCAL copy, before the network is involved at all.
   *
   *  Measured on a real session: 85MB / 30 971 records took 15-20s to pull from
   *  the server, against 286ms to read the same bytes from disk. The chat only
   *  needs its tail to paint, which is a fraction of that again.
   *
   *  Returns how many records were served, so the caller can tell whether it
   *  still owes the renderer a full replay. Zero means the mirror was empty,
   *  unreadable, or belongs to a file the server has since rewritten — every
   *  one of which falls back to the network path unchanged. */
  async replayFromMirror(maxRecords = MIRROR_PAINT_RECORDS): Promise<number> {
    const records = await this.readMirrorTail(maxRecords)
    if (records.length === 0) return 0
    // Straight to the renderer, not through the batcher: these are already
    // ordered and bounded, and the point is to be on screen immediately.
    this.window.webContents.send('jsonl-entries', this.tabId, records)
    return records.length
  }

  replayJsonlToRenderer(): void {
    // Flush any in-flight IPC batches FIRST. Otherwise the renderer sees the
    // stale cache first and a 50ms-later batch with overlapping entries
    // second, which confuses dedup + scroll-pinning.
    this.batcher.flushMain()
    this.batcher.flushSubagent()

    // Reset the renderer's live flag so it re-enters replay mode. Without
    // this, a tab that previously saw `replayComplete` would treat the
    // cached historical entries below as live, resurrecting dead agents/teams
    // in the sidebar. The cached replayComplete is re-sent at the end, so
    // the renderer transitions back to live after all replay entries are
    // parsed.
    this.window.webContents.send('jsonl-status', this.tabId, { status: 'watching', replayComplete: false })

    // Everything after the entries must run as the drain's CONTINUATION, not on
    // the next line: the transcript now leaves in bounded, yielding chunks, so a
    // trailing send issued here would overtake the chunks still in flight and
    // the renderer would see `replayComplete` before the history it completes.
    const tail = (): void => {
      for (const [cacheKey, slot] of this.cachedSubagentEntries) {
        if (slot.entries.length > 0) {
          this.window.webContents.send('jsonl-subagent-entries', this.tabId, slot.agentName, slot.entries, slot.agentId ?? cacheKey)
        }
      }
      // Replay actual cached status LAST so replayComplete transition happens
      // after all historical entries have been processed.
      if (this.cachedJsonlStatus) {
        this.window.webContents.send('jsonl-status', this.tabId, this.cachedJsonlStatus)
      } else if (this.state.status !== 'authenticated') {
        // Warm-prefill без единого серверного replayComplete (аудит #70 C12):
        // кэша статуса нет, и рендерер застревал в replay-mode (флаг live
        // сброшен выше и никем не возвращался). Зеркало показано целиком —
        // закрываем реплей синтетически; активация таба прогонит настоящий
        // цикл false→true заново. У АУТЕНТИФИЦИРОВАННОГО соединения статуса
        // может не быть лишь при реплее В ПОЛЁТЕ — там закрывать нельзя
        // (исторические записи прочитались бы как live).
        this.window.webContents.send('jsonl-status', this.tabId, {
          status: 'watching',
          replayComplete: true,
        })
      }
      if (ConnectionManager.lastTree) {
        this.window.webContents.send('tree-update', ConnectionManager.lastTree)
      }
      // Re-emit the in-flight streaming stub LAST (after historical entries +
      // replayComplete), so a webview that reloaded mid-turn re-shows the partial
      // assistant bubble and keeps animating from the live deltas that follow.
      // Harmless if the turn already finalized: the client drops it once the
      // canonical entry with the same message.id is present.
      if (this.cachedStreamingEntry) {
        this.window.webContents.send('streaming-entry', this.tabId, this.cachedStreamingEntry)
      }
    }

    // The whole transcript used to go out in ONE ipc here, bypassing the
    // batcher's 150-entry cap — the only jsonl-entries send that did. On a real
    // session that is a 46MB frame (191ms of JSON.parse in the renderer alone);
    // at the 20k cache cap ≈169MB / ≈985ms of serialization in a single
    // uninterruptible task. The prod freeze logs match it exactly:
    // lastCh=kamin:webview:post, counts=[] (the renderer processed NOTHING — one
    // long task, not a flood), heap spiking to ~105MB over a 27MB baseline.
    // Replay only the NEWEST REPLAY_TAIL_MAX of the cache, not the whole (up to
    // 20k) history. The renderer holds a windowed store; re-pushing 20k on every
    // reload was the freeze — 133 batches, each re-run through the renderer's
    // O(store) append pipeline over a store that grew to 20k (O(N²)). The older
    // pages come back from the disk mirror on scroll-up; overlap with the
    // mirror-first tail paint is deduped by uuid.
    //
    // "Newest" is by `_ord`, NOT array position: the historical replay streams
    // the current segment NEWEST-batch-first (emitRangeFromEnd), so the array's
    // FRONT is the recent tail and its END is the oldest — `.slice(-N)` would
    // grab the oldest N and could omit the current conversation entirely. `_ord`
    // is the server's file-order counter, so the largest `_ord`s are the newest.
    let toReplay = this.cachedJsonlEntries
    if (toReplay.length > REPLAY_TAIL_MAX) {
      const ord = (e: unknown): number => (e as { _ord?: number })._ord ?? 0
      toReplay = [...toReplay].sort((a, b) => ord(a) - ord(b)).slice(-REPLAY_TAIL_MAX)
    }
    if (toReplay.length > 0) this.batcher.drainMainThen([...toReplay], tail)
    else tail()
  }

  /** Применения одного тула из полного кэша хоста — для инлайн-просмотра в
   *  панели Tools. Панельный стор держит только plan/todo-срез (OOM-диета),
   *  поэтому записи отдаёт хост по запросу: assistant-записи с tool_use этого
   *  тула + спаренные tool_result. Ограничено последними MAX парами. */
  toolUsageEntries(toolName: string, max = 60): unknown[] {
    const strip = (n: string): string => n.replace(/^mcp__[^_]+(?:_[^_]+)*__/, '')
    const useIds = new Set<string>()
    const out: unknown[] = []
    for (const raw of this.cachedJsonlEntries) {
      const e = raw as { type?: string; message?: { content?: unknown } }
      const content = e.message?.content
      if (!Array.isArray(content)) continue
      if (e.type === 'assistant') {
        let hit = false
        for (const b of content as Array<{ type?: string; id?: string; name?: unknown }>) {
          if (b?.type === 'tool_use' && typeof b.name === 'string' && strip(b.name) === toolName) {
            if (b.id) useIds.add(b.id)
            hit = true
          }
        }
        if (hit) out.push(raw)
      } else if (e.type === 'user') {
        for (const b of content as Array<{ type?: string; tool_use_id?: string }>) {
          if (b?.type === 'tool_result' && b.tool_use_id && useIds.has(b.tool_use_id)) {
            out.push(raw)
            break
          }
        }
      }
    }
    return out.slice(-max * 2)
  }

  /** Send an elicitation response back to the server. */
  respondElicitation(requestId: string, action: 'accept' | 'deny' | 'dismiss', content?: Record<string, unknown>): void {
    this.send({ type: 'elicitation:response', requestId, action, content })
    ConnectionManager.onElicitationResolved?.(this.tabId, requestId)
  }

  /** Request effort level change (triggers PTY restart on server). */
  changeEffort(effort: string): void { this.send({ type: 'session:change-effort', effort }) }

  /** Request model change (triggers PTY restart on server). */
  changeModel(model: string): void { this.send({ type: 'session:change-model', model }) }

  /** Resume a previous session by conversationId. */
  resume(conversationId: string): void {
    this.send({
      type: 'session:resume',
      token: this.config.token,
      conversationId,
      cwd: this.config.cwd,
      cols: 120,
      rows: 40,
      protocolVersion: CLIENT_STREAM_PROTOCOL,
    })
  }

  /** Request raw JSONL file content from the server for download.
   *  Resolves with the file content or an error. */
  /** Point the mirror at this conversation's file. Cheap and idempotent — the
   *  conversation id can be learned more than once (resume, restart). */
  private ensureMirror(): void {
    const dir = process.env.KAMIN_CACHE_DIR
    if (!dir || !this.conversationId || this.mirror) return
    try {
      const root = path.join(dir, 'transcripts')
      fs.mkdirSync(root, { recursive: true })
      this.mirror = new TranscriptMirror(root, this.conversationId)
    } catch {
      this.mirror = null // no local copy this run; the session is unaffected
    }
  }

  /** What our local copy can prove it holds — the resume point handed to the
   *  server. `null` means "send me everything". */
  async getMirrorCursor(): Promise<{ head: string; pos: number } | null> {
    this.ensureMirror()
    const cursor = await this.mirror?.readCursor()
    return cursor ? { head: cursor.head, pos: cursor.pos } : null
  }

  /** The tail of the local copy, for painting the chat before the network
   *  answers at all. */
  async readMirrorTail(maxRecords: number): Promise<unknown[]> {
    this.ensureMirror()
    return (await this.mirror?.readTail(maxRecords)) ?? []
  }

  /** Serve the page of records just OLDER than `beforePos` (the webview's oldest
   *  held byte offset) so a windowed chat can grow upward on scroll. Delivered on
   *  a DEDICATED channel — the renderer prepends them as-is (they are already in
   *  file order and entirely older than the window), never re-sorting the store.
   *  Mirror-backed: it is populated as batches arrive + at replay-complete, so a
   *  session long enough to need windowing has it. An empty result (start of file
   *  reached, or no mirror) simply tells the renderer there is nothing more. */
  async loadOlderPage(beforePos: number, count: number): Promise<void> {
    this.ensureMirror()
    const records = (await this.mirror?.readRange(beforePos, count)) ?? []
    this.window.webContents.send('jsonl:older-page', this.tabId, { beforePos, records })
  }

  /** The FULL compact-segment index — every segment with visible-message counts,
   *  not just the ones in the renderer's resident window. Computed server-side
   *  from the authoritative transcript and pushed with replayComplete; this
   *  on-demand path just re-sends the cached copy (a webview reload re-requests
   *  it). Empty until the first replay-complete lands. */
  loadBoundaries(): void {
    const index = this.lastSegmentIndex ?? { boundaries: [], counts: [] }
    this.window.webContents.send('jsonl:boundaries-result', this.tabId, index)
  }

  /** One archived compact segment loaded whole by TIMESTAMP range — the renderer
   *  replaces its window with it (the "view an old conversation" path). Served by
   *  the SERVER from the authoritative file (the local mirror is a bounded tail
   *  and may not hold an old range); the response arrives on `jsonl:segment-response`
   *  and is forwarded to the renderer. Empty fromTs = from the start; empty
   *  toTs = to the newest. */
  loadSegment(fromTs: string, toTs: string): void {
    this.sendRaw({ type: 'jsonl:segment-request', fromTs, toTs })
  }

  requestJsonlDownload(): Promise<{ content: string | null; fileName: string | null; error?: string }> {
    return new Promise((resolve) => {
      this._downloadResolve = resolve
      this.sendRaw({ type: 'jsonl:download-request' })
      // If this fires, the response never came at all — the transport is fast
      // (36MB in ~2s from this same server), so a timeout means a fault, not a
      // slow file. The caller used to discard this result, which is why a failed
      // download looked like a button that does nothing.
      // The handle is kept and cleared on resolve: a stale timer from download A
      // otherwise fired into download B's resolver (killing it mid-transfer) —
      // B's chunks then hit a null resolver and were silently dropped.
      if (this._downloadTimeout) clearTimeout(this._downloadTimeout)
      this._downloadTimeout = setTimeout(() => {
        this._downloadTimeout = null
        if (this._downloadResolve) {
          this._downloadResolve({ content: null, fileName: null, error: 'No response from the server (30s) — the session may have no active JSONL watcher' })
          this._downloadResolve = null
        }
      }, DOWNLOAD_TIMEOUT_MS)
    })
  }

  // ─── Private ────────────────────────────────────────────

  private dispatchMessage(msg: ServerMessage): void {
    handleServerMessage(msg, {
      window: this.window,
      tabId: this.tabId,
      batcher: this.batcher,
      setState: (s) => this.setState(s),
      getStatus: () => this.state.status,
      setLastModel: (m) => { this.lastModel = m },
      setSettingsDir: (d) => { this.settingsDir = d },
      setConversationId: (id) => {
        // A DIFFERENT conversation (resume fell back to a fresh session:
        // resumeNotFound / ghost-breaker) must not keep writing into the OLD
        // conversation's mirror file — that corrupted A.jsonl with B's
        // transcript and left B with no mirror at all.
        if (this.conversationId && this.conversationId !== id && this.mirror) {
          void this.mirror.close()
          this.mirror = null
          this.mirrorHead = null
        }
        this.conversationId = id
        this.ensureMirror()
      },
      scheduleReconnect: () => this.scheduleReconnect(),
      resetReconnectAttempt: () => { this.armStabilityReset() },
      noteSessionCreated: () => { this.noteSessionCreated() },
      noteSessionExit: () => { this.noteSessionExit() },
      noteResumeNotFound: () => {
        this.conversationId = null
        this.rapidResumeExits = 0
        this.mirrorHead = null
        void this.mirror?.reset()
        void vscode.window.showWarningMessage(
          'Conversation not found on this server — started a fresh one.',
        )
      },
      clearJsonlCache: () => {
        this.cachedJsonlEntries = []
        this.cachedSubagentEntries.clear()
        this.cachedJsonlStatus = null
        this.cachedStreamingEntry = null
      },
      noteTranscriptFingerprint: (head) => {
        // Records only start being mirrored once we know WHICH file they belong
        // to — a copy with no identity could never be proven a prefix later, so
        // writing it would be wasted disk.
        this.mirrorHead = head
        this.ensureMirror()
        // The history that streamed BEFORE this point (a resumed session's whole
        // transcript) was never mirrored — the append path was gated on this very
        // head. Write it down now, in file order, so the windowed store can
        // scroll it back after eviction instead of losing it. Idempotent on a
        // lived-from-start session: its first replay-complete has an empty cache,
        // so this no-ops and live appends build the mirror as before.
        if (this.cachedJsonlEntries.length > 0) {
          void this.mirror?.rewriteInOrder(this.cachedJsonlEntries as { _pos?: number; _posEnd?: number; uuid?: string }[], head)
        }
      },
      resetTranscriptMirror: () => {
        this.mirrorHead = null
        void this.mirror?.reset()
      },
      resolveDownload: (r) => {
        if (this._downloadTimeout) {
          clearTimeout(this._downloadTimeout)
          this._downloadTimeout = null
        }
        if (this._downloadResolve) {
          this._downloadResolve(r)
          this._downloadResolve = null
        }
      },
      beginDownload: (total) => {
        this._downloadParts = []
        this.window.webContents.send('jsonl:download-progress', this.tabId, { received: 0, total })
      },
      appendDownloadChunk: (chunk, sent, total) => {
        this._downloadParts?.push(chunk)
        this.window.webContents.send('jsonl:download-progress', this.tabId, { received: sent, total })
      },
      endDownload: (fileName) => {
        if (this._downloadTimeout) {
          clearTimeout(this._downloadTimeout)
          this._downloadTimeout = null
        }
        // join() once at the end: the parts were never concatenated per chunk,
        // so the transcript is materialised exactly once instead of growing a
        // string by repeated re-allocation.
        const content = this._downloadParts ? this._downloadParts.join('') : null
        this._downloadParts = null
        if (this._downloadResolve) {
          this._downloadResolve({ content, fileName })
          this._downloadResolve = null
        }
      },
      pushJsonlCache: (entries, replay) => {
        this.cachedJsonlEntries.push(...entries)
        // Persist as they arrive. Never a bulk dump: an append-only mirror can
        // be resumed after a crash, and writing the whole transcript at once is
        // exactly the stall this is meant to remove.
        //
        // ТОЛЬКО живой хвост: реплейные батчи (replay:true) приходят вне
        // файла-порядка (tail-preview и emitRangeFromEnd шлют ХВОСТ первым) —
        // append по ним прыгал курсором на конец файла, и всё, что реплей не
        // успел дослать до следующего рестарта PTY, навсегда отсекалось
        // фильтром «_pos >= cursor» (прод: «моё сообщение не отрисовалось»,
        // дыра строк 7092-7116). Историю зеркалу отдаёт rewriteInOrder на
        // replayComplete — целиком и в порядке файла.
        if (!replay && this.mirrorHead) void this.mirror?.append(entries as { _pos?: number; uuid?: string }[], this.mirrorHead)
      },
      getCachedJsonl: () => this.cachedJsonlEntries,
      setCachedStreamingEntry: (e) => { this.cachedStreamingEntry = e },
      pushSubagentCache: (key, agentName, agentId, entries) => {
        const slot = this.cachedSubagentEntries.get(key) ?? { agentName, agentId, entries: [] }
        slot.entries.push(...entries)
        // Крышка на слот (аналог trimJsonlCache для основного кэша): реплей
        // повторяется на каждый resync, а чистка была ТОЛЬКО на реконнекте
        // (clearJsonlCache) — долгая сессия с многими сабагентами копила все
        // их записи в памяти хоста без предела (главный накопитель 1.2GB).
        if (slot.entries.length > ConnectionManager.SUBAGENT_CACHE_CAP) {
          slot.entries.splice(0, slot.entries.length - ConnectionManager.SUBAGENT_CACHE_CAP)
        }
        this.cachedSubagentEntries.set(key, slot)
      },
      setCachedJsonlStatus: (s) => { this.cachedJsonlStatus = s },
      isRendererMuted: () => this.rendererMuted,
      setSegmentIndex: (index) => { this.lastSegmentIndex = index },
      isIntentionallyDisconnected: () => this.intentionalDisconnect,
      notifyAuthenticated: () => ConnectionManager.onAuthenticated?.(this),
      notifyElicitationRequest: (p) => ConnectionManager.onElicitationRequest?.(this.tabId, p),
      notifySessionTitle: (t) => ConnectionManager.onSessionTitle?.(this.tabId, t),
      notifyConversationId: (id) => ConnectionManager.onConversationId?.(this.tabId, id),
      notifySessionInfo: (effort, model) => {
        // Снимок для claude-bridge.getSessionInfo (тест-команды).
        if (effort) this.lastEffort = effort
        if (model) this.lastModel = model
        ConnectionManager.onSessionInfo?.(this.tabId, effort, model)
      },
      notifyTreeUpdate: (tree) => {
        if (ConnectionManager.enrichTree) ConnectionManager.enrichTree(tree)
        ConnectionManager.lastTree = tree
        this.window.webContents.send('tree-update', tree)
      },
      trackActivityForIdle: (rawTitle, isWorking) => this.idleTracker.track(rawTitle, isWorking),
      noteReplayCompleteForIdle: () => this.idleTracker.noteReplayComplete(),
      notifyActivity: (working, hookDriven) => ConnectionManager.onActivity?.(this.tabId, working, hookDriven),
      handleMcpCall: (m) => { void runMcpCall(this.mcpCallCtx(), m) },
      handleHookExecute: (m) => { void runHookExecute(this.mcpCallCtx(), m) },
    })
  }

  /** Build the McpCallCtx bag once — wraps the bits of class state the
   *  pipeline helpers need to peek at. */
  private mcpCallCtx(): McpCallCtx {
    return {
      window: this.window,
      tabId: this.tabId,
      send: (m) => this.send(m),
      pendingMcpCalls: this.pendingMcpCalls,
      pendingPermissions: this.pendingPermissions,
      permissionMode: () => this.permissionMode,
      permissionManager: this.permissionManager,
      denyMcp: (id, reason) => this.denyMcp(id, reason),
    }
  }

  /** Send a message to the server via WebSocket. Only sends if the
   *  connection is open. */
  private send(msg: ClientMessage): void {
    if (this.ws?.readyState === WebSocket.OPEN) {
      this.ws.send(JSON.stringify(msg))
    }
  }

  /** Send a raw JSON message (for server commands like session:delete-data) */
  sendRaw(msg: Record<string, unknown>): void {
    if (this.ws?.readyState === WebSocket.OPEN) {
      this.ws.send(JSON.stringify(msg))
    }
  }

  /** Update internal state and notify renderer via IPC. */
  private setState(state: ExtendedConnectionState): void {
    this.state = state
    // Session confirmed — the establish watchdog did its job; disarm + reset.
    if (state.status === 'authenticated') { this.clearEstablishTimer(); this.establishFailures = 0 }
    const rendererState: ConnectionState = {
      status: state.status === 'authenticated' ? 'connected'
        : state.status === 'connected' ? 'connecting'
        : state.status as ConnectionState['status'],
      sessionId: state.sessionId,
      error: state.error,
    }
    this.window.webContents.send('connection-state-changed', this.tabId, rendererState)
    ConnectionManager.onStatus?.(this.tabId, rendererState.status)
  }

  /** Schedule reconnect with exponential backoff. 1s, 2s, 4s, 8s, 16s, 30s. */
  private scheduleReconnect(): void {
    if (this.intentionalDisconnect) return
    if (this.reconnectTimer) return

    const delay = Math.min(
      RECONNECT_BASE_MS * Math.pow(RECONNECT_MULTIPLIER, this.reconnectAttempt),
      RECONNECT_MAX_MS,
    )
    this.reconnectAttempt++

    this.setState({
      status: 'connecting',
      error: `Reconnecting... (attempt ${this.reconnectAttempt}, ${Math.round(delay / 1000)}s)`,
      // The UI counts down to this instead of reprinting the delay: the text
      // above is already stale by the time anyone reads it, so a long backoff
      // was indistinguishable from a stalled one.
      nextRetryAt: Date.now() + delay,
      retryAttempt: this.reconnectAttempt,
    })

    this.reconnectTimer = setTimeout(() => {
      this.reconnectTimer = null
      if (!this.intentionalDisconnect && this.config.token) {
        this.connect()
      }
    }, delay)
  }

  private cancelReconnect(): void {
    if (this.reconnectTimer) {
      clearTimeout(this.reconnectTimer)
      this.reconnectTimer = null
    }
  }
}

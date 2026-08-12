// ============================================================================
// PTY Session Manager core — spawns and manages Claude CLI sessions
// ============================================================================

import { randomUUID } from 'crypto'
import fs from 'fs'
import os from 'os'
import path from 'path'
import type { WebSocket as WS } from 'ws'
import type { PtySession, SessionConfig } from './types'
import { DEFAULT_SESSION_MODEL } from '../config'
import { eventBus } from '../events/bus'
import { debugLog, warnLog, infoLog } from '../logging'
import { JsonlWatcher } from './jsonl-watcher'
import { leanEntries } from './jsonl-projection'
import {
  sendToClient,
  writeInputToSession,
  submitTextToSession,
  resizeSessionTerminal,
  attachStartupAutoResponder,
  attachOutputDebounce,
  clearInputThrottle,
} from './session-io'
import { clearSessionInputState, notifySessionAttachmentChanged } from './session-input-coordinator'
import { clearSession as clearHookSession, cancelSessionLocalExecs } from '../hooks'
import {
  buildSessionEnv as buildEnv,
  buildClaudeArgs,
} from './session-env'
import {
  SESSIONS_BASE,
  sanitizeDirName,
  writeSessionSettings,
  writeSessionClaudeMd,
  applySyncData,
} from './session-settings'
import { findOrRecreateSettingsDir, xbasename, repairTranscriptForResume, resolveNewestInChain } from './session-resume-helpers'
import { handleJsonlUserEntry } from './session-stats-recorder'
import { resetJsonlStats, accumulateJsonlStats } from './session-jsonl-stats'
import {
  pendingMcpCalls,
  sendMcpCall as sendMcpCallToSession,
  rejectAllPending,
  rejectPendingForSession,
  resendUndeliveredMcpCalls,
} from './session-mcp-call'
import { startNativeMitm } from '../proxy/native-mitm'
import { getStreamingSettings } from '../proxy/streaming-settings'
import { withSyncSnapshotLock } from '../sync/lock'

// Re-export the per-session MCP-call helpers so callers that historically
// imported them from session-core/session-manager keep working.
export { handleMcpResponse, handleMcpDenied } from './session-mcp-call'

// Re-export `sendToClient` for other modules in this package that pulled it
// from session-manager historically.
export { sendToClient }

// ---------------------------------------------------------------------------
// State (shared across this package)
// ---------------------------------------------------------------------------

export const sessions = new Map<string, PtySession>()

// Global backstop across ALL users. The per-user cap (10, enforced in
// session-ws) bounds one token, but nothing bounded the TOTAL — N users could
// multiply PTY processes + proxy ports + fs watchers + undici pools until the
// box ran out of PIDs / file descriptors. Generous so it only trips under
// genuine overload, never in normal single-user desktop use. Overridable.
const GLOBAL_MAX_SESSIONS = Number(process.env.BRIDGE_MAX_SESSIONS) || 200

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/**
 * Create a new PTY session bound to a WebSocket client.
 * Spawns `claude` CLI with per-session settings.json containing MCP config.
 */
export async function createSession(
  ws: WS,
  userName: string,
  tokenId: string,
  config: SessionConfig = {}
): Promise<PtySession> {
  // Global backstop — reject BEFORE spawning anything. Reattach (resume of a
  // live PTY) never reaches here, so a reconnecting client is unaffected.
  if (sessions.size >= GLOBAL_MAX_SESSIONS) {
    throw new Error(`Server session limit reached (${GLOBAL_MAX_SESSIONS}); try again later`)
  }
  const sessionId = randomUUID()
  const mcpToken = randomUUID() // per-session secret for MCP endpoint auth

  // Клиент мог принести УСТАРЕВШИЙ conversationId (авто-compact сменил файл,
  // пока клиент был отключён — уведомление ушло в закрытый сокет). Резюм
  // старого id воскрешал скомпакченный файл, и чат «откатывался». Идём по
  // персист-цепочке компактов до свежайшего id.
  if (config.resumeConversationId) {
    const tip = followCompactLinks(config.resumeConversationId)
    if (tip !== config.resumeConversationId) {
      infoLog('Resume: following compact chain', { requested: config.resumeConversationId, tip })
      config.resumeConversationId = tip
    }
  }

  // Reuse existing settingsDir when restarting (model/effort change with --resume).
  // This preserves the same project slug so Claude CLI can find the JSONL file.
  let settingsDir: string
  let resumeNotFound = false
  if (config.reuseSettingsDir) {
    settingsDir = config.reuseSettingsDir
    // Same repair as the find-path below — the restart path used to skip it,
    // so a transcript corrupted mid-turn kept 400-ing after model/effort change.
    if (config.resumeConversationId) {
      repairTranscriptForResume(settingsDir, config.resumeConversationId)
    }
  } else if (config.resumeConversationId) {
    // Resume from UI: find or recreate the settingsDir whose slug matches
    // the one in ~/.claude/projects/ containing <conversationId>.jsonl.
    const found = findOrRecreateSettingsDir(config.resumeConversationId, tokenId)
    if (found) {
      settingsDir = found
      debugLog('Resume: using settingsDir for conversationId', { settingsDir, conversationId: config.resumeConversationId })
    } else {
      // The conversation JSONL exists in NO slug dir — it's gone (expired /
      // deleted / never on this server / different machine). Passing
      // `--resume <id>` here makes the CLI exit code 1 ("No conversation found"),
      // the client reconnects and re-resumes the SAME ghost id, and it blinks
      // forever (Reconnecting…). Start a FRESH session instead: drop the resume
      // so no `--resume` is passed. The new session mints a new conversationId,
      // the JSONL watcher reports it via `session:conversation-id`, and the
      // client adopts it — the loop can't recur.
      const folderSuffix = config.cwd ? '-' + sanitizeDirName(config.cwd) : ''
      settingsDir = path.join(SESSIONS_BASE, tokenId + folderSuffix, sessionId)
      warnLog('Resume: conversation not found anywhere — starting a FRESH session', { conversationId: config.resumeConversationId })
      config.resumeConversationId = undefined
      resumeNotFound = true
    }
  } else {
    // Each session gets its own unique directory to avoid JSONL cross-talk.
    const folderSuffix = config.cwd ? '-' + sanitizeDirName(config.cwd) : ''
    settingsDir = path.join(SESSIONS_BASE, tokenId + folderSuffix, sessionId)
  }

  // Финальная страховка резюма: даже с целой картой compact-links выбранный id
  // мог отстать от реального tip цепочки (watcher остановлен до смерти CLI,
  // гонка записи карты, двойной писатель). CLI с отставшим id форкает файл от
  // старой точки — «после перезагрузки пропал кусок чата и консоли». Резюмим
  // самый свежий файл цепочки — ровно тот, что будет тейлить watcher.
  if (config.resumeConversationId) {
    const newest = resolveNewestInChain(settingsDir, config.resumeConversationId)
    if (newest && newest !== config.resumeConversationId) {
      infoLog('Resume: newer file in chain — resuming tip', { requested: config.resumeConversationId, tip: newest })
      recordCompactLink(config.resumeConversationId, newest) // самолечение карты
      config.resumeConversationId = newest
      repairTranscriptForResume(settingsDir, newest) // ремонт по ФИНАЛЬНОМУ id
    }
  }

  // Create session directory + settings (overwrites existing on restart)
  fs.mkdirSync(settingsDir, { recursive: true })
  writeSessionSettings(sessionId, settingsDir, mcpToken, config.model, config.cwd, config.bearerHash)

  // Write CLAUDE.md with user's real working directory info
  const userCwd = config.cwd
  if (userCwd) {
    writeSessionClaudeMd(settingsDir, userCwd)
  }

  // Apply per-token synced data (skills, agents, CLAUDE.md, project files)
  if (config.bearerHash) {
    await withSyncSnapshotLock(config.bearerHash, () => {
      applySyncData(settingsDir, config.bearerHash!, userCwd)
    })
  }

  debugLog('Creating PTY session', { sessionId, userName, cwd: config.cwd })

  // Dynamic import — node-pty is a native module
  const nodePty = await import('node-pty')

  const env = buildEnv(sessionId, userName, config.effort)
  const args = buildClaudeArgs(config)

  let streamThrottle: Map<string, { timer: NodeJS.Timeout; latest: any }> | null = null
  // ── Live streaming MITM proxy (per-token toggle).
  // When enabled, we spawn a per-session proxy on an ephemeral port and
  // point the CLI's HTTPS_PROXY at it. Anthropic SSE responses are tee'd
  // into our entry-synthesizer → fanned out via WS as `streaming:entry`
  // messages. JsonlViewer renders the live entry until JSONL watcher
  // delivers the final one with the same message.id.
  let streamingProxy: { port: number; caCertPath: string; stop: () => Promise<void>; interrupt: () => void } | undefined
  try {
    const settings = await getStreamingSettings(tokenId)
    if (settings.enabled) {
      // Chain through any pre-existing corporate proxy. Priority:
      //   1. Bridge's own dashboard settings (DuckDB) — set via the UI's
      //      Network/Proxy panel. These are the values users actually
      //      configure on prod ("сохранил в Settings").
      //   2. Process env (HTTPS_PROXY etc) — for docker-compose-style setups.
      // Without this look-up, MITM tls.connect goes direct to api.anthropic.com
      // from inside a corp network → ECONNRESET / TLS RST.
      const { getSetting } = await import('../config/settings')
      const upstream = getSetting('httpsProxy')
        || getSetting('httpProxy')
        || process.env.https_proxy || process.env.HTTPS_PROXY
        || process.env.http_proxy || process.env.HTTP_PROXY
      // Per-user visibility filters, shared by the old-client (onEntryUpdate)
      // and new-client (onSnapshot) paths so both honor showSideQuests /
      // showThinkingStream identically.
      const suppressStreamEntry = (entry: any): boolean => {
        if (entry.isSidechain && !settings.showSideQuests) return true
        if (!settings.showThinkingStream) {
          const onlyThinking = entry.message.content.length > 0
            && entry.message.content.every((b: any) => b.type === 'thinking' || b.type === 'redacted_thinking')
          if (onlyThinking && entry.__streaming) return true
        }
        return false
      }
      const proxy = await startNativeMitm({
        ptySessionId: sessionId,
        cliConversationId: config.resumeConversationId ?? undefined,
        upstreamProxyUrl: upstream && !upstream.startsWith('http://127.0.0.1') ? upstream : undefined,
        // Without the user's chosen model, the side-quest heuristic classifies
        // every Haiku main-thread turn as a side-quest and filters it out →
        // Haiku users saw no streaming at all.
        mainModel: config.model,
        onStatus: (info) => sendToClient(session.ws, { type: 'streaming:status', sessionId, ...info }),
        // NEW-client incremental path (protocolVersion>=1): a full entry only at
        // structural boundaries + tiny text/thinking appends between them. Sent
        // IMMEDIATELY (no throttle) so a delta never overtakes the boundary
        // snapshot that opened its block. Old clients (streamProtocol 0) get
        // nothing here and fall back to the throttled onEntryUpdate below.
        onSnapshot: (entry) => {
          if ((session.streamProtocol ?? 0) < 1) return
          if (suppressStreamEntry(entry)) return
          sendToClient(session.ws, { type: 'streaming:entry', sessionId, entry })
        },
        onDelta: (d) => {
          if ((session.streamProtocol ?? 0) < 1) return
          sendToClient(session.ws, { type: 'streaming:delta', sessionId, ...d })
        },
        onEntryUpdate: (entry, _isSideQuestFlag) => {
          // New clients drive the chat off onSnapshot+onDelta; skip the
          // per-tick full snapshot entirely for them.
          if ((session.streamProtocol ?? 0) >= 1) return
          if (suppressStreamEntry(entry)) return
          const isSideQuest = !!entry.isSidechain
          // Throttle streaming pushes: 100ms between sends per message-id.
          // Without this, ~30 sends/sec for a single turn collide in one
          // microtask flush of the bridge↔Electron WS, and the client
          // observably sees only 2 of 37 frames (start + finalize). The
          // coalescer keeps "latest entry" buffered and emits at most
          // 10 fps — visually smooth, well within ws buffer limits.
          // Final (`!__streaming`) bypasses the throttle and flushes any
          // pending buffered entry first.
          ;(session as any)._wsPushCount = ((session as any)._wsPushCount ?? 0) + 1
          const c = (session as any)._wsPushCount as number
          if (entry.message.content.length === 0 || !entry.__streaming || c % 25 === 0) {
            infoLog('[streaming] WS push', {
              sessionId, seq: c, messageId: entry.message.id,
              streaming: entry.__streaming, contentLen: entry.message.content.length,
              sidechain: isSideQuest,
              firstTextLen: (entry.message.content.find((b: any) => b.type === 'text') as any)?.text?.length ?? 0,
            })
          }

          if (!streamThrottle) streamThrottle = new Map()
          const msgId = entry.message.id
          // NB: send via session.ws (not the captured ws) — the client may have
          // reattached on a fresh WebSocket after a network blip.
          if (!entry.__streaming) {
            // Final entry — clear timer and emit synchronously.
            const slot = streamThrottle.get(msgId)
            if (slot?.timer) clearTimeout(slot.timer)
            streamThrottle.delete(msgId)
            sendToClient(session.ws, { type: 'streaming:entry', sessionId, entry })
            return
          }
          const slot = streamThrottle.get(msgId)
          if (slot) {
            // Already pending — overwrite buffered entry; don't reset timer.
            slot.latest = entry
            return
          }
          // First update for this messageId — send IMMEDIATELY (so the bubble
          // appears right away), then start a 100ms throttle window.
          sendToClient(session.ws, { type: 'streaming:entry', sessionId, entry })
          const timer = setTimeout(() => {
            const cur = streamThrottle!.get(msgId)
            streamThrottle!.delete(msgId)
            // Flush the FRESHEST buffered entry. (Bug fix: the old code called a
            // sendNow() closure that re-read streamThrottle.get(msgId) AFTER the
            // delete above — always undefined — and fell back to the first-of-
            // window `entry`, so every window re-sent a stale duplicate and the
            // buffered deltas were dropped.)
            if (cur?.latest) sendToClient(session.ws, { type: 'streaming:entry', sessionId, entry: cur.latest })
          }, 100)
          streamThrottle.set(msgId, { timer, latest: entry })
        },
      })
      streamingProxy = proxy
      env.HTTPS_PROXY = `http://127.0.0.1:${proxy.port}`
      env.HTTP_PROXY = `http://127.0.0.1:${proxy.port}`
      env.https_proxy = env.HTTPS_PROXY
      env.http_proxy = env.HTTP_PROXY
      env.NODE_EXTRA_CA_CERTS = proxy.caCertPath
      env.CLAUDE_CODE_PROXY_RESOLVES_HOSTS = '1'
      // KEEP NO_PROXY=127.0.0.1,localhost,::1 (set by injectProxyEnv) so the
      // CLI's MCP / OTLP calls to our bridge HTTP server don't loop through
      // the streaming proxy. api.anthropic.com is NOT in NO_PROXY so it
      // still routes through us.
      const lo = '127.0.0.1,localhost,::1'
      const parts = [env.NO_PROXY, env.no_proxy, lo].filter(Boolean)
      const merged = [...new Set(parts.join(',').split(',').map(s => s.trim()).filter(Boolean))].join(',')
      env.NO_PROXY = merged
      env.no_proxy = merged
      infoLog('[streaming] proxy started', { sessionId, port: proxy.port, upstream: upstream || 'direct' })
    } else {
      infoLog('[streaming] disabled for token (skipping proxy)', { sessionId, tokenId })
    }
  } catch (err) {
    warnLog('[streaming] proxy failed to start (CLI continues without live capture)', { sessionId, error: String(err) })
  }

  // ALWAYS use settingsDir as cwd on the server.
  // User's real path (config.cwd) is written to CLAUDE.md for context only.
  // Actual file operations go through MCP → Electron → user's machine.
  const cwd = settingsDir

  // On Windows, node-pty needs .cmd extension for npm global binaries
  const claudeCmd = process.platform === 'win32' ? 'claude.cmd' : 'claude'

  let pty: ReturnType<typeof import('node-pty').spawn>
  try {
    pty = nodePty.spawn(claudeCmd, args, {
      name: 'xterm-256color',
      cols: config.cols ?? 120,
      rows: config.rows ?? 40,
      cwd,
      env,
    })
  } catch (err) {
    // Spawn failed (claude binary missing, cwd gone, EAGAIN). The streaming
    // proxy was started above and would otherwise leak its listening socket +
    // dispatcher; a freshly-created settingsDir would leak too. Clean both up
    // before propagating so a spawn failure can't accrete zombie proxies/dirs.
    if (streamingProxy) { void streamingProxy.stop().catch(() => { /* swallow */ }) }
    if (!config.reuseSettingsDir) cleanupSessionDir(settingsDir)
    throw err
  }

  const session: PtySession = {
    id: sessionId,
    pty,
    ws,
    userName,
    tokenId,
    settingsDir,
    cwd: config.cwd || '',
    state: 'running',
    createdAt: new Date(),
    lastActivityAt: new Date(),
    mcpCallCount: 0,
    inputCount: 0,
    userMessages: 0,
    assistantMessages: 0,
    contextTokens: 0,
    totalTokens: 0,
    mcpLog: [],
    mcpInitialized: false,
    mcpLastError: null,
    mcpToken,
    outputBuffer: [],
    outputBufferBytes: 0,
    lastResizeAt: Date.now(),
    consoleShrunk: false,
    registeredTools: [],
    registeredResources: [],
    registeredResourceTemplates: [],
    registeredPrompts: [],
    cliConversationId: config.resumeConversationId || null,
    isSubAgent: config.isSubAgent || false,
    parentSessionId: config.parentSessionId || null,
    childSessions: [],
    pendingElicitations: new Map(),
    sessionTitle: null,
    isRestarting: false,
    effort: config.effort || 'high',
    model: config.model || DEFAULT_SESSION_MODEL,
    streamingProxy,
    bearerHash: config.bearerHash,
    streamProtocol: config.protocolVersion ?? 0,
    resumeNotFound,
  }

  sessions.set(sessionId, session)

  // Auto-answer interactive startup prompts:
  // 1. "Yes, I trust this folder" (Enter to confirm — option 1 is default)
  // 2. Effort level selection (already skipped via CLAUDE_CODE_EFFORT_LEVEL env)
  attachStartupAutoResponder(session)

  // Wire PTY output → WS with debounce (same pattern as terminal.ts)
  attachOutputDebounce(session)

  // Start JSONL watcher for real-time session log streaming + input counting
  // Pass settingsDir so the watcher only looks in the correct slug directory,
  // not in the entire ~/.claude/projects/ (which would pick up unrelated conversations)
  // When resuming, use epoch 0 so watcher finds the existing JSONL file (its mtime is in the past)
  const watcherStartTime = config.resumeConversationId ? 0 : Date.now()
  // All watcher callbacks send via session.ws (not the captured ws) so a
  // reattached client keeps receiving entries.
  const jsonlWatcher = new JsonlWatcher(
    watcherStartTime,
    // `replay: true` — батч истории/превью вне файла-порядка: клиент не должен
    // двигать по нему курсор зеркала (прод-дыра «сообщение не отрисовалось»).
    (entries, replay) => sendToClient(session.ws, { type: 'jsonl:entries', entries: leanEntries(entries), ...(replay ? { replay: true } : {}) }),
    (status) => sendToClient(session.ws, { type: 'jsonl:status', ...status }),
    (entry) => handleJsonlUserEntry(session, entry),
    settingsDir,
    (conversationId) => {
      // Смена id (авто-compact → новый файл): помнить ПРЕЖНИЕ id — клиент,
      // перезапустившийся со старым id из metadata, обязан находить эту живую
      // сессию (finder ниже матчит и по alias'ам); плюс персист линка
      // old→new, чтобы холодный резюм после смерти PTY дошёл до свежего
      // файла, а не воскрешал скомпакченный (инцидент «откат чата»).
      const prev = session.cliConversationId
      if (prev && prev !== conversationId) {
        ;(session.conversationAliases ??= []).push(prev)
        recordCompactLink(prev, conversationId)
      }
      session.cliConversationId = conversationId
      debugLog('JSONL conversationId discovered', { sessionId, conversationId })
      sendToClient(session.ws, { type: 'session:conversation-id', sessionId, conversationId })
      // Map JSONL session → PTY token so the JSONL sweeper can attribute
      // jsonl_events rows to the right API token (Stats panel + per-token
      // session list rely on this). DuckDB is async — wrap in a fire-and-
      // forget IIFE so we don't change the JsonlWatcher callback signature.
      void (async () => {
        try {
          const { getDb } = await import('../stats/database/lifecycle')
          const db = await getDb()
          const realCwd = config.cwd || null
          const titleAtInsert = session.sessionTitle || null
          await db.run(
            `INSERT INTO session_tokens (session_id, token_id, user_name, cwd, title, created_at, deleted)
             VALUES (?, ?, ?, ?, ?, ?, 0)
             ON CONFLICT (session_id) DO NOTHING`,
            [conversationId, tokenId, userName, realCwd, titleAtInsert, new Date().toISOString()],
          )
          await db.run(
            `UPDATE session_tokens
             SET token_id = ?,
                 user_name = ?,
                 cwd = COALESCE(NULLIF(cwd, ''), ?),
                 title = COALESCE(NULLIF(title, ''), ?)
             WHERE session_id = ? AND (token_id IS NULL OR token_id = '')`,
            [tokenId, userName, realCwd, titleAtInsert, conversationId],
          )
        } catch (err) {
          warnLog('session_tokens insert failed', { conversationId, error: String(err) })
        }
      })()
    },
    (agentName, entries, agentId) => sendToClient(session.ws, { type: 'jsonl:subagent-entries', agentName, agentId, entries: leanEntries(entries) }),
    () => resetJsonlStats(session),
    (entry) => accumulateJsonlStats(session, entry),
  )
  session.jsonlWatcher = jsonlWatcher
  jsonlWatcher.start()

  // PTY exit handler
  pty.onExit(({ exitCode }: { exitCode: number }) => {
    // Skip exit handling during effort-change restart
    if (session.isRestarting) {
      debugLog('PTY exited during restart (suppressed)', { sessionId, exitCode })
      return
    }
    debugLog('PTY exited', { sessionId, exitCode })
    session.state = 'exited'
    sendToClient(session.ws, { type: 'session:exit', code: exitCode, sessionId })
    cleanupSession(sessionId)
    eventBus.emit('session:destroyed', { sessionId, tokenId, userName, exitCode })
  })

  eventBus.emit('session:created', {
    sessionId,
    tokenId,
    userName,
    cwd: config.cwd || '',
    state: 'running' as const,
    createdAt: session.createdAt.toISOString(),
    lastActivityAt: session.lastActivityAt.toISOString(),
    mcpCallCount: 0,
    inputCount: 0,
  })
  debugLog('PTY session created', { sessionId, pid: pty.pid })

  return session
}

// How long a session survives after its client WS drops without an explicit
// session:end. Killing the CLI the instant the socket closed is what used to
// corrupt conversation JSONL (mid-write kill + instant-reconnect double
// writer) — see task #44.
const DETACH_GRACE_MS = Number(process.env.OCB_DETACH_GRACE_MS || 10 * 60 * 1000)
// After a polite kill signal, how long we give the CLI to flush + exit
// before escalating to SIGKILL.
const KILL_ESCALATE_MS = 3000

/**
 * Wait for the session's PTY to exit, escalating to SIGKILL after
 * KILL_ESCALATE_MS. Resolves once the process is gone (or after the
 * post-SIGKILL grace) — callers use this to guarantee the old CLI stopped
 * writing its JSONL before a new `--resume` CLI starts on the same file.
 */
function killPtyAndWait(session: PtySession): Promise<void> {
  return new Promise<void>((resolve) => {
    let settled = false
    const finish = () => { if (!settled) { settled = true; clearTimeout(escalate); clearTimeout(hardStop); resolve() } }
    const sub = session.pty.onExit(() => { sub.dispose(); finish() })
    const escalate = setTimeout(() => {
      try { session.pty.kill('SIGKILL') } catch { /* already gone */ }
    }, KILL_ESCALATE_MS)
    // SIGKILL cannot be ignored — if onExit still hasn't fired shortly after,
    // the process is gone and node-pty just lost the event; don't hang forever.
    const hardStop = setTimeout(finish, KILL_ESCALATE_MS + 1000)
    try { session.pty.kill() } catch { finish() }
  })
}

/**
 * Client WS dropped without an explicit session:end — keep the CLI alive for
 * a grace window awaiting reattach (network blip, app restart). The reaper
 * still covers long-abandoned sessions.
 */
export function detachSession(sessionId: string): void {
  const session = sessions.get(sessionId)
  if (!session) return
  if (session.state !== 'running') { destroySession(sessionId); return }
  if (session.detachGraceTimer) return // already detached

  session.detachedAt = new Date()
  notifySessionAttachmentChanged(session)
  session.detachGraceTimer = setTimeout(() => {
    debugLog('Detach grace expired — destroying session', { sessionId })
    destroySession(sessionId)
  }, DETACH_GRACE_MS)
  debugLog('Session detached (client WS closed), awaiting reattach', {
    sessionId, graceMs: DETACH_GRACE_MS,
  })
}

/**
 * Bind a fresh client WS to a live session (reattach after a network blip).
 * All send paths read session.ws dynamically, so swapping it is enough for
 * output/jsonl/streaming to follow. Pending elicitation widgets are re-sent,
 * and the CLI gets a resize nudge so its TUI repaints for the new client.
 */
export function reattachSession(session: PtySession, ws: WS): void {
  if (session.detachGraceTimer) {
    clearTimeout(session.detachGraceTimer)
    session.detachGraceTimer = null
  }
  session.detachedAt = null
  session.ws = ws
  session.lastActivityAt = new Date()
  notifySessionAttachmentChanged(session)

  for (const [requestId, pe] of session.pendingElicitations) {
    sendToClient(ws, {
      type: 'elicitation:request',
      requestId,
      toolName: pe.toolName,
      message: pe.message,
      requestedSchema: pe.requestedSchema,
    })
  }
  resendUndeliveredMcpCalls(session)

  // Instant console restore: replay the buffered raw PTY output so the new
  // client's xterm reconstructs the CURRENT screen immediately (it parses the
  // escape sequences — alt-screen clears/repaints land on the right final
  // state). Without this the console stayed blank until the resize-nudge below
  // made the CLI repaint — most visible after the bridge-shim hard-reload on WS
  // reconnect, which mounts a fresh empty xterm. The nudge still fires as an
  // authoritative refresh a moment later.
  if (session.outputBuffer.length > 0) {
    sendToClient(ws, { type: 'session:output', data: session.outputBuffer.join('\n') })
  }

  try {
    const { cols, rows } = session.pty
    if (cols > 2) {
      session.pty.resize(cols - 1, rows)
      setTimeout(() => { try { session.pty.resize(cols, rows) } catch { /* exited */ } }, 150)
    }
  } catch { /* exited */ }

  debugLog('Session reattached to new client WS', { sessionId: session.id })
}

/**
 * Find a live session for this token that already runs the given
 * conversation — the reattach target for session:resume.
 */
export function findSessionByConversation(tokenId: string, conversationId: string): PtySession | undefined {
  for (const s of sessions.values()) {
    if (s.tokenId !== tokenId || s.state !== 'running') continue
    // Матч и по ПРЕЖНИМ id (авто-compact сменил файл, клиент мог перезапуститься
    // со старым id из metadata — живую сессию нельзя терять: холодный резюм
    // старого id воскрешал скомпакченный файл, «откат чата»).
    if (s.cliConversationId === conversationId || s.conversationAliases?.includes(conversationId)) return s
  }
  return undefined
}

// ── Персист цепочки компактов: old conversationId → new ─────────────────
// Живёт дольше PTY: после смерти сессии холодный резюм по старому id обязан
// дойти до新ейшего файла цепочки.
const COMPACT_LINKS_PATH = path.join(os.homedir(), '.claude', 'open-claude-bridge', 'compact-links.json')
const COMPACT_LINKS_CAP = 500

export function recordCompactLink(oldId: string, newId: string): void {
  try {
    let links: Record<string, string> = {}
    try { links = JSON.parse(fs.readFileSync(COMPACT_LINKS_PATH, 'utf-8')) as Record<string, string> } catch { /* first write */ }
    links[oldId] = newId
    const keys = Object.keys(links)
    if (keys.length > COMPACT_LINKS_CAP) {
      for (const k of keys.slice(0, keys.length - COMPACT_LINKS_CAP)) delete links[k]
    }
    fs.mkdirSync(path.dirname(COMPACT_LINKS_PATH), { recursive: true })
    // tmp+rename: рестарт приложения резюмит пачку вкладок разом, параллельные
    // read→modify→write затирали рёбра друг друга (потерянное ребро = резюм
    // устаревшего id = дыра в чате). rename атомарен в пределах каталога.
    const tmp = `${COMPACT_LINKS_PATH}.tmp-${process.pid}-${Date.now()}`
    fs.writeFileSync(tmp, JSON.stringify(links))
    fs.renameSync(tmp, COMPACT_LINKS_PATH)
  } catch (err) {
    warnLog('recordCompactLink failed', { err: String(err) })
  }
}

/** Пройти цепочку компактов до свежайшего id (≤10 прыжков, гард циклов). */
export function followCompactLinks(conversationId: string): string {
  try {
    const links = JSON.parse(fs.readFileSync(COMPACT_LINKS_PATH, 'utf-8')) as Record<string, string>
    let current = conversationId
    const seen = new Set<string>([current])
    for (let hop = 0; hop < 10; hop++) {
      const next = links[current]
      if (!next || seen.has(next)) break
      seen.add(next)
      current = next
    }
    return current
  } catch {
    return conversationId
  }
}

/**
 * Destroy a session: kill PTY, clean up temp files.
 */
export function destroySession(sessionId: string): void {
  const session = sessions.get(sessionId)
  if (!session) return

  debugLog('Destroying session', { sessionId })
  if (session.detachGraceTimer) {
    clearTimeout(session.detachGraceTimer)
    session.detachGraceTimer = null
  }
  // Cancel any pending local hook executions waiting on this session's WS,
  // then drop the session's registered hooks so we don't leak entries.
  cancelSessionLocalExecs(sessionId)
  clearHookSession(sessionId)
  session.state = 'exiting'
  session.jsonlWatcher?.stop()

  // Polite kill first; escalate to SIGKILL if the CLI hasn't exited in time
  // (it flushes its JSONL on the way out — a hard kill here is what used to
  // tear transcripts).
  try { session.pty.kill() } catch {}
  setTimeout(() => {
    if (session.state !== 'exited') {
      try { session.pty.kill('SIGKILL') } catch { /* already gone */ }
    }
  }, KILL_ESCALATE_MS)
  // Watcher уже остановлен, а CLI на выходе мог финализировать НОВЫЙ файл
  // цепочки — без этого скана ребро терялось навсегда и следующий резюм шёл
  // по устаревшему id (дыра в чате). Отложенный скан ловит файл после смерти.
  const chainBase = session.cliConversationId
  const chainDir = session.settingsDir
  if (chainBase && chainDir) {
    setTimeout(() => {
      try {
        const tip = resolveNewestInChain(chainDir, chainBase)
        if (tip && tip !== chainBase) {
          infoLog('Post-kill chain scan: recording missed edge', { from: chainBase, tip })
          recordCompactLink(chainBase, tip)
        }
      } catch { /* best-effort */ }
    }, KILL_ESCALATE_MS + 2000)
  }
  cleanupSession(sessionId)
  eventBus.emit('session:destroyed', { sessionId, tokenId: session.tokenId, userName: session.userName, exitCode: -1 })
}

/**
 * Restart a session with a new effort level.
 * Kills the current PTY (suppressing exit events) and creates a new session
 * that resumes the same conversation with --resume + --effort.
 */
export async function restartWithEffort(
  sessionId: string,
  effort: string,
  ws: WS,
  userName: string,
  tokenId: string,
): Promise<PtySession> {
  const session = sessions.get(sessionId)
  if (!session) throw new Error('Session not found')
  if (!session.cliConversationId) throw new Error('No conversationId — cannot resume')

  const conversationId = session.cliConversationId
  const cwd = session.cwd
  const preservedModel = session.model
  const oldSettingsDir = session.settingsDir

  debugLog('Restarting session with new effort', { sessionId, effort, conversationId })

  // Mark as restarting so onExit handler doesn't send session:exit
  session.isRestarting = true

  // Stop JSONL watcher before killing PTY
  session.jsonlWatcher?.stop()

  // Tear down streaming proxy before respawn — new session will start its own.
  if (session.streamingProxy) {
    void session.streamingProxy.stop().catch(() => {})
    session.streamingProxy = undefined
  }

  // Kill current PTY and WAIT for it to exit — spawning the new --resume CLI
  // while the old one is still flushing used to interleave two writers on
  // the same conversation JSONL (chain forks / torn lines).
  await killPtyAndWait(session)

  // Reject the OLD session's in-flight MCP calls before we drop it — the restart
  // path skips cleanupSession() (isRestarting suppresses onExit), so without this
  // an interactive tool call pending at model/effort-change time would leak in the
  // shared pendingMcpCalls map forever.
  rejectPendingForSession(sessionId, 'Session restarting')
  // Remove from sessions map (without broadcasting exit events)
  sessions.delete(sessionId)

  // Create new session with resume + effort, reusing settingsDir so CLI finds JSONL
  return createSession(ws, userName, tokenId, {
    cwd,
    cols: 120,
    rows: 40,
    resumeConversationId: conversationId,
    effort,
    model: preservedModel || undefined,
    reuseSettingsDir: oldSettingsDir,
    bearerHash: session.bearerHash,
  })
}

/**
 * Restart a session with a new model.
 * Same pattern as restartWithEffort: kills PTY, creates new session with --resume + --model.
 */
export async function restartWithModel(
  sessionId: string,
  model: string,
  ws: WS,
  userName: string,
  tokenId: string,
): Promise<PtySession> {
  const session = sessions.get(sessionId)
  if (!session) throw new Error('Session not found')
  if (!session.cliConversationId) throw new Error('No conversationId — cannot resume')

  const conversationId = session.cliConversationId
  const cwd = session.cwd
  const preservedEffort = session.effort
  const oldSettingsDir = session.settingsDir

  debugLog('Restarting session with new model', { sessionId, model, conversationId })

  // Mark as restarting so onExit handler doesn't send session:exit
  session.isRestarting = true

  // Stop JSONL watcher before killing PTY
  session.jsonlWatcher?.stop()

  // Tear down streaming proxy before respawn — new session will start its own.
  if (session.streamingProxy) {
    void session.streamingProxy.stop().catch(() => {})
    session.streamingProxy = undefined
  }

  // Kill current PTY and WAIT for it to exit — same double-writer guard as
  // restartWithEffort.
  await killPtyAndWait(session)

  // Reject the OLD session's in-flight MCP calls before we drop it — the restart
  // path skips cleanupSession() (isRestarting suppresses onExit), so without this
  // an interactive tool call pending at model/effort-change time would leak in the
  // shared pendingMcpCalls map forever.
  rejectPendingForSession(sessionId, 'Session restarting')
  // Remove from sessions map (without broadcasting exit events)
  sessions.delete(sessionId)

  // Create new session with resume + model, reusing settingsDir so CLI finds JSONL
  return createSession(ws, userName, tokenId, {
    cwd,
    cols: 120,
    rows: 40,
    resumeConversationId: conversationId,
    model,
    effort: preservedEffort || undefined,
    reuseSettingsDir: oldSettingsDir,
    bearerHash: session.bearerHash,
  })
}

/**
 * Get a session by ID (for MCP routing).
 */
export function getSession(sessionId: string): PtySession | undefined {
  return sessions.get(sessionId)
}

/**
 * Get all active sessions.
 */
export function getAllSessions(): PtySession[] {
  return [...sessions.values()]
}

/**
 * Count active sessions for a specific user (by tokenId).
 */
export function countUserSessions(tokenId: string): number {
  let count = 0
  for (const s of sessions.values()) {
    if (s.tokenId === tokenId && s.state === 'running') count++
  }
  return count
}

/**
 * Graceful shutdown: kill all PTY sessions.
 */
export function shutdownAll(): void {
  debugLog('Shutting down all PTY sessions', { count: sessions.size })
  for (const [id, session] of sessions) {
    try { session.pty.kill() } catch {}
    try { session.ws.close() } catch {}
    clearSessionInputState(session)
    cleanupSessionDir(session.settingsDir)
  }
  sessions.clear()

  rejectAllPending('Server shutting down')
}

// ---------------------------------------------------------------------------
// I/O wrappers — preserve the (sessionId: string) public API by delegating
// to session-io's PtySession-taking helpers.
// ---------------------------------------------------------------------------

/**
 * Write PTY stdin data.
 */
export function writeInput(sessionId: string, data: string): void {
  const session = sessions.get(sessionId)
  if (!session) return
  writeInputToSession(session, data)
}

/**
 * Submit a full user message (bracketed paste + quiet-window gated CR).
 */
export function submitText(sessionId: string, text: string): void {
  const session = sessions.get(sessionId)
  if (!session) return
  submitTextToSession(session, text)
}

/**
 * Resize PTY terminal.
 */
export function resizeTerminal(sessionId: string, cols: number, rows: number): void {
  const session = sessions.get(sessionId)
  if (!session) return
  resizeSessionTerminal(session, cols, rows)
}

// ---------------------------------------------------------------------------
// MCP call management (Server → Electron → Server)
// ---------------------------------------------------------------------------

/**
 * Send an MCP tool call to Electron via WebSocket. Resolves when Electron
 * responds (or rejects on timeout/denial).
 */
export function sendMcpCall(
  sessionId: string,
  toolName: string,
  input: Record<string, unknown>,
): Promise<unknown> {
  const session = sessions.get(sessionId)
  if (!session) return Promise.reject(new Error(`Session ${sessionId} not found`))
  return sendMcpCallToSession(session, toolName, input)
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------


function cleanupSession(sessionId: string): void {
  const session = sessions.get(sessionId)
  if (!session) return

  session.jsonlWatcher?.stop()
  // Reject this session's in-flight MCP calls so their promises + any
  // long-lived SSE stream/heartbeat (interactive tools never time out) don't
  // leak after the session is gone.
  rejectPendingForSession(sessionId, 'Session destroyed')
  // Tear down the per-session MITM proxy (idempotent).
  if (session.streamingProxy) {
    void session.streamingProxy.stop().catch(() => { /* swallow */ })
    session.streamingProxy = undefined
  }
  sessions.delete(sessionId)
  clearInputThrottle(sessionId)
  clearSessionInputState(session)
  // Don't delete settingsDir — keep it so --continue can find conversation history.
  // The dir is in /tmp and gets cleaned on reboot anyway.
  broadcastSessionCount()
}

function cleanupSessionDir(dir: string): void {
  try {
    fs.rmSync(dir, { recursive: true, force: true })
  } catch (err) {
    warnLog('Failed to cleanup session dir', { dir, error: String(err) })
  }
}

function broadcastSessionCount(): void {
  eventBus.emit('terminal:sessions', { count: sessions.size })
}

// ---------------------------------------------------------------------------
// Elicitation handling (resolving pending elicitation from Electron)
// ---------------------------------------------------------------------------

/**
 * Resolve a pending elicitation for a session.
 * Called when Electron sends elicitation:response.
 */
export function handleElicitationResponse(
  sessionId: string,
  requestId: string,
  action: 'accept' | 'deny' | 'dismiss',
  content?: Record<string, unknown>
): void {
  const session = sessions.get(sessionId)
  if (!session) return

  const pending = session.pendingElicitations.get(requestId)
  if (!pending) {
    warnLog('Elicitation response for unknown requestId', { sessionId, requestId })
    return
  }

  session.pendingElicitations.delete(requestId)

  if (pending.deferred) {
    // HTTP already answered; inject the user's real reply as a normal follow-up
    // through the same serialized clear/paste/Enter path as every other submit.
    if (session.pty) {
      let text: string
      if (action === 'accept' && content) {
        const parts = Object.values(content).filter(v => typeof v === 'string' && v.trim().length > 0) as string[]
        text = parts.length > 0 ? parts.join(' ') : JSON.stringify(content)
      } else if (action === 'dismiss' || action === 'deny') {
        text = `[User dismissed the question]`
      } else {
        text = '(User accepted without details)'
      }
      // The PTY may have exited between the guard and here (session died while
      // the user was answering) — a write to a dead pty can throw synchronously.
      try {
        submitTextToSession(session, text)
      } catch (err) {
        warnLog('Elicitation PTY write failed (session likely exited)', {
          sessionId, requestId, error: err instanceof Error ? err.message : String(err),
        })
      }
    }
    eventBus.emit('elicitation:resolved', { sessionId, requestId, action })
    debugLog('Elicitation deferred→PTY delivery', { sessionId, requestId, action })
    return
  }

  pending.resolve({ action, content })
  eventBus.emit('elicitation:resolved', { sessionId, requestId, action })
  debugLog('Elicitation resolved', { sessionId, requestId, action })
}

// ---------------------------------------------------------------------------
// Agent tree management
// ---------------------------------------------------------------------------

/**
 * Register a child session under a parent (agent/teammate spawn).
 */
export function registerChildSession(parentSessionId: string, childSessionId: string): void {
  const parent = sessions.get(parentSessionId)
  if (parent) {
    parent.childSessions.push(childSessionId)
    eventBus.emit('session:tree-updated', { parentSessionId, childSessionId })
  }
}

/**
 * Delete session data (settingsDir) for a given conversationId.
 * Finds the settingsDir in SESSIONS_BASE that contains or maps to this conversation,
 * and removes it completely.
 */
export async function deleteSessionByConversationId(conversationId: string, user: string): Promise<void> {
  // 1. Check active sessions first — refuse to delete if PTY is running
  for (const session of sessions.values()) {
    if (session.cliConversationId === conversationId) {
      throw new Error('Cannot delete an active session — close it first')
    }
  }

  // 2. Scan SESSIONS_BASE for settingsDir containing this conversationId in its JSONL watcher config
  //    or find via findOrRecreateSettingsDir
  const normalizedBase = path.resolve(SESSIONS_BASE)
  if (!fs.existsSync(normalizedBase)) return

  for (const tokenDir of fs.readdirSync(normalizedBase, { withFileTypes: true })) {
    if (!tokenDir.isDirectory()) continue
    const tokenPath = path.join(normalizedBase, tokenDir.name)
    for (const sessionDir of fs.readdirSync(tokenPath, { withFileTypes: true })) {
      if (!sessionDir.isDirectory()) continue
      const candidate = path.join(tokenPath, sessionDir.name)
      // Check if this settingsDir's slug maps to a JSONL containing this conversationId
      const slug = path.resolve(candidate).replace(/[^a-zA-Z0-9]/g, '-')
      const projectsDir = path.join(os.homedir(), '.claude', 'projects')
      const jsonlPath = path.join(projectsDir, slug, `${conversationId}.jsonl`)
      if (fs.existsSync(jsonlPath)) {
        // Found it — delete settingsDir
        debugLog('Deleting session data', { conversationId, settingsDir: candidate, user })
        fs.rmSync(candidate, { recursive: true, force: true })
        // Also delete the JSONL file
        fs.rmSync(jsonlPath, { force: true })
        return
      }
    }
  }
  debugLog('Session data not found for deletion', { conversationId })
}

/**
 * Build the session tree for a token ID (for sidebar display).
 */
export function getSessionTree(tokenId: string): import('./types').TreeNode[] {
  const userSessions = [...sessions.values()].filter(s => s.tokenId === tokenId)
  const roots = userSessions.filter(s => !s.parentSessionId)

  function buildNode(session: PtySession): import('./types').TreeNode {
    const children = session.childSessions
      .map(id => sessions.get(id))
      .filter((s): s is PtySession => !!s)
      .map(buildNode)

    const folder = xbasename(session.cwd)
    return {
      id: session.id,
      type: session.isSubAgent ? 'agent' : 'session',
      label: session.isSubAgent ? `Agent (${folder})` : `#${session.inputCount} ${folder || 'session'}`,
      status: session.state === 'running' ? 'active' : session.state === 'exited' ? 'exited' : 'idle',
      cwd: session.cwd,
      children,
      parentId: session.parentSessionId || undefined,
      folderName: folder || undefined,
      sessionTitle: session.sessionTitle || undefined,
    }
  }

  return roots.map(buildNode)
}

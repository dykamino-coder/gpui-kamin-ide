// ============================================================================
// Session WebSocket — multiplexed PTY stream + MCP on /ws/session
// ============================================================================

import { WebSocketServer, WebSocket as WS } from 'ws'
import type { Server as HttpServer } from 'http'
import crypto from 'crypto'
import fs from 'fs'
import fsp from 'fs/promises'
import path from 'path'
import { fingerprintTranscript, canResume, isRecordBoundary, recordUuidMatches } from './jsonl-fingerprint'
import { SKIP_LINE_MARKER } from './jsonl-watcher'
import type { JsonlEntry } from '../../shared/jsonl-types'
import { registerWsRoute } from '../server/ws'
import { resolveToken } from '../auth/tokens'
import { debugLog, warnLog, errorLog, infoLog } from '../logging'
import type { ElectronToServerMsg, PtySession } from './types'
import {
  createSession,
  destroySession,
  detachSession,
  reattachSession,
  findSessionByConversation,
  followCompactLinks,
  writeInput,
  submitText,
  resizeTerminal,
  handleMcpResponse,
  handleMcpDenied,
  handleElicitationResponse,
  getSession,
  countUserSessions,
  restartWithEffort,
  restartWithModel,
  getSessionTree,
  deleteSessionByConversationId,
} from './session-manager'
import { eventBus } from '../events/bus'

/**
 * Map: sessionId → WS client.
 * Used by MCP HTTP handler to find the correct Electron client.
 */
/** Transcript download chunk size, in characters. Small enough that each frame
 *  serialises quickly and progress moves visibly; large enough that a 36MB
 *  transcript is ~36 frames rather than thousands. */
const DOWNLOAD_CHUNK_CHARS = 1_000_000

/** Yield to the event loop every N lines while parsing a whole transcript for a
 *  segment load, so the shared server stays responsive to other sessions. */
const SEGMENT_PARSE_YIELD_EVERY = 2_000

const sessionWsMap = new Map<string, WS>()

/**
 * Map: tokenId → Set of WS clients (for broadcasting tree updates).
 */
const tokenWsMap = new Map<string, Set<WS>>()
// In-flight session:resume по `${tokenId}:${conversationId}` — сериализация
// конкурентных резюмов одного диалога (см. комментарий в case 'session:resume').
const resumeLocks = new Map<string, Promise<void>>()
// Страховка от повисшего лока, если владелец упал вне внутреннего try.
const RESUME_LOCK_SAFETY_MS = 30_000

/**
 * Get the WS client for a session (used by mcp-http-handler).
 */
export function getSessionWs(sessionId: string): WS | undefined {
  return sessionWsMap.get(sessionId)
}

/**
 * Attach WebSocket server for Electron ↔ Server session communication.
 * Path: /ws/session
 */
export function attachSessionWebSocket(_server: HttpServer): void {
  // perMessageDeflate: the jsonl:entries / streaming:entry frames are
  // repetitive JSON (keys, uuids, base64 signatures) that deflates 5-10×.
  // The `ws` CLIENT already offers the extension by default; the server just
  // has to accept it (ws-server default is OFF). `threshold: 1024` skips the
  // tiny per-character session:output PTY frames (compressing those wastes
  // CPU); context takeover stays on (default) for best ratio on the big JSON
  // frames — memory is bounded by the number of live KaminIDE clients (one
  // /ws/session per user machine, not per session), which is small.
  const wss = new WebSocketServer({
    noServer: true,
    perMessageDeflate: { threshold: 1024 },
  })
  registerWsRoute('/ws/session', wss)

  wss.on('connection', (ws: WS) => {
    let authenticatedSessionId: string | null = null
    let authenticatedUser: string | null = null

    // Heartbeat: detect half-open sockets that the OS hasn't reported as
    // closed. Common when the client suspends (laptop lid close), drops a
    // VPN, or roams between WiFi networks — TCP RST never arrives, so the
    // server keeps thinking the WS is alive and never destroys the session.
    // We send a ping every 15s; if we don't get a pong by the next tick,
    // forcibly terminate the connection so 'close' fires and cleanup runs.
    let isAlive = true
    ws.on('pong', () => { isAlive = true })
    const heartbeat = setInterval(() => {
      if (!isAlive) {
        debugLog('Session WS heartbeat timeout — terminating', { sessionId: authenticatedSessionId })
        try { ws.terminate() } catch { /* ignore */ }
        return
      }
      isAlive = false
      try { ws.ping() } catch { /* socket already closing */ }
    }, 15_000)

    ws.on('message', async (raw: Buffer | string) => {
      let msg: ElectronToServerMsg
      try {
        msg = JSON.parse(typeof raw === 'string' ? raw : raw.toString())
      } catch {
        sendError(ws, 'Invalid JSON')
        return
      }

      switch (msg.type) {
        case 'session:create': {
          // Authenticate token
          const resolved = await resolveToken(msg.token)
          if (!resolved) {
            sendError(ws, 'Invalid token')
            ws.close(4001, 'Invalid token')
            return
          }

          // Clean up previous session on this WS (if any)
          if (authenticatedSessionId) {
            sessionWsMap.delete(authenticatedSessionId)
            destroySession(authenticatedSessionId)
            authenticatedSessionId = null
          }

          // Check max sessions for this user
          const maxSessions = 10 // TODO: get from token.max_sessions after DB migration
          const currentCount = countUserSessions(resolved.tokenId)
          if (currentCount >= maxSessions) {
            sendError(ws, `Max sessions reached (${maxSessions})`)
            ws.close(4002, 'Max sessions reached')
            return
          }

          try {
            const bearerHash = crypto.createHash('sha256').update(msg.token).digest('hex').slice(0, 16)
            const session = await createSession(
              ws,
              resolved.userName,
              resolved.tokenId,
              {
                cwd: msg.cwd,
                cols: msg.cols,
                rows: msg.rows,
                basePrompt: msg.basePrompt,
                transcriptMirrorDir: msg.transcriptMirrorDir,
                bearerHash,
                protocolVersion: msg.protocolVersion ?? 0,
              }
            )

            // Register external MCP tools BEFORE CLI initializes
            if (msg.externalTools && Array.isArray(msg.externalTools)) {
              session.registeredTools = msg.externalTools
              debugLog('External tools registered at session create', {
                sessionId: session.id,
                count: msg.externalTools.length,
                tools: msg.externalTools.map(t => t.name),
              })
            }

            authenticatedSessionId = session.id
            authenticatedUser = resolved.userName
            sessionWsMap.set(session.id, ws)

            // Track WS by tokenId for tree broadcasts
            if (!tokenWsMap.has(resolved.tokenId)) tokenWsMap.set(resolved.tokenId, new Set())
            tokenWsMap.get(resolved.tokenId)!.add(ws)

            ws.send(JSON.stringify({
              type: 'session:created',
              sessionId: session.id,
              effort: session.effort,
              model: session.model,
              settingsDir: session.settingsDir,
            }))

            // Send initial tree to this client
            broadcastTree(resolved.tokenId)

            debugLog('Session WS authenticated', {
              sessionId: session.id,
              userName: resolved.userName,
            })
          } catch (err) {
            const errMsg = err instanceof Error ? err.message : String(err)
            errorLog('Failed to create session', { error: errMsg })
            sendError(ws, `Failed to create session: ${errMsg}`)
          }
          break
        }

        case 'session:end': {
          // Explicit end from the client (Disconnect button / tab close).
          // Unlike a bare WS drop this destroys the session immediately —
          // no detach grace.
          if (!authenticatedSessionId) return
          debugLog('Session explicitly ended by client', { sessionId: authenticatedSessionId })
          sessionWsMap.delete(authenticatedSessionId)
          const endedSession = getSession(authenticatedSessionId)
          if (endedSession) {
            const clients = tokenWsMap.get(endedSession.tokenId)
            if (clients) {
              clients.delete(ws)
              if (clients.size === 0) tokenWsMap.delete(endedSession.tokenId)
            }
          }
          destroySession(authenticatedSessionId)
          authenticatedSessionId = null
          break
        }

        case 'session:input': {
          if (!authenticatedSessionId) {
            sendError(ws, 'Not authenticated — send session:create first')
            return
          }
          writeInput(authenticatedSessionId, msg.data)
          break
        }

        case 'session:interrupt': {
          // User pressed Stop. Ctrl+C alone only ends the CLI's NEXT turn — the
          // upstream SSE it already requested keeps flushing until the CLI drops
          // the socket, so the streamed message "arrives to completion" first.
          // Abort the in-flight upstream request directly so the stream stops NOW,
          // then SIGINT the CLI to end the turn.
          if (!authenticatedSessionId) return
          try { getSession(authenticatedSessionId)?.streamingProxy?.interrupt() } catch { /* no proxy this session */ }
          writeInput(authenticatedSessionId, '\x03')
          break
        }

        case 'session:submitText': {
          if (!authenticatedSessionId) {
            sendError(ws, 'Not authenticated — send session:create first')
            return
          }
          submitText(authenticatedSessionId, msg.data)
          break
        }

        case 'session:resize': {
          if (!authenticatedSessionId) return
          resizeTerminal(authenticatedSessionId, msg.cols, msg.rows)
          break
        }

        case 'mcp:response': {
          handleMcpResponse(msg.requestId, msg.result)
          break
        }

        case 'hook:response': {
          // Local hook executor in Electron returned its result.
          const { handleLocalHookResponse } = await import('../hooks/dispatcher')
          handleLocalHookResponse(msg.requestId, msg.result)
          break
        }

        case 'mcp:denied': {
          handleMcpDenied(msg.requestId, msg.reason)
          break
        }

        case 'elicitation:response': {
          if (!authenticatedSessionId) return
          handleElicitationResponse(
            authenticatedSessionId,
            msg.requestId,
            msg.action,
            msg.content
          )
          break
        }

        case 'session:resume': {
          // Resume a previous conversation
          const resolved = await resolveToken(msg.token)
          if (!resolved) {
            sendError(ws, 'Invalid token')
            ws.close(4001, 'Invalid token')
            return
          }

          // Clean up previous session on this WS (if any)
          if (authenticatedSessionId) {
            sessionWsMap.delete(authenticatedSessionId)
            destroySession(authenticatedSessionId)
            authenticatedSessionId = null
          }

          // Сериализация конкурентных resume одного диалога. Между
          // findSessionByConversation и регистрацией createSession есть
          // async-окно: два одновременных session:resume (реконнект ВСЕХ
          // вкладок перезапущенного приложения) оба не находили live-сессию
          // и спавнили ДВА CLI на один conversationId — два писателя одного
          // JSONL. Прод-транскрипт 968683a8: ~344 резюм-отпечатка, часть
          // ложится ВНУТРЬ чужих 5-секундных тул-лупов (интерливинг).
          // Второй resume ждёт первого, затем находит его сессию live-поиском
          // и реаттачится. Страховочный таймер снимает лок, если владелец
          // умер, не дойдя до release (исключение вне внутреннего try).
          // Канонизируем id ДО лока и live-поиска: вкладка с устаревшим id A
          // иначе не находила живую сессию, идущую под tip B той же цепочки,
          // и спавнила ВТОРОЙ CLI на ту же беседу (лок по сырому A тоже не
          // спасал) — двойной писатель, хвост в осиротевшем файле.
          if (msg.conversationId) {
            const canonical = followCompactLinks(msg.conversationId)
            if (canonical !== msg.conversationId) {
              infoLog('Resume: canonicalized to chain tip before lock', { requested: msg.conversationId, tip: canonical })
              msg.conversationId = canonical
            }
          }
          const resumeLockKey = msg.conversationId ? `${resolved.tokenId}:${msg.conversationId}` : null
          let releaseResumeLock: () => void = () => {}
          if (resumeLockKey) {
            while (resumeLocks.has(resumeLockKey)) {
              await resumeLocks.get(resumeLockKey)!.catch(() => { /* ждём, исход не важен */ })
            }
            let resolveLock!: () => void
            const lock = new Promise<void>((r) => { resolveLock = r })
            resumeLocks.set(resumeLockKey, lock)
            const safety = setTimeout(() => { releaseResumeLock() }, RESUME_LOCK_SAFETY_MS)
            releaseResumeLock = () => {
              clearTimeout(safety)
              if (resumeLocks.get(resumeLockKey) === lock) resumeLocks.delete(resumeLockKey)
              resolveLock()
            }
          }

          // Reattach path: the conversation's CLI is still alive (client WS
          // dropped and reconnected within the detach grace window, or the
          // same conversation is being opened from a fresh app instance).
          // Binding the new WS to the live session instead of killing +
          // respawning is what protects the JSONL from double-writer forks.
          const live = findSessionByConversation(resolved.tokenId, msg.conversationId)
          if (live) {
            const oldWs = sessionWsMap.get(live.id)
            if (oldWs && oldWs !== ws) {
              // Steal: a zombie WS is still bound (half-open socket) — close
              // it; its close handler is a no-op because the map no longer
              // points at it (stale-ws guard below).
              try { oldWs.close(4003, 'Session reattached from another client') } catch { /* ignore */ }
            }

            reattachSession(live, ws)
            authenticatedSessionId = live.id
            authenticatedUser = resolved.userName
            sessionWsMap.set(live.id, ws)
            if (!tokenWsMap.has(resolved.tokenId)) tokenWsMap.set(resolved.tokenId, new Set())
            tokenWsMap.get(resolved.tokenId)!.add(ws)

            // reattach does NOT recreate the session, so refresh the negotiated
            // stream protocol from THIS client (an old client reattaching a
            // session first created by a new one, or vice-versa, must get the
            // mode it actually supports).
            live.streamProtocol = msg.protocolVersion ?? 0

            if (msg.externalTools && Array.isArray(msg.externalTools)) {
              live.registeredTools = msg.externalTools
            }

            ws.send(JSON.stringify({
              type: 'session:created',
              sessionId: live.id,
              effort: live.effort,
              model: live.model,
              settingsDir: live.settingsDir,
              reattached: true,
            }))
            if (live.cliConversationId) {
              ws.send(JSON.stringify({
                type: 'session:conversation-id',
                sessionId: live.id,
                conversationId: live.cliConversationId,
              }))
            }
            // Re-emit the full transcript so the fresh client rebuilds its
            // chat state (same contract as a real resume).
            live.jsonlWatcher?.replayAll?.()

            broadcastTree(resolved.tokenId)
            debugLog('Session reattached via WS', {
              sessionId: live.id,
              conversationId: msg.conversationId,
            })
            releaseResumeLock()
            break
          }

          try {
            const bearerHash = crypto.createHash('sha256').update(msg.token).digest('hex').slice(0, 16)
            const session = await createSession(
              ws,
              resolved.userName,
              resolved.tokenId,
              {
                cwd: msg.cwd,
                cols: msg.cols,
                rows: msg.rows,
                resumeConversationId: msg.conversationId,
                // A resumed session respawns the CLI, so it must carry the
                // prompt too — otherwise the instructions only ever applied to
                // brand-new sessions.
                basePrompt: msg.basePrompt,
                transcriptMirrorDir: msg.transcriptMirrorDir,
                bearerHash,
                protocolVersion: msg.protocolVersion ?? 0,
              }
            )

            // Register external MCP tools BEFORE CLI initializes
            if (msg.externalTools && Array.isArray(msg.externalTools)) {
              session.registeredTools = msg.externalTools
              debugLog('External tools registered at session resume', {
                sessionId: session.id,
                count: msg.externalTools.length,
              })
            }

            authenticatedSessionId = session.id
            authenticatedUser = resolved.userName
            sessionWsMap.set(session.id, ws)

            // Track WS by tokenId for tree broadcasts
            if (!tokenWsMap.has(resolved.tokenId)) tokenWsMap.set(resolved.tokenId, new Set())
            tokenWsMap.get(resolved.tokenId)!.add(ws)

            ws.send(JSON.stringify({
              type: 'session:created',
              sessionId: session.id,
              effort: session.effort,
              model: session.model,
              settingsDir: session.settingsDir,
              resumeNotFound: session.resumeNotFound || undefined,
            }))

            // The requested conversation is not on this server — a fresh one
            // was started. Nothing hits the JSONL until the first message, so
            // without this the client sits in "Loading conversation…" until
            // its 20s watchdog. Declare the (empty) replay complete right away.
            if (session.resumeNotFound) {
              ws.send(JSON.stringify({
                type: 'jsonl:status',
                status: 'watching',
                replayComplete: true,
              }))
            }

            // Send initial tree to this client
            broadcastTree(resolved.tokenId)

            // Fire BridgeReconnect bridge-emit event — Electron client just
            // re-attached to (or first-attached to) this resumed session.
            try {
              const { emitBridgeEvent } = await import('../hooks/bridge-emitter')
              await emitBridgeEvent('BridgeReconnect', {
                sessionId: session.id,
                conversationId: msg.conversationId,
              })
            } catch { /* ignore */ }

            debugLog('Session resumed via WS', {
              sessionId: session.id,
              conversationId: msg.conversationId,
            })
          } catch (err) {
            const errMsg = err instanceof Error ? err.message : String(err)
            errorLog('Failed to resume session', { error: errMsg })
            sendError(ws, `Failed to resume session: ${errMsg}`)
          } finally {
            releaseResumeLock()
          }
          break
        }

        case 'session:change-effort': {
          if (!authenticatedSessionId) {
            sendError(ws, 'Not authenticated — send session:create first')
            return
          }

          const session = getSession(authenticatedSessionId)
          if (!session) {
            sendError(ws, 'Session not found')
            return
          }
          try {
            const oldSessionId = authenticatedSessionId
            // Preserve the negotiated stream protocol across the PTY restart —
            // the same client is still on this WS, it just won't re-handshake.
            const preservedProto = session.streamProtocol ?? 0

            // Remove old session from WS map
            sessionWsMap.delete(oldSessionId)

            let newSession: PtySession
            if (session.cliConversationId) {
              // Has conversation → restart with resume + new effort
              newSession = await restartWithEffort(
                oldSessionId,
                msg.effort,
                ws,
                authenticatedUser!,
                session.tokenId,
              )
            } else {
              // No conversation yet (empty chat) → recreate fresh with effort, preserve model
              session.isRestarting = true
              const preservedModel = session.model
              destroySession(oldSessionId)
              newSession = await createSession(
                ws,
                authenticatedUser!,
                session.tokenId,
                {
                  cwd: session.cwd,
                  cols: 120,
                  rows: 40,
                  effort: msg.effort,
                  model: preservedModel || undefined,
                  protocolVersion: preservedProto,
                }
              )
            }
            newSession.streamProtocol = preservedProto

            // Update tracked session
            authenticatedSessionId = newSession.id
            sessionWsMap.set(newSession.id, ws)

            // Notify client about the restart (include both effort + model for UI sync)
            ws.send(JSON.stringify({
              type: 'session:restarted',
              sessionId: newSession.id,
              effort: newSession.effort || msg.effort,
              model: newSession.model,
            }))

            debugLog('Session restarted with new effort', {
              oldSessionId,
              newSessionId: newSession.id,
              effort: msg.effort,
            })
          } catch (err) {
            const errMsg = err instanceof Error ? err.message : String(err)
            errorLog('Failed to change effort', { error: errMsg })
            sendError(ws, `Failed to change effort: ${errMsg}`)
          }
          break
        }

        case 'session:change-model': {
          if (!authenticatedSessionId) {
            sendError(ws, 'Not authenticated — send session:create first')
            return
          }

          const session = getSession(authenticatedSessionId)
          if (!session) {
            sendError(ws, 'Session not found')
            return
          }
          // Горячий путь (CLI умеет менять модель на лету slash-командой):
          // живой PTY с беседой — шлём `/model <id>` в stdin ВМЕСТО
          // killPtyAndWait+resume. Ctrl+U чистит строку ввода CLI (чат-ввод
          // идёт из вебвью целыми строками, так что строка пуста; черновик,
          // набранный прямо в панели Terminal, при этом потеряется —
          // осознанный компромисс). Если CLI сейчас стримит — команда встанет
          // в его очередь и применится после витка. Фоллбэк на рестарт — для
          // пустых сессий (нет беседы) и умершего PTY.
          if (session.cliConversationId && !session.isRestarting && msg.model) {
            writeInput(authenticatedSessionId, `\x15/model ${msg.model}\r`)
            session.model = msg.model
            ws.send(JSON.stringify({
              type: 'session:model-changed',
              sessionId: authenticatedSessionId,
              model: msg.model,
              effort: session.effort,
            }))
            return
          }
          try {
            const oldSessionId = authenticatedSessionId
            // Preserve the negotiated stream protocol across the PTY restart.
            const preservedProto = session.streamProtocol ?? 0

            // Remove old session from WS map
            sessionWsMap.delete(oldSessionId)

            let newSession: PtySession
            if (session.cliConversationId) {
              // Has conversation → restart with resume + new model
              newSession = await restartWithModel(
                oldSessionId,
                msg.model,
                ws,
                authenticatedUser!,
                session.tokenId,
              )
            } else {
              // No conversation yet (empty chat) → recreate fresh with model, preserve effort
              session.isRestarting = true
              const preservedEffort = session.effort
              destroySession(oldSessionId)
              newSession = await createSession(
                ws,
                authenticatedUser!,
                session.tokenId,
                {
                  cwd: session.cwd,
                  cols: 120,
                  rows: 40,
                  model: msg.model,
                  effort: preservedEffort || undefined,
                  protocolVersion: preservedProto,
                }
              )
            }
            newSession.streamProtocol = preservedProto

            // Update tracked session
            authenticatedSessionId = newSession.id
            sessionWsMap.set(newSession.id, ws)

            // Notify client about the restart (include both effort + model for UI sync)
            ws.send(JSON.stringify({
              type: 'session:restarted',
              sessionId: newSession.id,
              effort: newSession.effort,
              model: newSession.model || msg.model,
            }))

            debugLog('Session restarted with new model', {
              oldSessionId,
              newSessionId: newSession.id,
              model: msg.model,
            })
          } catch (err) {
            const errMsg = err instanceof Error ? err.message : String(err)
            errorLog('Failed to change model', { error: errMsg })
            sendError(ws, `Failed to change model: ${errMsg}`)
          }
          break
        }

        case 'session:delete-data': {
          // Delete session data (settingsDir) for a given conversationId
          const convId = msg.conversationId
          if (convId) {
            try {
              await deleteSessionByConversationId(convId, authenticatedUser || 'unknown')
              debugLog('Session data deleted', { conversationId: convId })
            } catch (err) {
              warnLog('Failed to delete session data', { conversationId: convId, error: String(err) })
            }
          }
          break
        }

        case 'mcp:register-external-tools': {
          // Electron sends external MCP tool schemas to merge into session's tool list
          if (!authenticatedSessionId) return
          const session = getSession(authenticatedSessionId)
          if (!session) return

          const externalTools = (msg as any).tools as Array<{ name: string; description: string; inputSchema: Record<string, unknown> }>
          if (!Array.isArray(externalTools)) return

          // Merge: keep existing registeredTools (built-in) and add/replace external ones
          // External tools are identified by not being in the built-in set
          const externalNames = new Set(externalTools.map(t => t.name))
          // Remove old external tools (anything that was previously registered and is now being refreshed)
          const keptTools = session.registeredTools.filter(t => !externalNames.has(t.name))
          session.registeredTools = [...keptTools, ...externalTools]

          debugLog('External MCP tools registered', {
            sessionId: authenticatedSessionId,
            externalCount: externalTools.length,
            totalCount: session.registeredTools.length,
            tools: externalTools.map(t => t.name),
          })
          break
        }

        // Incremental catch-up for a client that mirrors the transcript locally.
        // It says how much it already holds; we either top it up or tell it to
        // start over. Never silently: an ambiguous answer would let it splice new
        // records onto a stale history.
        case 'jsonl:sync-request': {
          if (!authenticatedSessionId) {
            sendError(ws, 'Not authenticated')
            return
          }
          const syncSession = getSession(authenticatedSessionId)
          const watcher = syncSession?.jsonlWatcher
          const syncPath = watcher?.getFilePath()
          // `streamFrom` is optional on the interface (older watcher shapes),
          // so a watcher without it can only serve a full replay.
          if (!watcher?.streamFrom || !syncPath) {
            ws.send(JSON.stringify({ type: 'jsonl:sync-reset', reason: 'no-transcript' }))
            return
          }
          const fp = await fingerprintTranscript(syncPath)
          const { sinceHead, sincePos, lastPos, lastUuid } =
            msg as unknown as { sinceHead?: string; sincePos?: number; lastPos?: number; lastUuid?: string }
          // Two independent gates. The fingerprint catches a rewritten file; the
          // boundary check catches a repaired one, where the head survives but
          // every offset after the edit has shifted (see isRecordBoundary).
          const atBoundary = sincePos === undefined ? false : await isRecordBoundary(syncPath, sincePos)
          const sameRecord = await recordUuidMatches(syncPath, lastPos, lastUuid)
          if (!canResume(fp, sinceHead, sincePos) || !atBoundary || !sameRecord) {
            // Rewritten (compaction), truncated, or the client has nothing —
            // it must drop its mirror and take a full replay.
            ws.send(JSON.stringify({ type: 'jsonl:sync-reset', head: fp?.head, size: fp?.size }))
            return
          }
          // Echo the offset we are resuming FROM: without it the client cannot
          // tell a topped-up stream from one that silently skipped a gap.
          ws.send(JSON.stringify({ type: 'jsonl:sync-begin', from: sincePos, head: fp?.head, size: fp?.size }))
          const endPos = await watcher.streamFrom(sincePos!, () => ws.readyState === ws.OPEN)
          ws.send(JSON.stringify({ type: 'jsonl:sync-end', head: fp?.head, size: endPos }))
          break
        }

        case 'jsonl:download-request': {
          if (!authenticatedSessionId) {
            sendError(ws, 'Not authenticated')
            return
          }
          const dlSession = getSession(authenticatedSessionId)
          if (!dlSession?.jsonlWatcher) {
            ws.send(JSON.stringify({ type: 'jsonl:download-response', content: null, fileName: null, error: 'No active JSONL watcher' }))
            return
          }
          const filePath = dlSession.jsonlWatcher.getFilePath()
          if (!filePath) {
            ws.send(JSON.stringify({ type: 'jsonl:download-response', content: null, fileName: null, error: 'JSONL file not yet discovered' }))
            return
          }
          try {
            // Async: a transcript is tens of MB and readFileSync blocked the
            // shared event loop for the whole read — stalling every OTHER
            // session on this server while one user clicked download.
            const content = await fsp.readFile(filePath, 'utf-8')
            const fileName = path.basename(filePath)
            // Chunked, not one 36MB frame. A single message meant one huge
            // JSON.stringify here and one huge parse on the client, with the
            // whole transcript materialised twice in each — and no way to show
            // progress, so the UI could only spin. Chunks keep each frame small
            // and let the client report real percentages.
            const total = content.length
            if (total <= DOWNLOAD_CHUNK_CHARS) {
              ws.send(JSON.stringify({ type: 'jsonl:download-response', content, fileName }))
            } else {
              ws.send(JSON.stringify({ type: 'jsonl:download-begin', fileName, total }))
              for (let sent = 0; sent < total; sent += DOWNLOAD_CHUNK_CHARS) {
                ws.send(JSON.stringify({
                  type: 'jsonl:download-chunk',
                  chunk: content.slice(sent, sent + DOWNLOAD_CHUNK_CHARS),
                  sent: Math.min(sent + DOWNLOAD_CHUNK_CHARS, total),
                  total,
                }))
              }
              ws.send(JSON.stringify({ type: 'jsonl:download-end', fileName, total }))
            }
          } catch (err) {
            ws.send(JSON.stringify({ type: 'jsonl:download-response', content: null, fileName: null, error: `Failed to read file: ${String(err)}` }))
          }
          break
        }

        // Load ONE archived compact segment whole, by timestamp range, from the
        // authoritative transcript (the client's mirror may not hold it). Mirrors
        // the retired mirror.readSegmentByTs: boundary dividers + ts-less rows
        // excluded; empty fromTs = start, empty toTs = to the newest.
        case 'jsonl:segment-request': {
          if (!authenticatedSessionId) {
            sendError(ws, 'Not authenticated')
            return
          }
          const { fromTs, toTs } = msg // WsMsgJsonlSegmentRequest — already narrowed
          const segPath = getSession(authenticatedSessionId)?.jsonlWatcher?.getFilePath()
          const records: JsonlEntry[] = []
          if (segPath) {
            try {
              const content = await fsp.readFile(segPath, 'utf-8')
              const lines = content.split('\n')
              // A full-transcript parse (tens of MB) must NOT run as one unbroken
              // sync loop: this server is shared, and a non-yielding 34k-line
              // JSON.parse burst stalls the event loop for EVERY session (the
              // same class the watcher's chunked reader avoids). Yield every
              // YIELD_EVERY lines so other sockets keep flowing. Rare path (a
              // click on an archived segment), so the extra ticks cost nothing.
              for (let i = 0; i < lines.length; i++) {
                if (i % SEGMENT_PARSE_YIELD_EVERY === 0) await new Promise<void>((r) => setImmediate(r))
                const line = lines[i]!
                if (!line.trim() || line.includes(SKIP_LINE_MARKER)) continue
                let r: JsonlEntry
                try { r = JSON.parse(line) as JsonlEntry } catch { continue }
                const ts = r.timestamp
                if (!ts) continue // ts-less rows can't be placed on the range axis
                if ((r as { subtype?: string }).subtype === 'compact_boundary') continue
                if (fromTs && ts < fromTs) continue
                if (toTs && ts >= toTs) continue
                records.push(r)
              }
            } catch { /* fall through — empty result tells the client to reset the view */ }
          }
          ws.send(JSON.stringify({ type: 'jsonl:segment-response', fromTs, toTs, records }))
          break
        }

        default: {
          warnLog('Unknown WS message type', { type: (msg as any).type })
        }
      }
    })

    // Shared teardown for close/error. A bare WS drop DETACHES the session
    // (grace window awaiting reattach) instead of killing the CLI — the old
    // instant destroySession here was the root cause of JSONL corruption
    // (mid-write kill + instant-reconnect double writer).
    const handleSocketGone = () => {
      clearInterval(heartbeat)
      if (!authenticatedSessionId) return

      // Stale-ws guard: if another WS already took over this session
      // (reattach steal), this socket's teardown must not touch it.
      if (sessionWsMap.get(authenticatedSessionId) !== ws) {
        authenticatedSessionId = null
        return
      }

      debugLog('Session WS disconnected — detaching', {
        sessionId: authenticatedSessionId,
        userName: authenticatedUser,
      })
      sessionWsMap.delete(authenticatedSessionId)

      // Clean up token WS map
      const session = getSession(authenticatedSessionId)
      if (session) {
        const clients = tokenWsMap.get(session.tokenId)
        if (clients) {
          clients.delete(ws)
          if (clients.size === 0) tokenWsMap.delete(session.tokenId)
        }
      }

      detachSession(authenticatedSessionId)
      authenticatedSessionId = null
    }

    ws.on('close', handleSocketGone)

    ws.on('error', (err) => {
      warnLog('Session WS error', { error: String(err) })
      handleSocketGone()
    })
  })
}

function sendError(ws: WS, error: string): void {
  if (ws.readyState === WS.OPEN) {
    ws.send(JSON.stringify({ type: 'session:error', error }))
  }
}

/**
 * Build and broadcast the session tree to all WS clients for a given token.
 */
function broadcastTree(tokenId: string): void {
  const clients = tokenWsMap.get(tokenId)
  if (!clients || clients.size === 0) return

  const tree = getSessionTree(tokenId)
  const msg = JSON.stringify({ type: 'session:tree-update', tree })

  for (const ws of clients) {
    if (ws.readyState === WS.OPEN) {
      ws.send(msg)
    }
  }
}

// Listen for tree changes and broadcast to affected Electron clients
eventBus.on('session:tree-updated', (data: any) => {
  // Find the tokenId from the parent session
  const parentSession = getSession(data?.parentSessionId)
  if (parentSession) {
    broadcastTree(parentSession.tokenId)
  }
})

// Also broadcast on session:created and session:destroyed (tree shape changes)
eventBus.on('session:created', (data: any) => {
  if (data?.tokenId) broadcastTree(data.tokenId)
})

eventBus.on('session:destroyed', (data: any) => {
  if (data?.tokenId) broadcastTree(data.tokenId)
})

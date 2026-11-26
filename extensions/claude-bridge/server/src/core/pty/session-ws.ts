// ============================================================================
// Session WebSocket — multiplexed PTY stream + MCP on /ws/session
// ============================================================================

import {
  flushedPristineTranscript,
  isClaudeMode,
  observedClaudeMode,
  permissionChangeBlocker,
  pristineClaudeChatId,
  publishClaudePermissions,
  retainClaudeMode,
} from './claude-permissions'
import {
  acceptCommandReport,
  runningShellCount,
  commandSnapshot,
  commandOutput,
  stopCommand,
  clearCompletedCommands,
  rememberCommandResult,
} from './commands'
import { cancelEndedSessionTasks, mcpTasks, settleMcpTask } from './mcp-tasks'
import { traceAnswered } from './mcp-trace'
import { lifecycleLog, type DestroyReason } from '../logging/lifecycle'
import { WebSocketServer, WebSocket as WS } from 'ws'
import type { Server as HttpServer } from 'http'
import crypto from 'crypto'
import fs from 'fs'
import fsp from 'fs/promises'
import path from 'path'
import { subagentTranscriptPath } from './subagent-transcript-path'
import { ResumeNotFoundError } from './session-resume-not-found'
import { SessionAdmissionError } from './session-admission'
import { armSwitchConfirm, effortCommand, isEffortLevel, isSafeModelId } from './live-switch'
import { interruptClaudeTurn, switchClaudeModelInPlace } from './claude-turn'
import { rememberEffortSelection } from './effort-selection'
import { noteEffortChanged } from './effort-note'
import type { SessionConfig } from '../types/pty'

/** A client's per-chat thinking display, validated: unknown values keep the default. */
function thinkingDisplayChoice(value: unknown): Pick<SessionConfig, 'thinkingDisplay'> {
  return value === 'summarized' || value === 'updates' ? { thinkingDisplay: value } : {}
}
import {
  codexBinding,
  codexEffort as codexReasoningEffort,
  effectiveEffort,
  codexModel as toCodexModel,
  saveCodexChoice,
} from './session-codex'
import { isAgentEngineId } from '../engine/engines'
import { broadcastAgentPreferences, handleAgentWsMessage, sendAgentPreferences } from './session-ws-agent'
import { chooseEngineForNewChat, mainAgentRefusal } from '../engine/engine-auth'
import { codexRuntime } from '../codex/codex-runtime'
import { streamFileChunks, waitForDrain, type DrainOutcome } from './download-backpressure'
import { fingerprintTranscript, canResume, isRecordBoundary, recordUuidMatches } from './jsonl-fingerprint'
import { SKIP_LINE_MARKER } from './jsonl-watcher'
import { readJsonlLines } from './jsonl-lines'
import { readHistoryPage } from './jsonl-page'
import type { JsonlEntry } from '../../shared/jsonl-types'
import { registerWsRoute } from '../server/ws'
import { resolveToken } from '../auth/tokens'
import { debugLog, warnLog, errorLog, infoLog } from '../logging'
import type { ClientToServerMsg, PtySession } from './types'
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
  handleElicitationResponse,
  getSession,
  restartWithEffort,
  restartWithModel,
  switchSessionEngine,
  getSessionTree,
  deleteSessionByConversationId,
  hibernateSession,
} from './session-manager'
import { acquireWakeSlot, promoteWake, registerHibernationHandle, type WakeTicket } from './session-hibernation'
import { hasInflightMcpCall, settleMcpResult } from './session-mcp-call'
import { sendToClient } from './session-io'
import { wsDeflateOptions } from './ws-compression'
import { isTerminalResponseInput } from './session-input-coordinator'
import { sessionCliControl } from './session-cli-control'

/** A queued message the leaving CLI had taken but not confirmed when the chat
 *  moved to the other engine: it is not sent on, the user decides. */
export const SWITCH_RETURNED_WARNING =
  'The chat moved to the other engine before this queued message was confirmed, so it was not sent on. ' +
  'Check the chat; your text is back in the input.'
import { eventBus } from '../events/bus'
import { sendSessionError as sendError } from './session-error'
import { notifyToolListChanged, replaceRegisteredTools } from './mcp-tool-notify'
import { normalizeHostDeclaration, sameHostDeclaration } from './host-declaration'
import { noteSegmentModel } from '../conversation/conversation'

/**
 * Map: sessionId → WS client.
 * Used by MCP HTTP handler to find the correct KaminIDE client.
 */
/** Transcript download chunk size, in bytes. Small enough that each frame
 *  serialises quickly and progress moves visibly; large enough that a 36MB
 *  transcript is ~36 frames rather than thousands. */
const DOWNLOAD_CHUNK_BYTES = 1024 * 1024
/** Порог обратного давления выгрузки — вчетверо ниже общего предела 16 МиБ:
 *  выгрузка не должна подводить соединение к границе, за которой обычные кадры
 *  сессии начнут БРОСАТЬСЯ. */
const DOWNLOAD_DRAIN = { limit: 4 * 1024 * 1024, pollMs: 25, timeoutMs: 60_000 }

/** Source bytes one segment response may carry. The reply is a single frame by
 *  protocol and must stay well under the 16 MiB per-connection send cap. */
const SEGMENT_RESPONSE_MAX_BYTES = 8 * 1024 * 1024

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

// Model selectors follow sign-in and sign-out of either engine live.
let agentAuthBroadcast: Promise<void> = Promise.resolve()
const pushAgentAuth = (): void => {
  agentAuthBroadcast = agentAuthBroadcast
    .then(() => broadcastAgentPreferences(tokenWsMap))
    .catch((error) => debugLog('agent auth broadcast failed', { error: (error as Error).message }))
}
eventBus.on('claude:account-updated', pushAgentAuth)
eventBus.on('codex:account-updated', pushAgentAuth)

/**
 * Get the WS client for a session (used by mcp-http-handler).
 */
/** What a message does to a chat whose CLI hibernates: `wake` respawns it
 *  first, `pass` needs no CLI, `ignore` has nothing to act on (its effect, if
 *  any, is kept for the wake), `release` drops the sleeping chat from this
 *  socket (a new chat, or an explicit end). */
/** What a sleeping chat keeps: enough to respawn it as it was. The stopped
 *  session object (output buffer, watchers, MCP log) is left to the GC. */
type SleepingChat = Pick<
  PtySession,
  | 'id'
  | 'tokenId'
  | 'cliConversationId'
  | 'settingsDir'
  | 'effort'
  | 'model'
  | 'streamProtocol'
  | 'host'
  | 'bearerHash'
  | 'conversation'
  | 'launchConfig'
  | 'registeredTools'
  | 'registeredResources'
  | 'registeredResourceTemplates'
  | 'registeredPrompts'
>

function sleepingChat(s: PtySession): SleepingChat {
  return {
    id: s.id,
    tokenId: s.tokenId,
    cliConversationId: s.cliConversationId,
    settingsDir: s.settingsDir,
    effort: s.effort,
    model: s.model,
    streamProtocol: s.streamProtocol,
    host: s.host,
    bearerHash: s.bearerHash,
    conversation: s.conversation,
    launchConfig: s.launchConfig,
    registeredTools: s.registeredTools,
    registeredResources: s.registeredResources,
    registeredResourceTemplates: s.registeredResourceTemplates,
    registeredPrompts: s.registeredPrompts,
  }
}
// Twenty CLIs booting at once on a small server can take well over ten seconds;
// waiting is better than falling back to terminal typing with a warning.
const WAKE_INPUT_READY_MS = 30_000

/** Resolves once the CLI accepts input (or after `limitMs`: delivery then
 *  takes the ordinary fallback path rather than waiting forever). */
async function untilInputReady(session: PtySession, limitMs: number): Promise<void> {
  const deadline = Date.now() + limitMs
  while (Date.now() < deadline && session.state === 'running') {
    const control = sessionCliControl(session)
    if (!control || control.inputReady()) return
    await new Promise((resolve) => setTimeout(resolve, 10))
  }
}

const IDLE_COMMANDS_REQUEST_MS = 10_000

/** Messages from the user that need the CLI: their wake goes to the front of
 *  the pool. Everything else that wakes a chat (a transcript request when a tab
 *  is shown, say) waits behind them like a warm-up. */
const URGENT_WAKE = new Set([
  'session:submitText',
  'session:input',
  'session:change-effort',
  'session:codex-collaboration',
  'session:change-model',
  'session:switch-engine',
])

export function sleepAction(type: string): 'wake' | 'warm' | 'pass' | 'ignore' | 'release' {
  switch (type) {
    case 'session:warm':
      return 'warm'
    case 'session:create':
    case 'session:resume':
    case 'session:end':
      return 'release'
    case 'session:resize':
    case 'session:interrupt':
    case 'session:submission-action':
    case 'mcp:response':
    case 'mcp:denied':
    case 'hook:response':
    case 'elicitation:response':
    case 'jsonl:subagent-select':
    case 'mcp:register-external-tools':
    case 'mcp:register-external-content':
      return 'ignore'
    case 'agent:set-default-engine':
    case 'codex:models':
    case 'session:delete-data':
      return 'pass'
    default:
      return type.startsWith('commands:') ? 'pass' : 'wake'
  }
}

export function getSessionWs(sessionId: string): WS | undefined {
  return sessionWsMap.get(sessionId)
}

/**
 * Attach WebSocket server for client host ↔ server session communication.
 * Path: /ws/session
 */
export function attachSessionWebSocket(_server: HttpServer): void {
  // perMessageDeflate: bulk frames (history pages, replays, top-ups, large
  // tool results) are repetitive JSON that deflates 5-10×, and the `ws` CLIENT
  // offers the extension by default; the server just has to accept it (ws-server
  // default is OFF). Small frames are sent as they are: deflating every live
  // record (~1-2 KiB) was the largest single cost on the main thread with 200
  // chats streaming, for a few hundred bytes saved each. Uncompressed and
  // compressed messages mix freely on one permessage-deflate connection (RFC
  // 7692 §6: RSV1 is set per message). The cut-off is `wsCompressMinBytes()`.
  // `ws` honours `threshold` only without server context takeover; with it
  // (the old setting) every frame was deflated whatever its size. Each bulk frame
  // is self-similar enough to keep its ratio on its own, and the connection no
  // longer carries a sliding window between messages.
  const wss = new WebSocketServer({
    noServer: true,
    perMessageDeflate: wsDeflateOptions(),
  })
  registerWsRoute('/ws/session', wss)

  wss.on('connection', (ws: WS) => {
    let authenticatedSessionId: string | null = null
    let authenticatedUser: string | null = null
    let authenticatedTokenId: string | null = null
    let socketGone = false
    // An engine switch replaces the chat's session; messages sent meanwhile
    // (a prompt typed right after the pick) wait for the new one instead of
    // failing with 'Not authenticated' and dropping the socket.
    let engineSwitch: Promise<unknown> | null = null
    type Startup = { reason?: 'disconnected' | 'ended' | 'replaced'; destroyReason?: DestroyReason }
    let startup: Startup | undefined
    const beginStartup = (reason: DestroyReason): Startup => {
      if (startup) {
        startup.reason ??= 'replaced'
        startup.destroyReason ??= reason
      }
      return (startup = {})
    }
    const ownsStartup = (operation: Startup) =>
      startup === operation && !operation.reason && !socketGone && ws.readyState === WS.OPEN
    const cancelStartup = (reason: Startup['reason']) => {
      if (startup) {
        startup.reason ??= reason
        if (reason === 'ended') startup.destroyReason ??= 'explicit_end'
      }
      startup = undefined
    }
    const dropTokenIndex = () => {
      if (!authenticatedTokenId) return
      const clients = tokenWsMap.get(authenticatedTokenId)
      clients?.delete(ws)
      if (clients?.size === 0) tokenWsMap.delete(authenticatedTokenId)
      authenticatedTokenId = null
    }
    const retireBinding = (reason: DestroyReason) => {
      dropTokenIndex()
      if (authenticatedSessionId && sessionWsMap.get(authenticatedSessionId) === ws) {
        sessionWsMap.delete(authenticatedSessionId)
        destroySession(authenticatedSessionId, reason)
      }
      authenticatedSessionId = null
      authenticatedUser = null
    }
    const releaseLateSession = (session: PtySession, operation: Startup) => {
      // A newer socket may already have stolen this registered session.
      if (session.ws !== ws || (sessionWsMap.has(session.id) && sessionWsMap.get(session.id) !== ws)) return
      if (operation.reason === 'ended' || operation.reason === 'replaced')
        destroySession(session.id, operation.destroyReason)
      else detachSession(session.id)
    }

    // Heartbeat: detect half-open sockets that the OS hasn't reported as
    // closed. Common when the client suspends (laptop lid close), drops a
    // VPN, or roams between WiFi networks — TCP RST never arrives, so the
    // server keeps thinking the WS is alive and never destroys the session.
    // We send a ping every 15s; if we don't get a pong by the next tick,
    // forcibly terminate the connection so 'close' fires and cleanup runs.
    let isAlive = true
    ws.on('pong', () => {
      isAlive = true
    })
    const heartbeat = setInterval(() => {
      if (!isAlive) {
        debugLog('Session WS heartbeat timeout — terminating', { sessionId: authenticatedSessionId })
        try {
          ws.terminate()
        } catch {
          /* ignore */
        }
        return
      }
      isAlive = false
      try {
        ws.ping()
      } catch {
        /* socket already closing */
      }
    }, 15_000)

    // The client's command report drives redelivery of lost calls and the
    // running-commands view. It is only time-critical while something runs:
    // a call in flight, an MCP task, or a background shell. An idle chat is
    // asked every 10 s instead of every second (200 chats × 2 frames/s was a
    // steady share of the main thread).
    let lastCommandsRequest = 0
    const commandsTimer = setInterval(() => {
      if (!authenticatedSessionId || sessionWsMap.get(authenticatedSessionId) !== ws) return
      const session = getSession(authenticatedSessionId)
      const now = Date.now()
      const live =
        !!session &&
        (hasInflightMcpCall(session.id) ||
          runningShellCount(session) > 0 ||
          (!!session.mcpEndpoint && mcpTasks.hasWorkingOwner(session.mcpEndpoint.id)))
      if (!live && now - lastCommandsRequest < IDLE_COMMANDS_REQUEST_MS) return
      lastCommandsRequest = now
      sendToClient(ws, { type: 'commands:request' })
    }, 1000)
    commandsTimer.unref?.()
    ws.once('close', () => clearInterval(commandsTimer))

    // ── Idle hibernation (session-hibernation.ts) ──────────────────────────
    // While asleep the chat has no CLI: `asleep` keeps the stopped session's
    // object (launch config, model, effort, registrations) and `done` settles
    // once its teardown finished. The next message that needs the CLI wakes it.
    let asleep: { session: SleepingChat; done: Promise<void>; cols: number; rows: number } | null = null
    const hibernate = () => {
      const id = authenticatedSessionId
      const session = id ? getSession(id) : undefined
      if (!id || !session || asleep || startup || engineSwitch || socketGone || sessionWsMap.get(id) !== ws)
        return false
      lifecycleLog('session_hibernate', {
        sessionId: id,
        idleMs: Date.now() - session.lastActivityAt.getTime(),
        conversationId: session.cliConversationId ?? undefined,
      })
      sessionWsMap.delete(id)
      const { cols, rows } = session.pty
      asleep = { session: sleepingChat(session), done: hibernateSession(id), cols, rows }
      sendHibernationState(id, 'asleep')
      return true
    }
    const unregisterHibernation = registerHibernationHandle({
      awakeSessionId: () =>
        !asleep &&
        !startup &&
        !engineSwitch &&
        authenticatedSessionId &&
        sessionWsMap.get(authenticatedSessionId) === ws
          ? authenticatedSessionId
          : null,
      hibernate,
    })

    // Hibernation as the client sees it, on the ordinary activity frame: an
    // asleep chat is idle at its prompt (typing into it sends `session:warm`),
    // a waking one shows a working status until its new CLI reports itself.
    const sendHibernationState = (sessionId: string, hibernation: 'asleep' | 'waking' | 'awake') =>
      sendToClient(ws, {
        type: 'session:activity',
        sessionId,
        hookDriven: true,
        rawTitle: hibernation === 'waking' ? 'Waking up the session…' : '',
        isWorking: hibernation === 'waking',
        waiting: false,
        ...(hibernation === 'asleep' ? { promptReady: true } : {}),
        hibernation,
      })
    ws.once('close', unregisterHibernation)

    // The wake in flight, if any: its pool ticket is promoted when a message
    // arrives for a chat that was only warming up.
    let waking: { ticket: WakeTicket; run: Promise<boolean> } | null = null
    const wake = (urgent: boolean): Promise<boolean> => {
      const sleeping = asleep!
      const old = sleeping.session
      const operation = beginStartup('replace_resume')
      const started = Date.now()
      const ticket: WakeTicket = { urgent }
      // The client shows the chat as waking until the new CLI reports itself.
      sendHibernationState(old.id, 'waking')
      const run = (async (): Promise<boolean> => {
        const release = await acquireWakeSlot(undefined, ticket)
        const slotAt = Date.now()
        let spawnedAt: number | undefined
        try {
          await sleeping.done
          if (!ownsStartup(operation)) return false
          // Another socket may have resumed this chat meanwhile (reconnect on
          // a new connection): never run two CLIs on one transcript.
          const launch = old.launchConfig ?? {}
          const identity = old.cliConversationId ?? launch.pristineClaudeId
          if (identity && findSessionByConversation(old.tokenId, identity))
            throw new Error('chat already running elsewhere')
          const resumeId =
            old.cliConversationId ??
            (launch.pristineClaudeId && flushedPristineTranscript(old.settingsDir, launch.pristineClaudeId)
              ? launch.pristineClaudeId
              : undefined)
          const fresh = await createSession(ws, authenticatedUser!, old.tokenId, {
            ...launch,
            cols: sleeping.cols,
            rows: sleeping.rows,
            effort: old.effort || undefined,
            model: old.model || undefined,
            protocolVersion: old.streamProtocol ?? 0,
            host: old.host,
            bearerHash: old.bearerHash,
            carryRegistrations: old,
            history: undefined,
            noSpawnOnNotFound: launch.claudePermissionMode && resumeId ? true : undefined,
            segmentStart: undefined,
            deferTranscript: undefined,
            // An empty chat had nothing to resume: it starts fresh, same cwd/model.
            resumeConversationId: resumeId,
            reuseSettingsDir: old.cliConversationId || launch.pristineClaudeId ? old.settingsDir : undefined,
            conversation: old.conversation,
          })
          if (!ownsStartup(operation)) {
            releaseLateSession(fresh, operation)
            return false
          }
          fresh.streamProtocol = old.streamProtocol ?? 0
          spawnedAt = Date.now()
          // The slot is held until the CLI takes input: a CLI boot is CPU-bound
          // well past the spawn, and twenty at once would all finish late.
          // Messages that woke the chat wait until the CLI takes input the normal
          // way; delivered earlier they would fall back to terminal typing.
          await untilInputReady(fresh, WAKE_INPUT_READY_MS)
          if (!ownsStartup(operation)) {
            releaseLateSession(fresh, operation)
            return false
          }
          asleep = null
          authenticatedSessionId = fresh.id
          sessionWsMap.set(fresh.id, ws)
          authenticatedTokenId = old.tokenId
          if (!tokenWsMap.has(old.tokenId)) tokenWsMap.set(old.tokenId, new Set())
          tokenWsMap.get(old.tokenId)!.add(ws)
          ws.send(
            JSON.stringify({
              type: 'session:restarted',
              sessionId: fresh.id,
              mcpTaskOwner: fresh.mcpEndpoint?.id,
              codexCollaboration: codexBinding(fresh)?.agent.collaboration,
              effort: fresh.effort,
              model: fresh.model,
            }),
          )
          publishClaudePermissions(fresh)
          codexBinding(fresh)?.pushPlanLimits()
          sendHibernationState(fresh.id, 'awake')
          const now = Date.now()
          // Phases: pool wait, server-side start (teardown wait, settings,
          // MCP endpoint, spawn), CLI boot until its input channel is up.
          lifecycleLog('session_wake', {
            sessionId: fresh.id,
            waitedMs: now - started,
            urgent: ticket.urgent,
            slotMs: slotAt - started,
            startMs: spawnedAt! - slotAt,
            bootMs: now - spawnedAt!,
          })
          return true
        } catch (err) {
          if (!ownsStartup(operation)) return false
          const error = err instanceof Error ? err.message : String(err)
          errorLog('Session wake failed', { sessionId: old.id, error })
          asleep = null
          authenticatedSessionId = null
          // The client's ordinary path for a dead CLI: it resumes the chat.
          sendToClient(ws, { type: 'session:exit', code: -1, sessionId: old.id })
          return false
        } finally {
          release()
          if (startup === operation) startup = undefined
        }
      })()
      engineSwitch = run
      waking = { ticket, run }
      void run.finally(() => {
        if (engineSwitch === run) engineSwitch = null
        if (waking?.run === run) waking = null
      })
      return run
    }

    ws.on('message', async (raw: Buffer | string) => {
      if (socketGone || ws.readyState !== WS.OPEN) return
      let msg: ClientToServerMsg
      try {
        msg = JSON.parse(typeof raw === 'string' ? raw : raw.toString())
      } catch {
        sendError(ws, 'Invalid JSON')
        return
      }
      if (msg.type === 'session:change-claude-permission-mode') {
        const id = authenticatedSessionId
        if (!id || msg.sessionId !== id) return
        const session = getSession(id)
        if (
          !session ||
          session.engine === 'codex' ||
          session.ws !== ws ||
          sessionWsMap.get(id) !== ws ||
          asleep ||
          startup ||
          engineSwitch
        ) {
          sendToClient(ws, {
            type: 'session:claude-permissions',
            sessionId: id,
            available: [],
            reason: 'The selected chat is not available for a mode change.',
          })
          return
        }
        if (!isClaudeMode(msg.mode)) {
          publishClaudePermissions(session, 'Select the requested mode in the native Console.')
          return
        }
        const pristineId = pristineClaudeChatId(session)
        const reason = permissionChangeBlocker(session, pristineId)
        if (reason) {
          publishClaudePermissions(session, reason)
          return
        }
        try {
          retainClaudeMode(session, msg.mode)
          session.launchConfig = {
            ...session.launchConfig,
            claudePermissionMode: msg.mode,
            pristineClaudeId: pristineId,
          }
          publishClaudePermissions(session)
          if (observedClaudeMode(session) === msg.mode) return
          // No await between admission and stopping the idle terminal. The normal
          // wake path owns the old process, socket, transcript and queued input.
          if (!hibernate()) {
            publishClaudePermissions(session, 'The chat changed before it could restart.')
            return
          }
          await wake(true)
        } catch {
          publishClaudePermissions(session, 'The mode could not be saved. Reconnect using the native Console.')
        }
        return
      }
      if (asleep) {
        // Before waiting on a wake: a message is confirmed at once (it waits in
        // order for the new CLI), and a warm-up it lands on goes to the front.
        // Focus and cursor reports from the client's terminal answer the stopped
        // CLI; they are not input and must not wake the chat.
        if (msg.type === 'session:input' && isTerminalResponseInput(msg.data)) return
        const action = sleepAction(msg.type)
        if (action === 'wake' && msg.type === 'session:submitText' && msg.submissionId)
          sendToClient(ws, {
            type: 'session:submission',
            sessionId: asleep.session.id,
            id: msg.submissionId,
            text: msg.data,
            state: 'queued',
            cancelable: false,
            createdAt: Date.now(),
          })
        if (action === 'wake' && waking && URGENT_WAKE.has(msg.type)) promoteWake(waking.ticket)
        if (action === 'warm') {
          if (!engineSwitch) void wake(false)
          return
        }
      }
      if (engineSwitch && msg.type !== 'session:end') {
        await engineSwitch.catch(() => {})
        if (socketGone || ws.readyState !== WS.OPEN) return
      }
      if (asleep) {
        const sleeping = asleep
        switch (sleepAction(msg.type)) {
          case 'warm':
            return
          case 'ignore':
            if (msg.type === 'session:resize' && msg.cols > 0 && msg.rows > 0) {
              sleeping.cols = msg.cols
              sleeping.rows = msg.rows
            } else if (msg.type === 'mcp:register-external-tools' && Array.isArray((msg as any).tools)) {
              sleeping.session.registeredTools = (msg as any).tools
            } else if (msg.type === 'mcp:register-external-content') {
              sleeping.session.registeredResources = Array.isArray(msg.resources) ? msg.resources : []
              sleeping.session.registeredResourceTemplates = Array.isArray(msg.resourceTemplates)
                ? msg.resourceTemplates
                : []
              sleeping.session.registeredPrompts = Array.isArray(msg.prompts) ? msg.prompts : []
            }
            return
          case 'pass':
            break
          case 'release':
            // A new chat or an explicit end on this socket: the sleeping one is
            // simply left stopped (its transcript stays resumable).
            await sleeping.done
            if (asleep === sleeping) {
              asleep = null
              authenticatedSessionId = null
            }
            if (socketGone || ws.readyState !== WS.OPEN) return
            break
          case 'wake':
            if (!(await (engineSwitch ?? wake(URGENT_WAKE.has(msg.type))))) return
            if (socketGone || ws.readyState !== WS.OPEN) return
            break
        }
      }
      switch (msg.type) {
        case 'session:create': {
          // INC-2026-0070: a create carries no conversation id by definition —
          // the journal shows whether a tab asked for a fresh session.
          lifecycleLog('session_request', { requestKind: 'create', resumeRequested: false })
          const operation = beginStartup('replace_start')
          try {
            // Authenticate token
            const resolved = await resolveToken(msg.token)
            if (!ownsStartup(operation)) return
            if (!resolved) {
              sendError(ws, 'Invalid token', 4001, 'Invalid token')
              return
            }

            retireBinding('replace_start')

            // Admission reservations in createSession cover pending startups too.
            try {
              // A new chat on a signed-out engine opens on the signed-in one,
              // with a notice. Older clients send no engine and keep Claude.
              const engineChoice = isAgentEngineId(msg.engine)
                ? await chooseEngineForNewChat(msg.engine, { runtime: codexRuntime() })
                : null
              if (!ownsStartup(operation)) return
              const bearerHash = crypto.createHash('sha256').update(msg.token).digest('hex').slice(0, 16)
              const session = await createSession(ws, resolved.userName, resolved.tokenId, {
                // Explicit per chat: clients send the user's main agent (agent:preferences).
                ...(engineChoice ? { engine: engineChoice.engine } : {}),
                cwd: msg.cwd,
                cols: msg.cols,
                rows: msg.rows,
                basePrompt: msg.basePrompt,
                ...thinkingDisplayChoice(msg.thinkingDisplay),
                transcriptMirrorDir: msg.transcriptMirrorDir,
                host: normalizeHostDeclaration(msg.host),
                bearerHash,
                protocolVersion: msg.protocolVersion ?? 0,
                history: msg.history,
                carryRegistrations: Array.isArray(msg.externalTools)
                  ? { registeredTools: msg.externalTools }
                  : undefined,
              })

              if (!ownsStartup(operation)) {
                releaseLateSession(session, operation)
                return
              }

              authenticatedSessionId = session.id
              authenticatedUser = resolved.userName
              authenticatedTokenId = resolved.tokenId
              sessionWsMap.set(session.id, ws)

              // Track WS by tokenId for tree broadcasts
              if (!tokenWsMap.has(resolved.tokenId)) tokenWsMap.set(resolved.tokenId, new Set())
              tokenWsMap.get(resolved.tokenId)!.add(ws)

              ws.send(
                JSON.stringify({
                  type: 'session:created',
                  sessionId: session.id,
                  mcpTaskOwner: session.mcpEndpoint?.id,
                  codexCollaboration: codexBinding(session)?.agent.collaboration,
                  effort: session.effort,
                  model: session.model,
                  settingsDir: session.settingsDir,
                  engine: session.engine ?? 'claude',
                  ...(engineChoice?.notice ? { engineNotice: engineChoice.notice } : {}),
                }),
              )
              publishClaudePermissions(session)
              codexBinding(session)?.pushPlanLimits()
              void sendAgentPreferences(ws, resolved.tokenId)

              // Send initial tree to this client
              broadcastTree(resolved.tokenId)

              debugLog('Session WS authenticated', {
                sessionId: session.id,
                userName: resolved.userName,
              })
            } catch (err) {
              if (!ownsStartup(operation)) return
              const errMsg = err instanceof Error ? err.message : String(err)
              errorLog('Failed to create session', { error: errMsg })
              sendError(
                ws,
                `Failed to create session: ${errMsg}`,
                err instanceof SessionAdmissionError && err.kind === 'token' ? 4002 : undefined,
                'Max sessions reached',
              )
            }
            break
          } catch (error) {
            if (ownsStartup(operation)) sendError(ws, 'Session authentication failed')
          } finally {
            if (startup === operation) startup = undefined
          }
          break
        }

        case 'agent:set-default-engine':
        case 'codex:models': {
          await handleAgentWsMessage(ws, msg, authenticatedTokenId)
          break
        }

        case 'session:end': {
          if (msg.handoff && authenticatedSessionId) {
            const session = getSession(authenticatedSessionId)
            sendToClient(ws, {
              type: 'session:handoff',
              sessionId: authenticatedSessionId,
              transferable: session ? (sessionCliControl(session)?.handoff() ?? []) : [],
            })
          }
          cancelStartup('ended')
          const ending =
            authenticatedSessionId && sessionWsMap.get(authenticatedSessionId) === ws
              ? getSession(authenticatedSessionId)
              : undefined
          retireBinding('explicit_end')
          if (ending)
            void cancelEndedSessionTasks(ending).catch((error) =>
              debugLog('ended session task cancel failed', { error: (error as Error).message }),
            )
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
          interruptClaudeTurn(authenticatedSessionId, getSession, writeInput)
          break
        }

        case 'session:submitText': {
          if (!authenticatedSessionId) {
            sendError(ws, 'Not authenticated — send session:create first')
            return
          }
          submitText(authenticatedSessionId, msg.data, msg.submissionId)
          break
        }

        case 'session:warm':
          // The chat is awake already: nothing to warm.
          break

        case 'session:submission-action': {
          if (!authenticatedSessionId) return
          const session = getSession(authenticatedSessionId)
          if (session && typeof msg.id === 'string') sessionCliControl(session)?.action(msg.id, msg.action)
          break
        }

        case 'session:resize': {
          if (!authenticatedSessionId) return
          resizeTerminal(authenticatedSessionId, msg.cols, msg.rows)
          break
        }

        case 'commands:clear':
        case 'commands:list':
        case 'commands:report':
        case 'commands:stop':
        case 'commands:output': {
          if (!authenticatedSessionId || sessionWsMap.get(authenticatedSessionId) !== ws) return
          const session = getSession(authenticatedSessionId)
          if (!session) return
          if (msg.type === 'commands:clear') await clearCompletedCommands(session)
          if (msg.type === 'commands:report') await acceptCommandReport(session, msg.report)
          if (msg.type === 'commands:stop' && typeof msg.id === 'string') await stopCommand(session, msg.id)
          if (msg.type === 'commands:output') {
            if (typeof msg.id === 'string')
              sendToClient(ws, { type: 'commands:output', id: msg.id, output: await commandOutput(session, msg.id) })
            break
          }
          sendToClient(ws, { type: 'commands:snapshot', snapshot: await commandSnapshot(session) })
          break
        }

        case 'mcp:response':
        case 'mcp:denied': {
          if (!authenticatedSessionId || sessionWsMap.get(authenticatedSessionId) !== ws) return
          if (msg.type === 'mcp:response') traceAnswered(msg.requestId, msg.timing)
          let accepted: boolean
          try {
            const session = getSession(authenticatedSessionId)
            accepted = msg.taskOwner
              ? msg.sessionId === authenticatedSessionId && !!session && (await settleMcpTask(session, msg))
              : settleMcpResult(authenticatedSessionId, msg)
            if (accepted && session)
              rememberCommandResult(session, msg.requestId, msg.type === 'mcp:response' ? msg.result : msg.reason)
          } catch {
            break
          } // no ack until durable settlement; outbox retries
          sendToClient(ws, {
            type: 'mcp:result-ack',
            sessionId: authenticatedSessionId,
            requestId: msg.requestId,
            accepted,
          })
          break
        }

        case 'hook:response': {
          // Local hook executor in the client host returned its result.
          const { handleLocalHookResponse } = await import('../hooks/dispatcher')
          handleLocalHookResponse(msg.requestId, msg.result)
          break
        }

        case 'elicitation:response': {
          if (!authenticatedSessionId) return
          handleElicitationResponse(authenticatedSessionId, msg.requestId, msg.action, msg.content)
          break
        }

        case 'session:resume': {
          lifecycleLog('session_request', {
            requestKind: 'resume',
            resumeRequested: Boolean(msg.conversationId),
            resumeConversationId: msg.conversationId,
          })
          const operation = beginStartup('replace_resume')
          let releaseResumeLock: () => void = () => {}
          try {
            const resolved = await resolveToken(msg.token)
            if (!ownsStartup(operation)) return
            if (!resolved) {
              sendError(ws, 'Invalid token', 4001, 'Invalid token')
              return
            }

            retireBinding('replace_resume')
            // Canonicalize before the shared lock: compact-chain aliases must
            // serialize on the same live conversation, with ownership checked
            // after each await before a waiter can create or reattach a PTY.
            if (msg.conversationId) {
              const canonical = followCompactLinks(msg.conversationId)
              if (canonical !== msg.conversationId) {
                infoLog('Resume: canonicalized to chain tip before lock', {
                  requested: msg.conversationId,
                  tip: canonical,
                })
                msg.conversationId = canonical
              }
            }
            const resumeLockKey = msg.conversationId ? `${resolved.tokenId}:${msg.conversationId}` : null
            if (resumeLockKey) {
              if (resumeLocks.has(resumeLockKey)) lifecycleLog('resume_wait')
              while (resumeLocks.has(resumeLockKey)) {
                await resumeLocks.get(resumeLockKey)!.catch(() => {
                  /* wait for either outcome */
                })
              }
              if (!ownsStartup(operation)) return
              let resolveLock!: () => void
              const lock = new Promise<void>((resolve) => {
                resolveLock = resolve
              })
              resumeLocks.set(resumeLockKey, lock)
              const safety = setTimeout(() => {
                releaseResumeLock()
              }, RESUME_LOCK_SAFETY_MS)
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
            if (!ownsStartup(operation)) return
            const live = findSessionByConversation(resolved.tokenId, msg.conversationId)
            if (live) {
              lifecycleLog('resume_reuse', { sessionId: live.id, resumeConversationId: msg.conversationId })
              const oldWs = sessionWsMap.get(live.id)
              sessionWsMap.set(live.id, ws)
              if (oldWs && oldWs !== ws) {
                // Steal: a zombie WS is still bound (half-open socket) — close
                // it; its close handler is a no-op because the map no longer
                // points at it (stale-ws guard below).
                try {
                  oldWs.close(4003, 'Session reattached from another client')
                } catch {
                  /* ignore */
                }
              }

              authenticatedSessionId = live.id
              authenticatedUser = resolved.userName
              authenticatedTokenId = resolved.tokenId
              sessionWsMap.set(live.id, ws)
              if (!tokenWsMap.has(resolved.tokenId)) tokenWsMap.set(resolved.tokenId, new Set())
              tokenWsMap.get(resolved.tokenId)!.add(ws)

              // reattach does NOT recreate the session, so refresh the negotiated
              // stream protocol from THIS client (an old client reattaching a
              // session first created by a new one, or vice-versa, must get the
              // mode it actually supports).
              live.streamProtocol = msg.protocolVersion ?? 0

              // The declaration belongs to the attached client as well: a different
              // host kind changes which built-in tools the reused CLI may see.
              const declared = normalizeHostDeclaration(msg.host)
              const hostChanged = !sameHostDeclaration(live.host, declared)
              live.host = declared

              ws.send(
                JSON.stringify({
                  type: 'session:created',
                  sessionId: live.id,
                  mcpTaskOwner: live.mcpEndpoint?.id,
                  codexCollaboration: codexBinding(live)?.agent.collaboration,
                  effort: live.effort,
                  model: live.model,
                  settingsDir: live.settingsDir,
                  reattached: true,
                  engine: live.engine ?? 'claude',
                }),
              )
              // The client accepts terminal bytes only after session:created.
              // Reattach synchronously replays the live PTY buffer to this socket.
              reattachSession(live, ws)
              publishClaudePermissions(live)
              if (msg.externalTools && Array.isArray(msg.externalTools)) {
                // The reused CLI keeps its MCP connection: announce a changed
                // catalog instead of silently swapping the snapshot.
                replaceRegisteredTools(live, msg.externalTools, 'reattach')
              }
              if (hostChanged && live.mcpInitialized) notifyToolListChanged(live.id)

              if (live.cliConversationId) {
                lifecycleLog('conversation_id', {
                  sessionId: live.id,
                  conversationId: live.cliConversationId,
                  previousConversationId: msg.conversationId,
                  idSource: 'reattach',
                })
                ws.send(
                  JSON.stringify({
                    type: 'session:conversation-id',
                    sessionId: live.id,
                    conversationId: live.cliConversationId,
                  }),
                )
              }
              // Re-emit the full transcript so the fresh client rebuilds its
              // chat state (same contract as a real resume).
              live.jsonlWatcher?.replayAll?.(msg.history)

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
              const session = await createSession(ws, resolved.userName, resolved.tokenId, {
                cwd: msg.cwd,
                cols: msg.cols,
                rows: msg.rows,
                resumeConversationId: msg.conversationId,
                noSpawnOnNotFound: msg.noSpawnOnNotFound === true,
                ...(isAgentEngineId(msg.engine) ? { engine: msg.engine } : {}),
                // A resumed session respawns the CLI, so it must carry the
                // prompt too — otherwise the instructions only ever applied to
                // brand-new sessions.
                basePrompt: msg.basePrompt,
                ...thinkingDisplayChoice(msg.thinkingDisplay),
                transcriptMirrorDir: msg.transcriptMirrorDir,
                host: normalizeHostDeclaration(msg.host),
                bearerHash,
                protocolVersion: msg.protocolVersion ?? 0,
                history: msg.history,
                carryRegistrations: Array.isArray(msg.externalTools)
                  ? { registeredTools: msg.externalTools }
                  : undefined,
              })

              if (!ownsStartup(operation)) {
                releaseLateSession(session, operation)
                return
              }

              authenticatedSessionId = session.id
              authenticatedUser = resolved.userName
              authenticatedTokenId = resolved.tokenId
              sessionWsMap.set(session.id, ws)

              // Track WS by tokenId for tree broadcasts
              if (!tokenWsMap.has(resolved.tokenId)) tokenWsMap.set(resolved.tokenId, new Set())
              tokenWsMap.get(resolved.tokenId)!.add(ws)

              ws.send(
                JSON.stringify({
                  type: 'session:created',
                  sessionId: session.id,
                  mcpTaskOwner: session.mcpEndpoint?.id,
                  codexCollaboration: codexBinding(session)?.agent.collaboration,
                  effort: session.effort,
                  model: session.model,
                  settingsDir: session.settingsDir,
                  resumeNotFound: session.resumeNotFound || undefined,
                  engine: session.engine ?? 'claude',
                }),
              )

              publishClaudePermissions(session)
              codexBinding(session)?.pushPlanLimits()
              // The requested conversation is not on this server — a fresh one
              // was started. Nothing hits the JSONL until the first message, so
              // without this the client sits in "Loading conversation…" until
              // its 20s watchdog. Declare the (empty) replay complete right away.
              if (session.resumeNotFound || session.resumedOnFreshThread) {
                ws.send(
                  JSON.stringify({
                    type: 'jsonl:status',
                    status: 'watching',
                    replayComplete: true,
                  }),
                )
              }

              // Send initial tree to this client
              broadcastTree(resolved.tokenId)

              // Fire BridgeReconnect bridge-emit event — the client just
              // re-attached to (or first-attached to) this resumed session.
              try {
                const { emitBridgeEvent } = await import('../hooks/bridge-emitter')
                if (!ownsStartup(operation)) return
                await emitBridgeEvent('BridgeReconnect', {
                  sessionId: session.id,
                  conversationId: msg.conversationId,
                })
              } catch {
                /* ignore */
              }

              debugLog('Session resumed via WS', {
                sessionId: session.id,
                conversationId: msg.conversationId,
              })
            } catch (err) {
              if (!ownsStartup(operation)) return
              const errMsg = err instanceof Error ? err.message : String(err)
              if (err instanceof ResumeNotFoundError) {
                ws.send(JSON.stringify({ type: 'session:resume-not-found' }))
                return
              }
              errorLog('Failed to resume session', { error: errMsg })
              sendError(
                ws,
                `Failed to resume session: ${errMsg}`,
                err instanceof SessionAdmissionError && err.kind === 'token' ? 4002 : undefined,
                'Max sessions reached',
              )
            } finally {
              releaseResumeLock()
            }
            break
          } catch (error) {
            if (ownsStartup(operation)) sendError(ws, 'Session authentication failed')
          } finally {
            releaseResumeLock()
            if (startup === operation) startup = undefined
          }
          break
        }

        case 'session:codex-collaboration': {
          const session = authenticatedSessionId && getSession(authenticatedSessionId)
          if (!session || session.id !== msg.sessionId) return
          const binding = codexBinding(session)
          if (!binding) return
          if ((msg.mode === 'default' || msg.mode === 'plan') && binding.agent.chooseCollaboration(msg.mode)) {
            saveCodexChoice(session, { collaborationMode: msg.mode })
          } else {
            binding.appendSystem('The selected Codex collaboration mode is unavailable.', 'warning')
          }
          sendToClient(ws, {
            type: 'session:codex-collaboration',
            sessionId: session.id,
            codexCollaboration: binding.agent.collaboration,
          })
          return
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
          const codexEffort = codexBinding(session)
          if (codexEffort) {
            // codex takes the effort per turn: it applies from the next turn, no restart.
            const supported = codexRuntime().knownModel(session.model)?.reasoningEfforts
            const level = codexReasoningEffort(msg.effort, supported)
            if (level) {
              codexEffort.agent.setEffort(level)
              session.effort = level
              saveCodexChoice(session, { effort: level })
            } else {
              codexEffort.appendSystem('The selected Codex model does not support that reasoning effort.', 'warning')
            }
            ws.send(
              JSON.stringify({
                type: 'session:model-changed',
                sessionId: authenticatedSessionId,
                model: session.model,
                effort: session.effort,
                inPlace: true,
              }),
            )
            return
          }
          if (!isEffortLevel(msg.effort)) {
            sendError(ws, `Unsupported effort level: ${String(msg.effort).slice(0, 32)}`)
            return
          }
          // Live path: `/effort <level>` in the running CLI keeps the PTY, the
          // console and the warm transcript. The CLI's cache-warning dialog for
          // this level is confirmed by the responder (live-switch.ts) only if
          // it actually appears. The restart below stays as the fallback for
          // empty chats (CLI may still be starting) and a dead PTY.
          if (session.cliConversationId && !session.isRestarting) {
            if (writeInput(authenticatedSessionId, effortCommand(msg.effort))) {
              // Armed right after the write: PTY output arrives on a later
              // tick, so the dialog cannot slip past the responder.
              armSwitchConfirm(session, 'effort', msg.effort)
              session.effort = msg.effort
              noteEffortChanged(session)
              // The live `/effort` reaches only this process; a respawn reads
              // the choice back from here (INC-2026-0099).
              rememberEffortSelection(session.cliConversationId, msg.effort)
              ws.send(
                JSON.stringify({
                  type: 'session:model-changed',
                  sessionId: authenticatedSessionId,
                  model: session.model,
                  effort: msg.effort,
                  inPlace: true,
                }),
              )
              debugLog('Effort changed in place', { sessionId: authenticatedSessionId, effort: msg.effort })
              return
            }
          }
          const operation = beginStartup('effort_restart')
          try {
            const oldSessionId = authenticatedSessionId
            // Preserve the negotiated stream protocol across the PTY restart —
            // the same client is still on this WS, it just won't re-handshake.
            const preservedProto = session.streamProtocol ?? 0

            // Remove old session from WS map
            sessionWsMap.delete(oldSessionId)
            authenticatedSessionId = null

            let newSession: PtySession
            if (session.cliConversationId) {
              // Has conversation → restart with resume + new effort
              newSession = await restartWithEffort(oldSessionId, msg.effort, ws, authenticatedUser!, session.tokenId)
            } else {
              // No conversation yet (empty chat) → recreate fresh with effort, preserve model
              session.isRestarting = true
              const preservedModel = session.model
              destroySession(oldSessionId, 'effort_restart')
              newSession = await createSession(ws, authenticatedUser!, session.tokenId, {
                cwd: session.cwd,
                cols: 120,
                rows: 40,
                effort: msg.effort,
                model: preservedModel || undefined,
                protocolVersion: preservedProto,
                host: session.host,
              })
            }
            if (!ownsStartup(operation)) {
              releaseLateSession(newSession, operation)
              return
            }
            newSession.streamProtocol = preservedProto

            // Update tracked session
            authenticatedSessionId = newSession.id
            sessionWsMap.set(newSession.id, ws)
            authenticatedTokenId = session.tokenId
            if (!tokenWsMap.has(session.tokenId)) tokenWsMap.set(session.tokenId, new Set())
            tokenWsMap.get(session.tokenId)!.add(ws)

            // Notify client about the restart (include both effort + model for UI sync)
            ws.send(
              JSON.stringify({
                type: 'session:restarted',
                sessionId: newSession.id,
                mcpTaskOwner: newSession.mcpEndpoint?.id,
                effort: newSession.effort || msg.effort,
                model: newSession.model,
              }),
            )

            publishClaudePermissions(newSession)
            debugLog('Session restarted with new effort', {
              oldSessionId,
              newSessionId: newSession.id,
              effort: msg.effort,
            })
          } catch (err) {
            if (!ownsStartup(operation)) return
            const errMsg = err instanceof Error ? err.message : String(err)
            errorLog('Failed to change effort', { error: errMsg })
            sendError(ws, `Failed to change effort: ${errMsg}`)
          } finally {
            if (startup === operation) startup = undefined
          }
          break
        }

        // A model of the other engine picked in this chat: the conversation
        // continues on that engine, in this chat (conversation/manifest.ts).
        case 'session:switch-engine': {
          if (!authenticatedSessionId || !authenticatedUser) {
            sendError(ws, 'Not authenticated — send session:create first')
            return
          }
          const session = getSession(authenticatedSessionId)
          if (!session) {
            sendError(ws, 'Session not found')
            return
          }
          const refuse = (error: string) => ws.send(JSON.stringify({ type: 'session:engine-switch-failed', error }))
          const model = typeof msg.model === 'string' && msg.model ? msg.model : null
          if (!isAgentEngineId(msg.engine) || (model !== null && !isSafeModelId(model))) {
            refuse('Unsupported engine or model')
            return
          }
          if ((session.engine ?? 'claude') === msg.engine) {
            refuse('This chat already runs on that engine')
            return
          }
          // The gate goes up before the first await: a prompt sent meanwhile
          // waits for the joining engine instead of reaching the leaving one,
          // and a second switch waits, then finds the chat already moved.
          let switched!: () => void
          engineSwitch = new Promise<void>((resolve) => (switched = resolve))
          const oldSessionId = authenticatedSessionId
          let operation: ReturnType<typeof beginStartup> | undefined
          try {
            const refusal = await mainAgentRefusal(msg.engine, { runtime: codexRuntime() })
            if (refusal) {
              refuse(refusal)
              return
            }
            operation = beginStartup('engine_switch')
            sessionWsMap.delete(oldSessionId)
            authenticatedSessionId = null
            const {
              session: next,
              timings,
              contextBytes,
              carried,
            } = await switchSessionEngine(
              oldSessionId,
              { engine: msg.engine, model },
              ws,
              authenticatedUser,
              session.tokenId,
            )
            if (!ownsStartup(operation)) {
              releaseLateSession(next, operation)
              return
            }
            authenticatedSessionId = next.id
            sessionWsMap.set(next.id, ws)
            ws.send(
              JSON.stringify({
                type: 'session:restarted',
                sessionId: next.id,
                mcpTaskOwner: next.mcpEndpoint?.id,
                codexCollaboration: codexBinding(next)?.agent.collaboration,
                effort: next.effort,
                model: next.model || model,
                engine: next.engine ?? 'claude',
                engineSwitch: { ...timings, contextBytes },
              }),
            )
            publishClaudePermissions(next)
            codexBinding(next)?.pushPlanLimits()
            // The leaving CLI's queue, after the client knows the new session
            // and before the gate lets later prompts through (their order).
            for (const item of carried.returned)
              sendToClient(ws, {
                type: 'session:submission',
                sessionId: next.id,
                id: item.id,
                text: item.text,
                state: 'cancelled',
                rejected: true,
                cancelable: false,
                warning: SWITCH_RETURNED_WARNING,
                createdAt: item.createdAt,
              })
            for (const item of carried.moved) submitText(next.id, item.text, item.id)
            void sendAgentPreferences(ws, session.tokenId)
            broadcastTree(session.tokenId)
          } catch (err) {
            if (!operation || !ownsStartup(operation)) return
            const errMsg = err instanceof Error ? err.message : String(err)
            errorLog('Failed to switch engine', { error: errMsg })
            // The switch commits only once the joining engine runs: on a
            // failure the leaving one is still there and the tab stays on it.
            if (getSession(oldSessionId)) {
              authenticatedSessionId = oldSessionId
              sessionWsMap.set(oldSessionId, ws)
            }
            refuse(`Failed to switch engine: ${errMsg}`)
          } finally {
            if (operation && startup === operation) startup = undefined
            engineSwitch = null
            switched()
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
          if (!isSafeModelId(msg.model)) {
            sendError(ws, 'Unsupported model id')
            return
          }
          // Успех объявляется ТОЛЬКО если команда действительно легла в stdin.
          // На неживом PTY запись молча терялась, а плашка уже показывала новую
          // модель (INC-2026-0053) — такой случай уходит на честный перезапуск ниже.
          const codexModel = codexBinding(session)
          if (codexModel) {
            // Same thread, next turn: codex applies turn/start.model itself. A chat
            // keeps its engine, so a Claude id here leaves the model unchanged.
            const next = toCodexModel(msg.model)
            if (next) {
              await codexModel.agent.setModel(next)
              codexModel.modelChanged(next)
              session.model = next
              const level = effectiveEffort(next, session.effort, codexRuntime())
              codexModel.agent.setEffort(level)
              session.effort = level ?? null
              saveCodexChoice(session, { model: next, effort: level })
              if (session.conversation) noteSegmentModel(session.conversation, next)
            }
            ws.send(
              JSON.stringify({
                type: 'session:model-changed',
                sessionId: authenticatedSessionId,
                model: session.model,
                effort: session.effort,
                inPlace: true,
              }),
            )
            return
          }
          if (switchClaudeModelInPlace(session, authenticatedSessionId, msg.model, writeInput)) {
            if (session.conversation) noteSegmentModel(session.conversation, msg.model)
            ws.send(
              JSON.stringify({
                type: 'session:model-changed',
                sessionId: authenticatedSessionId,
                model: msg.model,
                effort: session.effort,
                // Горячая смена PTY не перезапускает — консоль чистить нечего.
                inPlace: true,
              }),
            )
            return
          }
          const operation = beginStartup('model_restart')
          try {
            const oldSessionId = authenticatedSessionId
            // Preserve the negotiated stream protocol across the PTY restart.
            const preservedProto = session.streamProtocol ?? 0

            // Remove old session from WS map
            sessionWsMap.delete(oldSessionId)
            authenticatedSessionId = null

            let newSession: PtySession
            if (session.cliConversationId) {
              // Has conversation → restart with resume + new model
              newSession = await restartWithModel(oldSessionId, msg.model, ws, authenticatedUser!, session.tokenId)
            } else {
              // No conversation yet (empty chat) → recreate fresh with model, preserve effort
              session.isRestarting = true
              const preservedEffort = session.effort
              destroySession(oldSessionId, 'model_restart')
              newSession = await createSession(ws, authenticatedUser!, session.tokenId, {
                cwd: session.cwd,
                cols: 120,
                rows: 40,
                model: msg.model,
                effort: preservedEffort || undefined,
                protocolVersion: preservedProto,
                host: session.host,
              })
            }
            if (!ownsStartup(operation)) {
              releaseLateSession(newSession, operation)
              return
            }
            newSession.streamProtocol = preservedProto

            // Update tracked session
            authenticatedSessionId = newSession.id
            sessionWsMap.set(newSession.id, ws)
            authenticatedTokenId = session.tokenId
            if (!tokenWsMap.has(session.tokenId)) tokenWsMap.set(session.tokenId, new Set())
            tokenWsMap.get(session.tokenId)!.add(ws)

            // Notify client about the restart (include both effort + model for UI sync)
            ws.send(
              JSON.stringify({
                type: 'session:restarted',
                sessionId: newSession.id,
                mcpTaskOwner: newSession.mcpEndpoint?.id,
                effort: newSession.effort,
                model: newSession.model || msg.model,
              }),
            )

            publishClaudePermissions(newSession)
            debugLog('Session restarted with new model', {
              oldSessionId,
              newSessionId: newSession.id,
              model: msg.model,
            })
          } catch (err) {
            if (!ownsStartup(operation)) return
            const errMsg = err instanceof Error ? err.message : String(err)
            errorLog('Failed to change model', { error: errMsg })
            sendError(ws, `Failed to change model: ${errMsg}`)
          } finally {
            if (startup === operation) startup = undefined
          }
          break
        }

        case 'session:delete-data': {
          if (!authenticatedUser || !authenticatedTokenId) {
            warnLog('Session data deletion refused: unauthenticated connection')
            sendError(ws, 'Authentication required to delete session data')
            break
          }
          const convId = msg.conversationId
          if (convId) {
            try {
              await deleteSessionByConversationId(convId, authenticatedUser, authenticatedTokenId)
              debugLog('Session data deleted', { conversationId: convId })
            } catch (err) {
              warnLog('Failed to delete session data', { conversationId: convId, error: String(err) })
            }
          }
          break
        }

        case 'mcp:register-external-tools': {
          // Client sends external MCP tool schemas to merge into the session tool list.
          if (!authenticatedSessionId) return
          const session = getSession(authenticatedSessionId)
          if (!session) return

          const externalTools = (msg as any).tools as Array<{
            name: string
            description: string
            inputSchema: Record<string, unknown>
          }>
          if (!Array.isArray(externalTools)) return

          // This message is a complete external-tool snapshot. Replacing the
          // array removes tools from disabled/uninstalled/disconnected servers;
          // the previous name-based merge kept every disappeared old tool.
          const catalogChanged = replaceRegisteredTools(session, externalTools, 'register-external-tools')

          debugLog('External MCP tools registered', {
            catalogChanged,
            sessionId: authenticatedSessionId,
            externalCount: externalTools.length,
            totalCount: session.registeredTools.length,
            tools: externalTools.map((t) => t.name),
          })
          break
        }

        case 'mcp:register-external-content': {
          if (!authenticatedSessionId) return
          const session = getSession(authenticatedSessionId)
          if (!session) return
          session.registeredResources = Array.isArray(msg.resources) ? msg.resources : []
          session.registeredResourceTemplates = Array.isArray(msg.resourceTemplates) ? msg.resourceTemplates : []
          session.registeredPrompts = Array.isArray(msg.prompts) ? msg.prompts : []
          debugLog('External MCP content registered', {
            sessionId: authenticatedSessionId,
            resourceCount: session.registeredResources.length,
            resourceTemplateCount: session.registeredResourceTemplates.length,
            promptCount: session.registeredPrompts.length,
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
          const { sinceHead, sincePos, lastPos, lastUuid } = msg as unknown as {
            sinceHead?: string
            sincePos?: number
            lastPos?: number
            lastUuid?: string
          }
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

        case 'jsonl:subagent-select': {
          if (!authenticatedSessionId) return
          getSession(authenticatedSessionId)?.jsonlWatcher?.selectSubagent?.(msg.agentId)
          break
        }

        case 'jsonl:download-request': {
          if (!authenticatedSessionId) {
            sendError(ws, 'Not authenticated')
            return
          }
          const dlSession = getSession(authenticatedSessionId)
          if (!dlSession?.jsonlWatcher) {
            ws.send(
              JSON.stringify({
                type: 'jsonl:download-response',
                content: null,
                fileName: null,
                error: 'No active JSONL watcher',
              }),
            )
            return
          }
          const mainPath = dlSession.jsonlWatcher.getFilePath()
          const filePath = msg.agentId
            ? await subagentTranscriptPath(mainPath, msg.agentId, dlSession.conversationAliases)
            : mainPath
          if (!filePath) {
            ws.send(
              JSON.stringify({
                type: 'jsonl:download-response',
                content: null,
                fileName: null,
                error: 'JSONL file not yet discovered',
              }),
            )
            return
          }
          try {
            // Streamed: the transcript (often 100+ MiB) is never held whole —
            // one bounded chunk is read, decoded and sent at a time, under the
            // same drain gate. `total`/`sent` are BYTES of the file snapshot
            // taken now (the client only uses them for progress); a UTF-8
            // character split by a chunk edge is carried by the decoder.
            const fileName = path.basename(filePath)
            const total = (await fsp.stat(filePath)).size
            if (total <= DOWNLOAD_CHUNK_BYTES) {
              const content = await fsp.readFile(filePath, 'utf-8')
              ws.send(JSON.stringify({ type: 'jsonl:download-response', content, fileName }))
            } else {
              ws.send(JSON.stringify({ type: 'jsonl:download-begin', fileName, total }))
              // Обратное давление БЕЗ ПОТЕРЬ. Раньше куски уходили подряд, без
              // ожидания опустошения буфера и без отмены при разрыве: на
              // медленном соединении выгрузка копилась поверх обычного трафика
              // сессии (INC-2026-0019). Бросать кусок, как это делает
              // `sendToClient` за пределом, здесь нельзя — у выгрузки нет
              // повторной доставки, и брошенный кусок означает молча
              // испорченный файл.
              const aborted = await streamFileChunks(ws, filePath, total, DOWNLOAD_CHUNK_BYTES, DOWNLOAD_DRAIN)
              if (aborted === null) {
                ws.send(JSON.stringify({ type: 'jsonl:download-end', fileName, total }))
              } else if (aborted === 'timeout' && ws.readyState === ws.OPEN) {
                // Молчания быть не должно: клиент обязан узнать, что файл
                // неполон, а не остаться с оборванной выгрузкой.
                ws.send(
                  JSON.stringify({
                    type: 'jsonl:download-response',
                    content: null,
                    fileName: null,
                    error: 'Download aborted: the connection did not drain',
                  }),
                )
              }
            }
          } catch (err) {
            ws.send(
              JSON.stringify({
                type: 'jsonl:download-response',
                content: null,
                fileName: null,
                error: `Failed to read file: ${String(err)}`,
              }),
            )
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
          if (msg.paged) {
            if (msg.beforePos !== undefined && (!Number.isSafeInteger(msg.beforePos) || msg.beforePos < 0)) {
              sendError(ws, 'Invalid history cursor')
              break
            }
            const sessionId = authenticatedSessionId
            const watcher = getSession(sessionId)?.jsonlWatcher
            const file = watcher?.getFilePath()
            try {
              const range = fromTs || toTs ? await watcher?.segmentRange?.(fromTs, toTs) : { lower: 0, upper: Infinity }
              const page =
                file && range
                  ? await readHistoryPage(file, Math.min(msg.beforePos ?? Infinity, range.upper), range.lower)
                  : { records: [], nextPos: 0, bytesRead: 0 }
              // A switch or rewrite invalidates the positions captured above.
              if (
                authenticatedSessionId !== sessionId ||
                getSession(sessionId)?.jsonlWatcher !== watcher ||
                watcher?.getFilePath() !== file
              )
                break
              if (ws.readyState === ws.OPEN)
                ws.send(
                  JSON.stringify({
                    type: 'jsonl:segment-response',
                    fromTs,
                    toTs,
                    ...page,
                    ...(msg.beforePos === undefined ? {} : { beforePos: msg.beforePos }),
                  }),
                )
            } catch {
              if (ws.readyState === ws.OPEN) sendError(ws, 'History page unavailable')
            }
            break
          }
          const segPath = getSession(authenticatedSessionId)?.jsonlWatcher?.getFilePath()
          const records: JsonlEntry[] = []
          let truncated = false
          if (segPath) {
            // Streamed and cancellable: the transcript is never held as one
            // string or line array, the reader yields to the event loop, and
            // the read stops when the requesting socket closes. The response
            // stays one frame by protocol, so its source bytes are capped well
            // under the per-connection send cap; past it the reply says so.
            const abort = new AbortController()
            let bytes = 0
            try {
              for await (const line of readJsonlLines(segPath, { signal: abort.signal })) {
                if (ws.readyState !== ws.OPEN) {
                  abort.abort()
                  break
                }
                if (line.text.includes(SKIP_LINE_MARKER)) continue
                let r: JsonlEntry
                try {
                  r = JSON.parse(line.text) as JsonlEntry
                } catch {
                  continue
                }
                const ts = r.timestamp
                if (!ts) continue // ts-less rows can't be placed on the range axis
                if ((r as { subtype?: string }).subtype === 'compact_boundary') continue
                if (fromTs && ts < fromTs) continue
                if (toTs && ts >= toTs) continue
                bytes += line.end - line.start
                if (bytes > SEGMENT_RESPONSE_MAX_BYTES) {
                  truncated = true
                  break
                }
                records.push(r)
              }
            } catch {
              /* fall through — empty result tells the client to reset the view */
            }
          }
          if (ws.readyState !== ws.OPEN) break
          ws.send(
            JSON.stringify({
              type: 'jsonl:segment-response',
              fromTs,
              toTs,
              records,
              ...(truncated ? { truncated } : {}),
            }),
          )
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
      socketGone = true
      cancelStartup('disconnected')
      dropTokenIndex()
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

// Listen for tree changes and broadcast to affected clients.
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

eventBus.on('session:destroyed', (event: any) => {
  const data = event?.data ?? event
  const socket = sessionWsMap.get(data?.sessionId)
  sessionWsMap.delete(data?.sessionId)
  if (socket && ![...sessionWsMap.values()].includes(socket)) {
    const clients = tokenWsMap.get(data?.tokenId)
    clients?.delete(socket)
    if (clients?.size === 0) tokenWsMap.delete(data.tokenId)
  }
  if (data?.tokenId) broadcastTree(data.tokenId)
})

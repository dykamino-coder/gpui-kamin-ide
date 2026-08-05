// Server-message dispatcher extracted from connection-manager.ts
// (Sprint 5 / Stage E1). The switch was the largest single block in the
// file (~244 LOC). We move it here, taking a thin context bag so the
// handler can poke back into the manager (callbacks, state setters,
// batcher, IPC channel).

import type { BrowserWindow } from '@kaminide/host-compat'
import type { ServerMessage } from '../../shared/mcp-protocol'
import type { JsonlBatcher } from './jsonl-batcher'
import type { ExtendedConnectionState } from './connection-manager'

export interface HandlerCtx {
  window: BrowserWindow
  tabId: string
  batcher: JsonlBatcher

  setState: (s: ExtendedConnectionState) => void
  setLastModel: (m: string) => void
  setSettingsDir: (d: string) => void
  setConversationId: (id: string) => void
  scheduleReconnect: () => void
  resetReconnectAttempt: () => void
  /** Ghost-resume loop breaker: mark session:created, and on session:exit decide
   *  whether to drop the (dead) conversationId before the next resume. */
  noteSessionCreated: () => void
  noteSessionExit: () => void
  /** Server answered a resume with «conversation not found» and started a
   *  fresh one: drop the ghost conversationId + old mirror, tell the user. */
  noteResumeNotFound: () => void
  clearJsonlCache: () => void
  resolveDownload: (r: { content: string | null; fileName: string | null; error?: string }) => void
  /** Identity of the transcript file, so the mirror can be trusted later. */
  noteTranscriptFingerprint: (head: string) => void
  /** The file was rewritten — throw the local copy away rather than extend it. */
  resetTranscriptMirror: () => void
  beginDownload: (total: number) => void
  appendDownloadChunk: (chunk: string, sent: number, total: number) => void
  endDownload: (fileName: string | null) => void
  /** `replay` — серверный флаг батча истории/превью (вне файла-порядка):
   *  зеркалу такой батч отдавать нельзя, курсор прыгнет на конец файла и
   *  недосланный промежуток навсегда отфильтруется как «дубль». */
  pushJsonlCache: (entries: any[], replay?: boolean) => void
  getCachedJsonl: () => any[]
  setCachedStreamingEntry: (entry: any) => void
  pushSubagentCache: (key: string, agentName: string, agentId: string | undefined, entries: any[]) => void
  setCachedJsonlStatus: (status: any) => void
  /** Server's authoritative compact-segment index (arrives with replayComplete).
   *  Cached so an on-demand `loadBoundaries` re-request can answer without a
   *  round-trip; cleared on compaction (the file changed). */
  setSegmentIndex: (index: { boundaries: { ts: string }[]; counts: number[] } | null) => void
  isIntentionallyDisconnected: () => boolean
  /** Текущий статус соединения (для авто-сброса не-фатального 'error', C16). */
  getStatus: () => string
  /** Background (warm-pool) tab — live streaming has no render target. */
  isRendererMuted?: () => boolean
  notifyAuthenticated: () => void
  notifyElicitationRequest: (payload: { requestId: string; toolName: string; message: string }) => void
  notifySessionTitle: (title: string) => void
  notifyConversationId: (id: string) => void
  notifySessionInfo: (effort?: string, model?: string) => void
  notifyTreeUpdate: (tree: any[]) => void
  trackActivityForIdle: (rawTitle: string | undefined, isWorking: boolean) => void
  /** Реплей закрыт — снять settle-окно idle-трекера (C17). */
  noteReplayCompleteForIdle: () => void
  notifyActivity: (working: boolean, hookDriven: boolean) => void

  handleMcpCall: (msg: { requestId: string; toolName: string; input: Record<string, unknown> }) => void
  handleHookExecute: (msg: any) => void
}

const JSONL_CACHE_MAX = 20_000
/** Trim in blocks so the cost is amortised: once the cap is reached EVERY
 *  arriving batch would otherwise re-trim. */
const JSONL_CACHE_SLACK = 1_000

/** Drop the oldest entries BY FILE ORDER (`_ord`), not by arrival order.
 *
 *  This used to be `splice(0, over)` — drop from the front — which was correct
 *  only while arrival order matched file order. The replay now sends the CURRENT
 *  dialog first (newest turns first, so the chat paints where the user is
 *  looking), which puts the freshest entries at the FRONT: dropping from there
 *  would evict exactly the conversation in view, and the next renderer reload
 *  would replay old history with today's turns missing. */
export function trimJsonlCache(cached: { _ord?: number }[]): void {
  const over = cached.length - JSONL_CACHE_MAX
  if (over < JSONL_CACHE_SLACK) return
  // Sorting the ORDS (plain numbers) is far cheaper than sorting the entry
  // objects, and `_ord` is unique per entry, so the cutoff drops exactly `over`.
  const ords = cached.map((e) => e._ord ?? 0).sort((a, b) => a - b)
  const cutoff = ords[over - 1] ?? 0
  const kept = cached.filter((e) => (e._ord ?? 0) > cutoff)
  // Не push(...kept): spread на ~20k элементов упирается в лимит аргументов V8.
  cached.length = kept.length
  for (let i = 0; i < kept.length; i++) cached[i] = kept[i]
}

export function handleServerMessage(msg: ServerMessage, ctx: HandlerCtx): void {
  switch (msg.type) {
    case 'session:created': {
      ctx.resetReconnectAttempt()
      ctx.noteSessionCreated()
      ctx.clearJsonlCache()
      // NOTE: do NOT send `{compacted:true}` here. session:created fires on
      // every reconnect/restart; renderer's appendJsonlEntries already
      // dedupes by uuid, so a replay of cached entries is a no-op when the
      // tab already has them. Sending compacted=true wiped the chat and
      // caused a visible flash before replay re-populated. Real CLI
      // compaction is handled server-side via jsonl:status compacted:true.
      ctx.window.webContents.send('jsonl-status', ctx.tabId, { status: 'searching' })
      // Resume target is gone on this server — the session is FRESH. Forget
      // the ghost conversationId (a reconnect must not re-resume it) and the
      // old transcript mirror; the server follows up with an instant
      // replayComplete so the chat shows empty instead of "Loading…".
      if ((msg as any).resumeNotFound) ctx.noteResumeNotFound()
      ctx.setState({ status: 'authenticated', sessionId: msg.sessionId })
      const settingsDir = (msg as any).settingsDir
      if (settingsDir) ctx.setSettingsDir(settingsDir)
      if (msg.effort || msg.model) {
        ctx.window.webContents.send('session-restarted', ctx.tabId, {
          sessionId: msg.sessionId,
          effort: msg.effort,
          model: msg.model,
        })
      }
      ctx.notifySessionInfo(msg.effort, msg.model)
      ctx.notifyAuthenticated()
      break
    }

    case 'session:output':
      ctx.window.webContents.send('pty-output', ctx.tabId, msg.data)
      break

    case 'session:exit':
      ctx.window.webContents.send('pty-exit', ctx.tabId, msg.code)
      ctx.setState({ status: 'disconnected' })
      // Break the ghost-resume blink BEFORE the reconnect: if this was a resume
      // that authenticated then died fast, drop the dead conversationId so the
      // next attempt starts fresh instead of re-resuming the same dead id.
      ctx.noteSessionExit()
      // Session ended — schedule reconnect so user can start a new session.
      if (!ctx.isIntentionallyDisconnected()) ctx.scheduleReconnect()
      break

    case 'session:error':
      // Не-фатальная серверная ошибка БЕЗ закрытия сокета красила статус в
      // 'error' навсегда — красный дот жил до ручного реконнекта (аудит #70
      // C16). Живой сокет = сессия работает: показываем ошибку и через 5с
      // возвращаемся в 'connected', если ничего нового не случилось.
      ctx.setState({ status: 'error', error: msg.error })
      setTimeout(() => {
        if (ctx.getStatus() === 'error' && !ctx.isIntentionallyDisconnected()) {
          ctx.setState({ status: 'connected', error: undefined })
        }
      }, 5000)
      break

    case 'mcp:call':
      ctx.handleMcpCall(msg)
      break

    case 'hook:execute':
      ctx.handleHookExecute(msg)
      break

    case 'streaming:entry': {
      // Live MITM-proxy assistant entry. Forward immediately to renderer
      // (no batching — latency matters for word-by-word render). Renderer
      // dedups by message.id.
      const entry = (msg as any).entry
      if (entry) {
        // Background (warm-pool) tab: nobody renders its live stream — keep the
        // pipe free (the cached stub still replays on activation).
        if (!ctx.isRendererMuted?.()) ctx.window.webContents.send('streaming-entry', ctx.tabId, entry)
        // Cache the in-flight stub so a webview reload mid-turn can re-show it
        // (replayJsonlToRenderer re-emits it). Clear on the final non-streaming
        // entry so a completed turn's stub isn't resurrected.
        ctx.setCachedStreamingEntry?.(entry.__streaming ? entry : null)
      }
      break
    }

    case 'streaming:delta': {
      // Incremental text/thinking append between boundary snapshots (new stream
      // protocol). Forward immediately — same latency rationale as entry.
      if (ctx.isRendererMuted?.()) break // background tab: no live render target
      const d = msg as any
      ctx.window.webContents.send('streaming-delta', ctx.tabId, {
        msgId: d.msgId, blockIdx: d.blockIdx, appendText: d.appendText, kind: d.kind,
      })
      break
    }

    case 'streaming:status': {
      // Rate-limit (429) / overload (5xx) from the MITM proxy — the CLI is
      // retrying under the hood; surface it so the chat isn't silently stalled.
      ctx.window.webContents.send('streaming-status', ctx.tabId, {
        code: (msg as any).code,
        retryAfterMs: (msg as any).retryAfterMs,
      })
      break
    }

    case 'jsonl:entries': {
      const entries = (msg as any).entries as any[]
      // Cache entries for replay after renderer reload. Capped — long
      // sessions accumulate tens of thousands of records; without a ceiling
      // the main process holds them all in RAM forever and each renderer
      // reload replays the full backlog over IPC. 20000 covers the longest
      // realistic single-session conversation while keeping the cache
      // <50MB on assistant-heavy streams.
      ctx.pushJsonlCache(entries, (msg as any).replay === true)
      trimJsonlCache(ctx.getCachedJsonl())
      ctx.batcher.queueMain(entries)
      // Cache last model from assistant entries
      for (let i = entries.length - 1; i >= 0; i--) {
        if (entries[i].type === 'assistant' && entries[i].model) {
          ctx.setLastModel(entries[i].model)
          break
        }
      }
      break
    }

    case 'jsonl:status': {
      const statusData = {
        status: (msg as any).status,
        filePath: (msg as any).filePath,
        error: (msg as any).error,
        compacted: (msg as any).compacted,
        replayComplete: (msg as any).replayComplete,
        // Live byte-progress of the historical replay → the renderer's "loading
        // history … N%". Transient like `compacted`: NOT cached below, or a
        // renderer reload would replay a frozen bar at whatever % it last saw.
        replayProgress: (msg as any).replayProgress,
      }
      // The transcript's identity, handed over at replay-complete. It is what
      // lets a local mirror prove, on the next launch, that its copy is still a
      // prefix of the server's file rather than of one that has been rewritten.
      const fileHead = (msg as any).fileHead as string | undefined
      if (fileHead) ctx.noteTranscriptFingerprint(fileHead)
      // C17: settle-окно idle-трекера держится до конца реплея — фикс. 4с не
      // хватало крупным сессиям, и stale-блип тостил «finished».
      if (statusData.replayComplete) ctx.noteReplayCompleteForIdle()
      // On compaction, clear cached entries (new JSONL file). Also drop any
      // in-flight batch — those entries belong to the pre-compact file. The
      // mirror goes too: a compacted file is a DIFFERENT file, and extending
      // the old copy would splice the new conversation onto the old one.
      if (statusData.compacted) {
        ctx.clearJsonlCache()
        ctx.batcher.clear()
        ctx.resetTranscriptMirror()
        ctx.setSegmentIndex(null) // stale: the compacted file is a new conversation
      }
      // The authoritative compact-segment index, built server-side from the full
      // transcript and delivered with replayComplete. Cache it (for an on-demand
      // re-request) and push it straight to the strip on its own channel — the
      // client no longer computes this from its bounded local mirror.
      const segmentIndex = (msg as any).segmentIndex as { boundaries: { ts: string }[]; counts: number[] } | undefined
      if (segmentIndex) {
        ctx.setSegmentIndex(segmentIndex)
        ctx.window.webContents.send('jsonl:boundaries-result', ctx.tabId, segmentIndex)
      }
      // Flush pending batched entries BEFORE forwarding a transition-
      // marking status — otherwise `replayComplete` arrives in the renderer
      // ahead of the straggling replay entries batched behind the timer,
      // and the renderer treats those historical entries as live.
      if (statusData.replayComplete || statusData.compacted) {
        ctx.batcher.flushMain()
        ctx.batcher.flushSubagent()
      }
      // NEVER cache `compacted`. It is a one-shot EVENT — "the CLI just switched
      // to a new transcript file, drop what you have" — not a state to replay.
      // Cached, it became a self-sustaining wipe loop: replayJsonlToRenderer
      // sends entries and then re-emits this status LAST, so the webview wiped
      // the store the replay had just filled; the 2.5s watchdog saw an empty
      // store and asked for another replay; repeat forever. That is the reported
      // "switch to another session, come back, and everything's cleared".
      // It never self-healed because the only recovery the webview has is the
      // very call that re-triggers the wipe — and the flag never cleared on its
      // own, since a replay that aborts (reap / reattach / next switchToFile)
      // returns without ever sending the `replayComplete` that would replace it.
      const { compacted: _oneShot, replayProgress: _transient, ...cacheableStatus } = statusData
      ctx.setCachedJsonlStatus(cacheableStatus)
      ctx.window.webContents.send('jsonl-status', ctx.tabId, statusData)
      break
    }

    case 'jsonl:segment-response': {
      // One archived segment, read by the server from the authoritative file →
      // straight to the renderer's "view an old conversation" channel. Stateless:
      // the response carries its own range, so no pending-request bookkeeping.
      ctx.window.webContents.send('jsonl:segment-page', ctx.tabId, {
        fromTs: (msg as any).fromTs,
        toTs: (msg as any).toTs,
        records: (msg as any).records ?? [],
      })
      break
    }

    case 'jsonl:subagent-entries': {
      const agentName = (msg as any).agentName as string
      const agentId = (msg as any).agentId as string | undefined
      const subEntries = (msg as any).entries as any[]
      // Cache subagent entries by agentId (unique per file) to avoid
      // collisions when multiple agents have the same name (e.g. two bobs
      // in different teams).
      const cacheKey = agentId || agentName
      ctx.pushSubagentCache(cacheKey, agentName, agentId, subEntries)
      ctx.batcher.queueSubagent(cacheKey, agentName, agentId, subEntries)
      break
    }

    case 'jsonl:download-response':
      ctx.resolveDownload({
        content: (msg as any).content as string | null,
        fileName: (msg as any).fileName as string | null,
        error: (msg as any).error as string | undefined,
      })
      break

    // Chunked transfer for a big transcript. One 36MB frame meant one huge
    // stringify on the server and one huge parse here, with nothing to report
    // in between — the UI could only spin. Chunks make the progress real.
    case 'jsonl:download-begin':
      ctx.beginDownload((msg as any).total as number)
      break

    case 'jsonl:download-chunk':
      ctx.appendDownloadChunk(
        (msg as any).chunk as string,
        (msg as any).sent as number,
        (msg as any).total as number,
      )
      break

    case 'jsonl:download-end':
      ctx.endDownload((msg as any).fileName as string | null)
      break

    case 'elicitation:request': {
      const requestId = (msg as any).requestId as string
      const toolName = (msg as any).toolName as string
      const message = (msg as any).message as string
      ctx.window.webContents.send('elicitation-request', ctx.tabId, {
        requestId, toolName, message,
        requestedSchema: (msg as any).requestedSchema,
      })
      ctx.notifyElicitationRequest({ requestId, toolName, message })
      break
    }

    case 'session:agent-spawned':
      ctx.window.webContents.send('agent-spawned', ctx.tabId, {
        parentSessionId: (msg as any).parentSessionId,
        childSessionId: (msg as any).childSessionId,
        agentName: (msg as any).agentName,
        description: (msg as any).description,
      })
      break

    case 'session:title': {
      const title = (msg as any).title as string
      if (title) {
        ctx.window.webContents.send('session:title', ctx.tabId, title)
        ctx.notifySessionTitle(title)
      }
      break
    }

    case 'session:conversation-id': {
      const conversationId = (msg as any).conversationId as string
      if (conversationId) {
        ctx.setConversationId(conversationId)
        ctx.notifyConversationId(conversationId)
      }
      break
    }

    case 'session:tree-update':
      ctx.notifyTreeUpdate((msg as any).tree)
      break

    case 'session:activity': {
      const m = msg as any
      const rawTitle = m.rawTitle as string | undefined
      const isWorking = !!m.isWorking
      const hookDriven = m.hookDriven === true
      // Forward the FULL payload. bridge-status-hooks push session:activity with
      // hookDriven/promptReady/waiting as the AUTHORITATIVE working+waiting state
      // (UserPromptSubmit/Stop/Notification/…). The old forward sent only
      // rawTitle+isWorking, so the webview never saw hookDriven → it stayed on
      // the flappy OSC-spinner heuristic (the unreliable-spinner bug). Passing
      // hookDriven to notifyActivity makes the native session-row dot
      // hook-authoritative too (see SessionsBridge.onActivity).
      ctx.window.webContents.send('session:activity', ctx.tabId, {
        rawTitle, isWorking, hookDriven,
        ...(typeof m.promptReady === 'boolean' ? { promptReady: m.promptReady } : {}),
        ...(typeof m.waiting === 'boolean' ? { waiting: m.waiting } : {}),
        ...(typeof m.lastMessage === 'string' ? { lastMessage: m.lastMessage } : {}),
      })
      ctx.trackActivityForIdle(rawTitle, isWorking)
      ctx.notifyActivity(isWorking, hookDriven)
      break
    }

    case 'session:model-changed':
      // Горячая смена модели (/model в живой PTY): sessionId прежний, реплей
      // и JSONL-кэш НЕ трогаем — обновляем только UI-плашки.
      ctx.window.webContents.send('session-restarted', ctx.tabId, {
        sessionId: (msg as any).sessionId,
        effort: (msg as any).effort,
        model: (msg as any).model,
      })
      ctx.notifySessionInfo((msg as any).effort, (msg as any).model)
      break

    case 'session:restarted':
      ctx.resetReconnectAttempt()
      ctx.clearJsonlCache()
      ctx.setState({ status: 'authenticated', sessionId: (msg as any).sessionId })
      ctx.window.webContents.send('session-restarted', ctx.tabId, {
        sessionId: (msg as any).sessionId,
        effort: (msg as any).effort,
        model: (msg as any).model,
      })
      ctx.notifySessionInfo((msg as any).effort, (msg as any).model)
      break
  }
}

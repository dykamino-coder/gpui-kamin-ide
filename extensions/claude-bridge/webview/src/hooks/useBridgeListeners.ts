import { useEffect, useRef } from 'preact/hooks'

import type { KaminBridgeApi } from '../../shared/types'
import type { TabInfo, TreeNode } from '../../shared/types'
import type { ElicitationRequest, PermissionRequest } from '../../shared/types'

// Signals
import { tabs, activeTabId, sessionTree, pinnedTabs, applyTabSwitch, switchTabLocal } from '../signals/tabs'
import { markOpen } from '../signals/open-timeline'
import { scheduleSaveTabsState, ghostifyClosedPinned, reclaimGhostIfMatch } from '../lib/tabs-persist'
import { mcpLoading, tabPromptReady, lastSendAt, reconnectNonce } from '../signals/connection'
import { mcpServers } from '../signals/customize'
import { tabActivity, tabWaiting, sidebarMode, activeCustomizePanel } from '../signals/ui'
import { hostEditorSelection } from '../signals/file-viewer'
import { showToast } from '../signals/toasts'
import { activeWidgets } from '../signals/widgets'
import { tabAgentTrees, tabJsonlLive, subagentTileState, clearAgentTabState } from '../signals/agents'

// Terminal registry for bridge.onOutput routing
import { terminalRegistry, resetConsoleForReconnect } from '../signals/terminal-registry'
import { splitTrailingEscape, stripMouseTracking } from '../lib/strip-mouse-tracking'

// Agent tree helpers
import { parseAgentEntries, markAllAgentsExited, scheduleCleanup, mergeAgentTree, touchAgentAlive } from './useAgentTree'

// Queue: auto-send queued messages when CLI becomes idle
import { appendJsonlEntries, applyStreamingEntry, applyStreamingDelta, clearJsonlEntries, jsonlEntriesByTab, replaceWindowWithSegment, setArchivedView } from '../signals/jsonl'
import { setSegmentIndex, dropSegmentIndex, type SegmentIndex } from '../signals/all-segments'
import { setReplayProgress } from '../signals/replay-progress'
import { touchTab, sweepTabMemory, forgetTabRecency } from '../signals/jsonl-eviction'
import { projectPlanTodoEntries } from '../signals/jsonl-project'
import { recordToolUsage, resetToolUsage } from '../signals/tool-usage'
import {
  applyConnectionEvent,
  ConnectionSnapshotRequestGate,
  forgetTabConnection,
  mergeReconnectTabSnapshot,
  reconcilePromptReadiness,
  reconcileCreatedTab,
  reconcileTabSnapshot,
} from '../signals/tab-connection-reconcile'

/**
 * Sets up ALL bridge.onXxx event listeners and their cleanup.
 * Also fires one-shot queries (isVscodeAvailable, getMcpStatus).
 *
 * This hook is purely side-effectful and returns nothing.
 */
/** Which panel this listener runs in. Only the CHAT panel renders the jsonl
 *  viewer and needs the full store; the tools panels derive plan/todos from a
 *  tiny projected slice; the customize panels need none of the session stream.
 *  Gating here is what stopped every panel from holding its own ~1GB copy of
 *  the conversation (the OOM/freeze root — see jsonl-project.ts). */
export type WebviewRole = 'chat' | 'tools' | 'customize'

/** Per-agent cap on a subagent tile's retained transcript. The tile shows recent
 *  activity only; the full record lives in the JSONL viewer. The push was
 *  unbounded AND never pruned (subagentTileState is keyed by agent, not tab), so
 *  a long team session walked the shared WebView2 heap to OOM — three panels at
 *  ~480MB each of accumulated subagent entries in the freeze report. */
const SUBAGENT_TILE_MAX_ENTRIES = 600

// МОДУЛЬНЫЙ уровень, не тело effect: ре-подписка на реконнекте (deps
// reconnectNonce) пересоздавала эти структуры — hook-driven таб снова
// отдавался OSC-эвристике, и Stop мог флипнуться в Send посреди хода сразу
// после реконнекта (аудит #70 C7). Extension-сторона свой Set переживает
// реконнект — теперь симметрично.
const hookDrivenTabs = new Set<string>()
const stuckIdleTimers = new Map<string, ReturnType<typeof setTimeout>>()
// Глобальный дебаунс реконнект-реплея (см. C4 ниже): нонс бампается по разу
// на каждое переподключившееся соединение, а реплей нужен один.
let lastReconnectReplayAt = 0
const RECONNECT_REPLAY_DEBOUNCE_MS = 2000
function cancelStuckIdle(tabId: string): void {
  const t = stuckIdleTimers.get(tabId)
  if (t) { clearTimeout(t); stuckIdleTimers.delete(tabId) }
}

export function useBridgeListeners(
  bridge: KaminBridgeApi,
  promptDebounceTimers: { current: Map<string, ReturnType<typeof setTimeout>> },
  vscodeAvailable: { value: boolean },
  role: WebviewRole = 'chat',
  // Whether THIS panel is a consumer of agent-tile / agent-tree data. The live
  // renderer is the Agents tool section (AgentTilesRow); the chat panel keeps it
  // too for its sidebar agent tree (and the legacy full-app tile row, if revived)
  // — but bounded. console, plan, todos and customize consume NONE of it, so they
  // must never accumulate subagent transcripts or the agent tree: that was ~2/3
  // of the OOM (console + plan each held a full, growing copy they never showed).
  // Defaults to the chat panel.
  agentData: boolean = role === 'chat',
): void {
  const reconnectSnapshotRequests = useRef(new ConnectionSnapshotRequestGate())

  useEffect(() => {
    let reconnectSnapshotRequest: number | undefined
    // hookDrivenTabs / stuckIdleTimers живут на уровне модуля (см. выше):
    // Set табов с детерминированными lifecycle-хуками CLI (OSC-эвристика для
    // них лишь косметика) и дебаунс-страховка от потерянного Stop-хука.

    // VS Code availability
    bridge.isVscodeAvailable().then(available => {
      vscodeAvailable.value = available
    })

    // Seed the composer's "attach file" context with the host editor's CURRENT
    // selection (live updates arrive via onEditorSelection below; this covers
    // the window before the first change fires + after a reconnect-reload).
    bridge.getEditorSelection().then((sel) => { hostEditorSelection.value = sel })

    // Tab lifecycle
    const unsubTabCreated = bridge.onTabCreated((tab: TabInfo) => {
      if (!tabs.value.find(t => t.id === tab.id)) {
        const decorated = { ...reconcileCreatedTab(tab), pinned: pinnedTabs.value.has(tab.id) }
        tabs.value = [...tabs.value, decorated]
        scheduleSaveTabsState()
      }
      // If this tab resumes a pinned conversation that had gone cold, reclaim
      // the ghost entry: carry pin + top-strip slot onto the new tab id.
      reclaimGhostIfMatch(tab)
      // NOT set here: createTab emits a sequenced `tab:switched` right after
      // `tab:created`, and that is the single authority for activeTabId. Setting
      // it here too — unsequenced — let a late `tab:created` for a session the
      // user already left clobber a newer switch (the overtake bug's second
      // vector). The switched handler below owns it.
      // The jump-out-of-Customize on tab creation moved to the switched handler:
      // warm-pool BACKGROUND resumes also emit `tab:created`, and yanking the
      // user out of Customize for a tab they never asked to see was wrong.
    })

    const unsubTabClosed = bridge.onTabClosed((tabId: string) => {
      forgetTabConnection(tabId)
      const closedTab = tabs.value.find(t => t.id === tabId)
      const remaining = tabs.value.filter(t => t.id !== tabId)
      tabs.value = remaining
      // Release the closed session's memory — this was the cumulative leak that
      // walked the shared WebView2 renderer toward OOM: closing a huge tab kept
      // its full ~100MB entries graph (+ window.__jsonlEntries ref + agent tree)
      // reachable forever. Only the SESSION that was closed is dropped.
      clearJsonlEntries(tabId)
      clearAgentTabState(tabId)
      forgetTabRecency(tabId)
      resetToolUsage(tabId)
      // Виджеты (permission/elicitation/askUser) закрытого таба — сироты:
      // ответить по ним уже некому, а они оставались на экране (аудит #70 C11).
      if (activeWidgets.value.some(w => w.data?.tabId === tabId)) {
        activeWidgets.value = activeWidgets.value.filter(w => w.data?.tabId !== tabId)
      }
      // If a pinned tab was closed, keep the pin alive as a ghost so the chip
      // stays in the top strip and the user can click to resume the session.
      if (closedTab && pinnedTabs.value.has(tabId)) {
        ghostifyClosedPinned(closedTab)
        const next = new Set(pinnedTabs.value)
        next.delete(tabId)
        pinnedTabs.value = next
      }
      scheduleSaveTabsState()
      if (sessionTree.value) {
        sessionTree.value = sessionTree.value.filter(n => n.id !== tabId)
      }
      // If closed tab was active, switch to another or show welcome
      if (activeTabId.value === tabId) {
        if (remaining.length > 0) {
          switchTabLocal(remaining[remaining.length - 1].id)
        } else {
          switchTabLocal(null)
          sidebarMode.value = 'sessions'
        }
      }
    })

    const unsubTabSwitched = bridge.onTabSwitched((tabId: string, seq?: number) => {
      // Reject a switch the user has already moved past (see applyTabSwitch).
      // A stale one must not touch recency or trigger an eviction sweep either.
      if (!applyTabSwitch(tabId, typeof seq === 'number' ? seq : Number.MAX_SAFE_INTEGER)) return
      // A tab becoming ACTIVE while the user sits in Customize pulls them back
      // to the chat — covers renderer paths and OS-level entry points (Explorer
      // "Open Claude Bridge" calls createTab in the main process directly).
      // Lives here, not on `tab:created`: background warm-pool resumes create
      // tabs without switching, and those must not yank the user anywhere.
      if (sidebarMode.value === 'customize') {
        sidebarMode.value = 'sessions'
        activeCustomizePanel.value = null
      }
      // Switching is when retained transcripts can be released: the tab we just
      // left is now a background tab, and the one we arrived at refills itself
      // if it was evicted earlier.
      touchTab(tabId)
      const evicted = sweepTabMemory(tabId)
      if (evicted.length > 0) console.info('[bridge] released background transcripts', evicted)
    })

    const unsubTabListChanged = bridge.onTabListChanged((newTabs: TabInfo[]) => {
      const pinSet = pinnedTabs.value
      const reconciled = reconcileTabSnapshot(newTabs)
      tabs.value = reconciled.map(t => ({ ...t, pinned: pinSet.has(t.id) }))
      // Sync model/effort for active tab
      const tid = activeTabId.value
      if (tid) {
        const active = newTabs.find(t => t.id === tid)
        if (active?.model) {
          // Signals for model/effort are managed in InputControls dropdowns
        }
      }
    })

    // Connection state
    const unsubConnectionState = bridge.onConnectionStateChange((tabId: string, state) => {
      // Писать tabs ТОЛЬКО при фактическом изменении: безусловная замена
      // массива ререндерила всех подписчиков широкого сигнала (все баблы
      // ленты) на каждый флап соединения (аудит #70 D3).
      const applied = applyConnectionEvent(tabs.value, tabId, state)
      tabs.value = applied.tabs
      // A stale state must not execute the promptReady side effect after its
      // tab update was rejected by revision/authority ordering.
      if (!applied.accepted) return

      // Once the tab reaches "connected", assume the CLI prompt is ready by
      // default. Otherwise we sit in isBusy until the ❯ glyph is spotted or
      // an OSC-based idle signal arrives, which can take ~30s on cold-start
      // and leaves the Stop button stuck in place.
      tabPromptReady.value = reconcilePromptReadiness(tabPromptReady.value, [{ id: tabId, status: state.status }])
    })

    // PTY output — accumulate chunks and flush at most once per animation
    // frame. Collapses bursts of small PTY frames (e.g. every character of
    // token-by-token streaming output) into one xterm.write call, keeping
    // the main thread free for scrolling / selection.
    const unsubOutput = bridge.onOutput((tabId: string, data: string) => {
      const entry = terminalRegistry.get(tabId)
      if (entry) {
        if (entry.needsClear) {
          // A clear resets the buffer; drop anything we queued before it.
          entry.terminal.clear()
          entry.needsClear = false
          entry.pendingOutput.length = 0
        }
        entry.pendingOutput.push(data)
        if (entry.rafHandle === null) {
          entry.rafHandle = requestAnimationFrame(() => {
            // Re-resolve the entry inside rAF — the tab may have been closed
            // between scheduling and this callback firing. Writing to a
            // disposed terminal silently grows its scrollback + leaks memory.
            const live = terminalRegistry.get(tabId)
            if (!live) return
            live.rafHandle = null
            if (live.pendingOutput.length === 0) return
            const joined = live.outputCarry + live.pendingOutput.join('')
            live.pendingOutput.length = 0
            // Strip mouse-tracking DECSET so plain drag SELECTS text in the
            // console instead of feeding mouse events to the CLI (which made
            // the Bridge Console entirely unselectable). Split-sequence-safe:
            // a trailing partial escape is carried to the next flush.
            const [writable, carry] = splitTrailingEscape(joined)
            live.outputCarry = carry
            if (writable) live.terminal.write(stripMouseTracking(writable))
          })
        }
      }
      // Detect CLI prompt readiness (debounced, per-tab lastSendAt).
      // Skip entirely once the tab is hook-driven: the Stop hook owns
      // promptReady, and a prompt glyph appearing inside assistant output would
      // otherwise flip Stop→Send mid-turn (the classic false-positive race).
      if (hookDrivenTabs.has(tabId)) return
      const ANSI_RE = /\x1b\[[0-9;]*[a-zA-Z]|\x1b\][^\x07]*\x07/g
      const stripped = data.replace(ANSI_RE, '')
      if (stripped.includes('\u276F')) {
        const tabLastSend = lastSendAt.value.get(tabId) ?? 0
        if (Date.now() - tabLastSend < 2000) return
        // Clear previous debounce for this tab
        const prev = promptDebounceTimers.current.get(tabId)
        if (prev) clearTimeout(prev)
        promptDebounceTimers.current.set(tabId, setTimeout(() => {
          promptDebounceTimers.current.delete(tabId)
          const next = new Map(tabPromptReady.value)
          next.set(tabId, true)
          tabPromptReady.value = next
          // CLI now owns the queue — it dispatches queued prompts itself when idle.
        }, 500))
      }
    })

    // PTY exit
    const unsubExit = bridge.onExit((tabId: string, code: number) => {
      const entry = terminalRegistry.get(tabId)
      if (entry) {
        entry.terminal.writeln('')
        entry.terminal.writeln(`\x1b[33mSession exited with code ${code}\x1b[0m`)
        entry.terminal.writeln(`\x1b[90mReconnecting...\x1b[0m`)
        entry.needsClear = true
      }
      const next = new Map(tabPromptReady.value)
      next.set(tabId, false)
      tabPromptReady.value = next
    })

    // Session restarted (effort/model change) — reflect new model/effort on
    // the tab so dropdowns show the session's actual settings.
    const unsubSessionRestarted = bridge.onSessionRestarted((tabId: string, data) => {
      const entry = terminalRegistry.get(tabId)
      if (entry) entry.needsClear = true
      if (data && (data.effort || data.model)) {
        const cur = tabs.value.find(t => t.id === tabId)
        const nextEffort = data.effort ?? cur?.effort
        const nextModel = data.model ?? cur?.model
        if (cur && (cur.effort !== nextEffort || cur.model !== nextModel)) {
          tabs.value = tabs.value.map(t => t.id === tabId ? { ...t, effort: nextEffort, model: nextModel } : t)
        }
      }
    })

    // Manual Reconnect — the server ended the old session and `--resume`s a
    // fresh PTY, so the console must drop the dead session's screen (deferred
    // wipe on the resumed session's first output + snapshot drop). Broadcast to
    // every iframe: the console terminal lives in the Console tool's iframe, a
    // DIFFERENT JS context from the chat where the Reconnect button lives, so an
    // in-webview call would never reach it (see resetConsoleForReconnect).
    const unsubConsoleReset = bridge.onConsoleReset((tabId: string) => {
      resetConsoleForReconnect(tabId)
    })

    // JSONL events — update global store + agent tree. Store growth is gated by
    // role: chat keeps everything (it renders the viewer); tools keeps only the
    // plan/todo-relevant slice (activePlan/activeTodos read nothing else); the
    // agent tree is built from the batch, not the store, so it runs regardless;
    // customize needs none of this. Without the gate every panel accumulated the
    // full ~1GB store — three panels at 1.17GB each in the freeze report.
    const unsubJsonlEntries = role === 'customize' ? (() => {}) : bridge.onJsonlEntries((tabId: string, entries: any[]) => {
      const toStore = role === 'tools' ? projectPlanTodoEntries(entries as any) : (entries as any)
      appendJsonlEntries(tabId, toStore)
      // Счётчики панели Tools: из ПОЛНОГО батча (не среза), записей не хранит —
      // только имя→count с дедупом по id tool_use.
      recordToolUsage(tabId, entries)
      // Keeps a background tab that's mid-response at the top of the recency
      // order, so memory eviction takes the genuinely idle ones first.
      touchTab(tabId)
      // Agent-tree parsing only where the tree/tiles actually render (chat +
      // Agents section). On console/plan/todos it built a tree nobody shows —
      // pure waste, and it walked `entries` (the FULL batch, not the slim slice).
      if (agentData) {
        const agentChanged = parseAgentEntries(tabId, entries)
        if (agentChanged && sessionTree.value) {
          sessionTree.value = mergeAgentTree(sessionTree.value)
          scheduleCleanup(tabId)
        }
      }
    })

    // Live streaming only feeds the chat viewer's in-flight bubble — it writes
    // into the jsonl store via applyStreaming*. The tools/customize panels don't
    // render it and plan/todos read only finalized TaskCreate/TodoWrite, so gate
    // it to chat and keep their stores from filling with streaming deltas.
    const streamingOn = role === 'chat'
    // MITM proxy live streaming entry — replace-by-message-id, no batching.
    const unsubStreamingEntry = streamingOn ? bridge.onStreamingEntry?.((tabId: string, entry: any) => {
      applyStreamingEntry(tabId, entry)
    }) : undefined

    // Incremental text/thinking append between boundary snapshots (protocol>=1).
    const unsubStreamingDelta = streamingOn ? bridge.onStreamingDelta?.((tabId: string, d: any) => {
      applyStreamingDelta(tabId, d)
    }) : undefined

    // Rate-limit / overload from the proxy — the CLI retries silently, so
    // surface it as a transient toast instead of letting the chat just stall.
    const streamStatusAt = new Map<number, number>()
    const unsubStreamingStatus = streamingOn ? bridge.onStreamingStatus?.((_tabId: string, info) => {
      // Debounce: a burst of retries on the same code shouldn't spam toasts.
      const now = Date.now()
      if (now - (streamStatusAt.get(info.code) ?? 0) < 8000) return
      streamStatusAt.set(info.code, now)
      const secs = info.retryAfterMs ? Math.ceil(info.retryAfterMs / 1000) : null
      showToast({
        type: 'info',
        title: info.code === 429 ? 'Rate limited' : `Anthropic busy (${info.code})`,
        message: `Claude is retrying${secs ? ` in ~${secs}s` : ''}…`,
        duration: 6000,
      })
    }) : undefined

    const unsubJsonlStatus = bridge.onJsonlStatus((tabId: string, status: any) => {
      if (status.status === 'watching' && !status.replayComplete) {
        tabJsonlLive.value = new Set([...tabJsonlLive.value].filter(id => id !== tabId))
        const next = new Map(tabAgentTrees.value)
        next.delete(tabId)
        tabAgentTrees.value = next
        // Реплей начнётся с нуля — счётчики тулов тоже (дедуп по id их
        // восстановит без задвоения).
        resetToolUsage(tabId)
      }
      if (status.replayComplete && !tabJsonlLive.value.has(tabId)) {
        // Clear the download bar for THIS tab regardless of whether it's active —
        // JsonlViewer only clears it for the active one, so a background tab that
        // finished replaying while the user was elsewhere would keep a stale
        // progress value forever, permanently wedging its input (isLoading).
        setReplayProgress(tabId, null)
        // The mirror is fully written now — pull the complete compact-segment
        // index so the segment strip can show every compact, not only the ones
        // that fit the resident window.
        if (role !== 'customize') bridge.requestBoundaries?.(tabId)
        setTimeout(() => {
          if (tabJsonlLive.value.has(tabId)) return
          const nextLive = new Set(tabJsonlLive.value)
          nextLive.add(tabId)
          tabJsonlLive.value = nextLive
          const changed = markAllAgentsExited(tabId)
          if (changed && sessionTree.value) {
            sessionTree.value = mergeAgentTree(sessionTree.value)
          }
        }, 50)
      }
      if (status.compacted) {
        // A compaction switched the CLI to a new transcript file; the entries we
        // hold are from the old file (different `_ord` space, so they'd coexist
        // with the new ones instead of being replaced) — drop them and let the
        // server's post-switch replay repaint the new file.
        clearJsonlEntries(tabId)
        // The transcript file switched — the old boundary index + any archived
        // view are stale; drop them (a fresh index arrives at the next replay).
        setArchivedView(tabId, null)
        dropSegmentIndex(tabId)
        const nextLive = new Set(tabJsonlLive.value)
        nextLive.delete(tabId)
        tabJsonlLive.value = nextLive
        // …but that server replay (`readInitialContent`) aborts at its next
        // `await` if `replayGen` bumps — a concurrent reconnect `replayAll()` or
        // the NEXT compaction does exactly that. On a heavily-compacting session
        // (50+ compactions + socket flaps) the replay can die mid-flight, leaving
        // the chat PERMANENTLY EMPTY ("worked for a while, then everything
        // vanished"). Watchdog: if entries haven't repopulated shortly, actively
        // re-request the replay, targeting this tab (compactions fire on the
        // session in use, and an untargeted replay re-pushes every open one).
        if (tabId === activeTabId.value) {
          setTimeout(() => {
            if (tabId !== activeTabId.value) return
            const cur = jsonlEntriesByTab.value.get(tabId)
            if (cur && cur.length > 0) return // replay landed — nothing to do
            if (tabJsonlLive.value.has(tabId)) return // replay completed empty-but-live (genuinely empty)
            try { bridge.requestJsonlReplay(tabId) } catch (err) {
              console.warn('[bridge] post-compaction replay watchdog failed', err)
            }
          }, 2500)
        }
      }
    })

    // Full compact-segment index (all segments incl. out-of-window + record
    // counts) from the mirror — the strip renders it so every compact is visible.
    const unsubBoundaries = role === 'customize' ? (() => {}) : bridge.onBoundaries?.((tabId: string, p: SegmentIndex) => {
      setSegmentIndex(tabId, { boundaries: p.boundaries ?? [], counts: p.counts ?? [] })
    }) ?? (() => {})

    // One archived compact segment loaded from the mirror → REPLACE the window
    // with it. The pill click already set the archived-view gate + ts pin, so
    // live entries are held off and the view resolves to this segment.
    const unsubSegmentPage = role === 'customize' ? (() => {}) : bridge.onSegmentPage?.((tabId: string, p: { records: any[] }) => {
      // An empty segment (all-system rows, or ts-less) must NOT leave the tab
      // stuck in archived mode with live updates gated off and the old window
      // still showing — revert to Current instead of silently no-op'ing.
      if (!p?.records || p.records.length === 0) {
        setArchivedView(tabId, null)
        return
      }
      replaceWindowWithSegment(tabId, p.records)
    }) ?? (() => {})

    // Subagent transcripts feed the agent tiles, which render ONLY in the chat
    // panel (AppMainContent) and the Agents tool section. console/plan/todos and
    // customize show no agent UI — gate on `agentData` so they never accumulate
    // this (the OOM: they each held a full, growing copy they never displayed).
    const unsubSubagentEntries = !agentData ? (() => {}) : bridge.onJsonlSubagentEntries((tabId: string, agentName: string, entries: any[], agentId?: string) => {
      const tid = activeTabId.value
      if (tabId !== tid) return
      const routingKey = agentId || agentName
      const curMap = subagentTileState.value
      let state = curMap.get(routingKey) ?? curMap.get(agentName)
      // uuid-дедуп: серверный реплей субагентских файлов повторяется на каждый
      // реаттач/resync — без фильтра транскрипт агента множился кратно.
      let fresh: any[] = entries
      if (state) {
        const seen = state.seen ?? (state.seen = new Set<string>())
        fresh = entries.filter((e) => { const u = (e as { uuid?: string }).uuid; if (!u) return true; if (seen.has(u)) return false; seen.add(u); return true })
      }
      if (!state) {
        // ПЕРВЫЙ батч кладём сразу: state создавался пустым и пришедшие записи
        // выбрасывались — реплей завершённого агента приходит ОДНИМ батчем, и
        // его вид оставался «No messages from this agent yet» (прод-скрин).
        const firstSeen = new Set<string>()
        for (const e of entries) { const u = (e as { uuid?: string }).uuid; if (u) firstSeen.add(u) }
        state = { tileKey: '', entries: [...entries], seen: firstSeen }
        const nextMap = new Map(curMap)
        nextMap.set(routingKey, state)
        if (routingKey !== agentName) nextMap.set(agentName, state)
        subagentTileState.value = nextMap
      } else if (fresh.length > 0) {
        state.entries.push(...fresh)
        // Hard per-agent cap — keep the most recent window, drop older overflow.
        // Without this the tile transcript grew without limit for the whole
        // session (never pruned) and was the dominant term in the shared-heap OOM.
        if (state.entries.length > SUBAGENT_TILE_MAX_ENTRIES) {
          state.entries.splice(0, state.entries.length - SUBAGENT_TILE_MAX_ENTRIES)
        }
        subagentTileState.value = new Map(subagentTileState.value)
      }
      // Пульс жизни для staleness-прунера: рабочий агент пишет в свой JSONL,
      // даже если не шлёт teammate-message — без touch его running ложно
      // протухал бы через STALE_RUNNING_MS.
      touchAgentAlive(tabId, agentName)
    })

    // Widget events
    const unsubElicitation = bridge.onElicitationRequest((tabId: string, req: ElicitationRequest) => {
      // Dedup ONLY the exact same request (a resend of the same requestId). Do
      // NOT evict other pending questions for the tab — a genuine second question
      // arriving while the first is unanswered used to wipe the first, so it
      // "never appeared". Keeping both is the safe tradeoff (worst case: a retry
      // shows twice; the user answers the live one).
      const filtered = activeWidgets.value.filter(w => w.requestId !== req.requestId)
      activeWidgets.value = [...filtered, {
        requestId: req.requestId,
        resolved: false,
        type: 'elicitation',
        data: { tabId, request: req },
      }]
    })

    const unsubPermission = bridge.onPermissionRequest((tabId: string, req: PermissionRequest) => {
      const filtered = activeWidgets.value.filter(w => w.requestId !== req.requestId)
      activeWidgets.value = [...filtered, {
        requestId: req.requestId,
        resolved: false,
        type: 'permission',
        data: { tabId, request: req },
      }]
    })

    const unsubAskUser = bridge.onAskUserQuestion((tabId: string, data) => {
      const filtered = activeWidgets.value.filter(w =>
        w.requestId !== data.requestId &&
        !((w.type === 'elicitation' || w.type === 'askUser') && w.data?.tabId === tabId)
      )
      activeWidgets.value = [...filtered, {
        requestId: data.requestId,
        resolved: false,
        type: 'askUser',
        data: { tabId, data },
      }]
    })

    // Session tree
    const unsubTreeUpdate = bridge.onTreeUpdate((tree: TreeNode[]) => {
      sessionTree.value = tree
    })

    const unsubSessionTitle = bridge.onSessionTitle((tabId: string, title: string) => {
      // Только при фактической смене (см. D3 выше): title ретранслируется
      // и без изменений.
      const cur = tabs.value.find(t => t.id === tabId)
      if (!cur || cur.sessionTitle === title) return
      tabs.value = tabs.value.map(t =>
        t.id === tabId ? { ...t, sessionTitle: title } : t
      )
    })

    const unsubSessionActivity = bridge.onSessionActivity((tabId: string, data) => {
      const d = data as {
        rawTitle?: string; isWorking?: boolean
        hookDriven?: boolean; promptReady?: boolean; waiting?: boolean
      }

      // Deterministic path — the CLI's own lifecycle hooks (UserPromptSubmit /
      // Stop / SessionStart / Notification). These beat the OSC-spinner + prompt-
      // glyph heuristics: set exactly the fields the event carried and mark the
      // tab hook-driven so the flappy fallback paths below stop fighting it.
      if (d.hookDriven) {
        hookDrivenTabs.add(tabId)
        // A real working assertion voids any pending stuck-idle clear.
        if (d.isWorking === true) cancelStuckIdle(tabId)
        if (typeof d.isWorking === 'boolean' || typeof d.rawTitle === 'string') {
          const prev = tabActivity.value.get(tabId) ?? { rawTitle: '', isWorking: false }
          const na = new Map(tabActivity.value)
          na.set(tabId, {
            rawTitle: typeof d.rawTitle === 'string' ? d.rawTitle : prev.rawTitle,
            isWorking: typeof d.isWorking === 'boolean' ? d.isWorking : prev.isWorking,
          })
          tabActivity.value = na
        }
        if (typeof d.promptReady === 'boolean') {
          if (d.promptReady) markOpen(tabId, 'prompt-ready')
          const nr = new Map(tabPromptReady.value); nr.set(tabId, d.promptReady); tabPromptReady.value = nr
        }
        if (typeof d.waiting === 'boolean') {
          const nw = new Map(tabWaiting.value); nw.set(tabId, d.waiting); tabWaiting.value = nw
        }
        return
      }

      // OSC fallback — once a tab is hook-driven, only refresh the cosmetic
      // spinner text; never let flappy OSC working/promptReady override the hook.
      if (hookDrivenTabs.has(tabId)) {
        const prev = tabActivity.value.get(tabId)
        if (prev && typeof d.rawTitle === 'string' && d.rawTitle !== prev.rawTitle) {
          const na = new Map(tabActivity.value)
          na.set(tabId, { ...prev, rawTitle: d.rawTitle })
          tabActivity.value = na
        }
        // Stuck-working recovery (see stuckIdleTimers). OSC only ever reports
        // isWorking:false on a genuine idle glyph (✳/›), never mid-stream, so a
        // false while the hook state is still "working" means the Stop hook was
        // lost. Confirm the idle holds ~1.5s, then clear — a live spinner glyph
        // (isWorking:true) in that window cancels it.
        if (d.isWorking === true) cancelStuckIdle(tabId)
        else if (d.isWorking === false && prev?.isWorking && !stuckIdleTimers.has(tabId)) {
          const tabLastSend = lastSendAt.value.get(tabId) ?? 0
          if (Date.now() - tabLastSend < 500) return  // fresh-send echo, not idle
          stuckIdleTimers.set(tabId, setTimeout(() => {
            stuckIdleTimers.delete(tabId)
            const cur = tabActivity.value.get(tabId)
            if (!cur?.isWorking) return
            const na = new Map(tabActivity.value)
            na.set(tabId, { ...cur, isWorking: false })
            tabActivity.value = na
            const nr = new Map(tabPromptReady.value); nr.set(tabId, true); tabPromptReady.value = nr
          }, 1500))
        }
        return
      }

      const next = new Map(tabActivity.value)
      next.set(tabId, { rawTitle: d.rawTitle ?? '', isWorking: !!d.isWorking })
      tabActivity.value = next

      // CLI went idle (OSC spinner dropped) — promptReady is a more reliable
      // source of truth than the \u276F debounce, which misses on chunk
      // splits and leaves the Stop button stuck.
      if (!d.isWorking) {
        const tabLastSend = lastSendAt.value.get(tabId) ?? 0
        if (Date.now() - tabLastSend < 500) return  // guard against echo of a fresh send
        const prev = promptDebounceTimers.current.get(tabId)
        if (prev) clearTimeout(prev)
        const nextReady = new Map(tabPromptReady.value)
        nextReady.set(tabId, true)
        tabPromptReady.value = nextReady
      }
    })

    const unsubMcpActivity = bridge.onMcpActivity((_tabId: string, call) => {
      console.log('[MCP]', _tabId, call.status, call.toolName, call)
    })

    // MCP servers
    const unsubMcpServersChanged = bridge.onMcpServersChanged((servers) => {
      mcpServers.value = servers
    })

    const unsubMcpLoading = bridge.onMcpLoading(() => {
      mcpLoading.value = true
      markOpen(activeTabId.value, 'mcp-loading')
    })

    const unsubMcpReady = bridge.onMcpReady(() => {
      mcpLoading.value = false
      markOpen(activeTabId.value, 'mcp-ready')
    })

    // Fallback: query MCP status in case mcp:ready fired before renderer loaded.
    // The channel is not implemented on the kamin host (resolves null) — treat
    // anything but an explicit `false` as ready, otherwise `!null` locked the
    // input bar in "queue" mode forever.
    bridge.getMcpStatus().then((ready: boolean | null) => {
      mcpLoading.value = ready === false
    }).catch(() => { mcpLoading.value = false })

    // Config ready — no-op. First-run UX is handled by NoTokenGateModal, not
    // by auto-navigation. Jumping into Settings on every config-ready signal
    // kicked the user out of the chat view.
    const unsubConfigReady = bridge.onConfigReady((_hasConfig: boolean) => { /* intentionally blank */ })

    // CWD changed -- main process handles new tab creation
    const unsubCwdChanged = bridge.onCwdChanged((_cwd: string) => {
      // noop
    })

    // Mini-window toast route: user clicked "Open" → main focused this window
    // and sent us the target tab. Activate it so the user lands on the right
    // widget/terminal without hunting for it.
    const unsubToastRoute = bridge.onToastRoute(({ tabId }) => {
      if (!tabId) return
      if (tabs.value.find(t => t.id === tabId)) {
        switchTabLocal(tabId)
        bridge.switchTab(tabId)
      }
    })

    // KaminIDE's host Monaco editor active file + selection → drives the
    // composer's "attach file" toggle (editor-context appended on send).
    const unsubEditorSel = bridge.onEditorSelection((sel) => { hostEditorSelection.value = sel })

    // Клик в панели Tools (другой iframe) → чат раскрывает применения тула.
    // toolusage-open теперь потребляет сама панель Tools (инлайн-просмотр);
    // чату подписка не нужна.

    // On a mid-session reconnect (reconnectNonce bumped by bridge-shim) this
    // effect re-ran and re-subscribed all handlers above with a fresh transport
    // registration. Re-pull the backlog so any entries that arrived while the
    // socket was down are recovered (mount-time replay is handled by ChatRoot/
    // useInit — this covers only the reconnect case). peek() so we don't add a
    // spurious extra dependency.
    // АДРЕСНО (активный таб) + глобальный дебаунс: реплей БЕЗ tabId гнал ВСЕ
    // табы во все панели, а рестарт сервера с тёплым пулом бампал нонс по
    // разу на соединение — до 5 циклов × панели × табы × тысячи записей,
    // мини-«reload storm» (аудит #70 C4). Пропущенные фоновые табы догонят
    // при переключении (lazy attach + watchdog).
    if (reconnectNonce.peek() > 0 && role !== 'customize') {
      const now = Date.now()
      if (now - lastReconnectReplayAt > RECONNECT_REPLAY_DEBOUNCE_MS) {
        lastReconnectReplayAt = now
        try {
          const active = activeTabId.peek()
          if (active) bridge.requestJsonlReplay(active)
          else bridge.requestJsonlReplay()
        } catch (err) {
          // Recovery path — surface failures instead of a silent "chat stopped".
          console.warn('[bridge] reconnect replay request failed', err)
        }
      }
    }

    // Re-subscription closes the event gap but cannot replay a state frame that
    // was already missed. Pull one atomic, versioned snapshot after reconnect;
    // reconciliation prevents this async response from rolling newer events
    // back while refreshing every tab, including the active composer state.
    if (reconnectNonce.peek() > 0) {
      reconnectSnapshotRequest = reconnectSnapshotRequests.current.begin()
      const reconnectSnapshotBaseline = new Set(tabs.peek().map(tab => tab.id))
      bridge.listTabs().then((snapshot) => {
        if (!reconnectSnapshotRequests.current.isCurrent(reconnectSnapshotRequest!)) return
        const pinSet = pinnedTabs.peek()
        // Preserve tabs/events created while listTabs was in flight and let the
        // per-tab authority/revision reconciler reject stale connection slices.
        const reconciled = mergeReconnectTabSnapshot(tabs.peek(), snapshot, reconnectSnapshotBaseline)
        tabs.value = reconciled.map(tab => ({ ...tab, pinned: pinSet.has(tab.id) }))
        tabPromptReady.value = reconcilePromptReadiness(tabPromptReady.value, reconciled)
      }).catch((err) => {
        console.warn('[bridge] reconnect state snapshot failed', err)
      })
    }

    return () => {
      if (reconnectSnapshotRequest !== undefined) {
        reconnectSnapshotRequests.current.invalidate(reconnectSnapshotRequest)
      }
      unsubEditorSel()
      unsubTabCreated()
      unsubTabClosed()
      unsubTabSwitched()
      unsubTabListChanged()
      unsubConnectionState()
      unsubOutput()
      unsubExit()
      unsubSessionRestarted()
      unsubConsoleReset()
      unsubJsonlEntries()
      unsubStreamingEntry?.()
      unsubStreamingDelta?.()
      unsubStreamingStatus?.()
      unsubJsonlStatus()
      unsubBoundaries()
      unsubSegmentPage()
      unsubSubagentEntries()
      unsubElicitation()
      unsubPermission()
      unsubAskUser()
      unsubTreeUpdate()
      unsubSessionTitle()
      unsubSessionActivity()
      unsubMcpActivity()
      unsubMcpServersChanged()
      unsubMcpLoading()
      unsubMcpReady()
      unsubConfigReady()
      unsubCwdChanged()
      unsubToastRoute()
      stuckIdleTimers.forEach(clearTimeout)
      stuckIdleTimers.clear()
    }
    // Re-run (tear down + re-subscribe) on a mid-session reconnect so stale subs
    // can't wedge the chat — replaces the old full-iframe location.reload().
  }, [reconnectNonce.value])
}

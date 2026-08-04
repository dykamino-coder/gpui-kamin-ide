import type { JSX } from 'preact'
import { useCallback, useEffect, useRef, useState } from 'preact/hooks'
import { compactSegments, activeSegmentIdx } from '../../signals/compact-segments'
import { activeTabId, tabs } from '../../signals/tabs'
import { jsonlEntriesByTab, getJsonlStructureVersion, tabNeedsRefill, clearRefillDebt, prependJsonlEntries, trimTabToWindow, STORE_WINDOW, STORE_SLACK, SCROLL_UP_MAX } from '../../signals/jsonl'
import { boundTabId } from '../../signals/session-binding'
import { useBridge } from '../../hooks/useBridge'
import { JsonlEntry } from './JsonlEntry'
import type { JsonlEntryData } from '../../types/jsonl'
import { useChatScrollPin } from './useChatScrollPin'
import { getRenderModel } from './derived-cache'
import { isApiErrorEntry } from './prepare-jsonl-entries'
import { shouldShrinkWindow } from './shrink-window'
import { chatAtBottom } from '../../signals/ui'
import { replayProgressByTab, setReplayProgress } from '../../signals/replay-progress'
import { mcpLoading } from '../../signals/connection'
import { markOpen, formatOpenTimeline, lastOpenPhase, stalledForMs, openTimelineTick } from '../../signals/open-timeline'
import { LoadingCue } from '../common/LoadingCue'
import styles from './JsonlViewer.module.css'

// Cap how many (visible) entries actually mount as DOM. A large session has
// thousands of entries; mounting them all in one synchronous render pass froze
// the shared renderer thread for seconds (content-visibility skips paint, not
// the vnode/DOM build). We mount the most-recent `renderCap` and offer a "show
// earlier" affordance — the chat is pinned to the bottom, so the recent turns +
// any live stream are always in the window. Bumped in chunks on demand.
const INITIAL_RENDER_CAP = 150
const RENDER_CAP_STEP = 400
// How many older records to pull from disk per scroll-up page, once the store's
// own window is exhausted (the windowed-store scroll-up — see onNearTop).
const OLDER_PAGE = 400
// STORE_WINDOW / STORE_SLACK / SCROLL_UP_MAX — the resident-store bounds. They
// live in signals/jsonl (the layer that owns the store and now enforces the same
// cap on background tabs the viewer never mounts); imported so the reader-aware
// bottom-trim below and the signal-layer backstop share one policy.
// The cap only ever grew. Reading back through a long session raises it 400 at a
// time and nothing ever lowered it, so after a few minutes of scrolling the chat
// held thousands of mounted rows — with their markdown, highlighting and tool
// blocks — until the tab was switched. Returning to the bottom is the one moment
// where dropping the excess is invisible: the rows being unmounted are far above
// the viewport, and the view is pinned to an end that does not move. Anywhere
// else it would yank the reader, which is exactly the bug the anchor logic in
// useChatScrollPin exists to prevent.
//
// The delay is not cosmetic: `chatAtBottom` flips true the instant a snap lands,
// including the snaps a streaming burst fires, and re-rendering the window on
// each of those would be worse than the leak.
const CAP_SHRINK_DELAY_MS = 3000
// Pure SAFETY fallback: only drop "Loading conversation…" if replay-complete
// never arrives (host stall / a session that didn't re-emit). Real sessions
// clear it the instant their first entry lands (the replay sends current-dialog
// FIRST and is now backpressure-paced so that batch arrives fast), so this timer
// should almost never fire. It was 8s, which on a big session could elapse
// BEFORE the first batch and flash the "empty session" placeholder mid-replay
// (the same premature-drop the console overlay had) — give it real headroom.
const REPLAY_WATCHDOG_MS = 20_000

export function JsonlViewer(): JSX.Element {
  const bridge = useBridge()
  const currentTabId = activeTabId.value
  const [renderCap, setRenderCap] = useState(INITIAL_RENDER_CAP)
  // Per-entry vnode cache keyed by uuid/message.id. The #1 freeze cause on a
  // huge session is that every stream frame reassigns `currentEntries` (a fresh
  // array ref), which re-runs the render below and rebuilds the vnode of EVERY
  // shown entry (markdown + highlight + tool blocks) ~30×/sec — even though only
  // the live streaming tail actually changed. Reusing the SAME vnode object for
  // an unchanged committed entry makes Preact bail out of diffing that subtree,
  // so a stream frame only rebuilds the live tail, not all ~150 shown rows.
  // Invalidated wholesale on tab switch (Preact mutates vnodes on diff, so a
  // vnode mounted then unmounted by a switch can't be safely re-used across the
  // remount); per-entry on a tool-result/interrupt/skip version bump (the only
  // things that change a committed row's output).
  const vnodeCacheRef = useRef(new Map<string, { vnode: JSX.Element; ver: string }>())
  // Tab whose JSONL is still replaying (no entries yet). Lets us show a
  // "Loading conversation…" cue instead of the "empty session" placeholder —
  // a large session's replay otherwise looks identical to a brand-new session.
  const [replayingTab, setReplayingTab] = useState<string | null>(currentTabId)
  // Seconds the load-cover has been up. Drives the phase timeline it shows after
  // a few seconds — and it must keep ticking, since a frozen "stuck for 12s"
  // reads as a frozen app rather than as a live measurement.
  const [coverSecs, setCoverSecs] = useState(0)

  const currentEntries = currentTabId
    ? (jsonlEntriesByTab.value.get(currentTabId) ?? [])
    : []

  // Infinite scroll-up: `hiddenCountRef` (set during render) = how many older
  // entries are windowed off; `loadingMoreRef` is a one-shot lock so a burst of
  // scroll events can't double-grow the window before the render commits.
  // Viewport stability on prepend is left to the browser's native CSS scroll
  // anchoring (overflow-anchor:auto, on by default) — a manual scrollHeight-delta
  // fights it AND mis-measures, because newly-mounted off-screen rows report
  // content-visibility's 80px placeholder height until they scroll into view.
  const hiddenCountRef = useRef(0)
  const loadingMoreRef = useRef(false)
  // Scroll-up beyond the held window: a fetch of the preceding disk page is in
  // flight (olderLoading), or the start of the transcript has been reached
  // (reachedStart) so there is nothing more to ask for. Per active tab; reset on
  // switch below.
  const olderLoadingRef = useRef(false)
  const reachedStartRef = useRef(false)

  const onNearTop = useCallback(() => {
    if (loadingMoreRef.current || olderLoadingRef.current) return
    // First mount more of the rows already HELD (cheap — no fetch).
    if (hiddenCountRef.current > 0) {
      loadingMoreRef.current = true
      setRenderCap((c) => c + RENDER_CAP_STEP)
      return
    }
    // The whole held window is on screen — pull the PRECEDING page from the disk
    // mirror (the windowed store grows upward on demand instead of holding the
    // entire marathon transcript, which is what drove the renderer to OOM).
    if (reachedStartRef.current || !currentTabId) return
    const store = jsonlEntriesByTab.value.get(currentTabId) ?? []
    // Don't grow the store past the scroll-up ceiling (would re-OOM — the trim
    // only fires at the bottom). Stop asking for older pages here.
    if (store.length >= SCROLL_UP_MAX) { reachedStartRef.current = true; return }
    const oldestPos = (store[0] as { _pos?: number } | undefined)?._pos
    if (typeof oldestPos !== 'number' || oldestPos <= 0) { reachedStartRef.current = true; return }
    olderLoadingRef.current = true
    bridge.loadOlderPage(currentTabId, oldestPos, OLDER_PAGE)
  }, [currentTabId, bridge])

  // The older page requested above. Prepend it and grow the DOM window to show
  // it; an empty page means the start of the transcript was reached.
  useEffect(() => {
    const unsub = bridge.onOlderPage((tabId: string, p: { records: any[] }) => {
      olderLoadingRef.current = false
      if (tabId !== activeTabId.value) return
      // If the reader scrolled back to the bottom while this was in flight, don't
      // prepend rows they're no longer looking at (and don't grow the cap the
      // bottom-trim is about to shrink) — the request is stale.
      if (chatAtBottom.peek()) return
      if (!p?.records || p.records.length === 0) { reachedStartRef.current = true; return }
      if (prependJsonlEntries(tabId, p.records)) setRenderCap((c) => c + p.records.length)
    })
    return unsub
  }, [])

  // Cap the resident store. Only at the bottom, so the trimmed (oldest) rows are
  // far above the viewport; slack keeps it from trimming — and re-running the
  // render model — on every single live append. Scroll-up brings the older pages
  // back from disk, so reachedStart is cleared: there is history above again.
  const atBottomForTrim = chatAtBottom.value
  useEffect(() => {
    if (!currentTabId || !atBottomForTrim) return
    if (currentEntries.length > STORE_WINDOW + STORE_SLACK) {
      if (trimTabToWindow(currentTabId, STORE_WINDOW)) reachedStartRef.current = false
    }
  }, [currentTabId, atBottomForTrim, currentEntries.length])

  const { containerRef } = useChatScrollPin(currentTabId ?? undefined, currentEntries, onNearTop)

  // Give the window back once the reader is at the bottom again. See
  // CAP_SHRINK_DELAY_MS for why this is the only safe moment and why it waits.
  const atBottom = chatAtBottom.value
  useEffect(() => {
    if (!shouldShrinkWindow({ atBottom, cap: renderCap, initialCap: INITIAL_RENDER_CAP, loadingMore: loadingMoreRef.current })) return
    const t = setTimeout(() => {
      // Re-check rather than trust the 3s-old snapshot: the reader may have
      // scrolled back up, or a scroll-up load may have started, in the meantime.
      if (!shouldShrinkWindow({ atBottom: chatAtBottom.peek(), cap: renderCap, initialCap: INITIAL_RENDER_CAP, loadingMore: loadingMoreRef.current })) return
      setRenderCap(INITIAL_RENDER_CAP)
    }, CAP_SHRINK_DELAY_MS)
    return () => { clearTimeout(t) }
  }, [atBottom, renderCap])

  // Release the load lock once the enlarged window has rendered.
  useEffect(() => { loadingMoreRef.current = false }, [renderCap])

  useEffect(() => {
    if (!currentTabId) return
    setRenderCap(INITIAL_RENDER_CAP) // fresh window when switching sessions
    vnodeCacheRef.current.clear() // different session → cached vnodes are stale
    loadingMoreRef.current = false // drop any in-flight scroll-up load from the old tab
    olderLoadingRef.current = false // …and any in-flight older-page fetch
    reachedStartRef.current = false // re-evaluate whether older pages exist for THIS tab
    const existing = jsonlEntriesByTab.value.get(currentTabId)
    // A tab we cleared ourselves OWES a refill even if it is no longer empty: a
    // single live entry landing after the clear (a /reload-skills echo will do)
    // used to disqualify it, leaving the session permanently stripped of its
    // history — seen in the wild as 3 entries and an EMPTY drop log, i.e. the
    // replay was never asked for rather than filtered out.
    const owed = tabNeedsRefill(currentTabId)
    const empty = !existing || existing.length === 0
    setReplayingTab(empty ? currentTabId : null)
    // Target THIS tab: an untargeted replay would re-push every other session.
    if (empty || owed) bridge.requestJsonlReplay(currentTabId)
  }, [currentTabId])

  // Publish what this viewer is actually SHOWING. Until it matches the active
  // session the composer must not send: during a big session's replay the screen
  // still holds the previous conversation while `activeTabId` already points at
  // the new one, and a message typed against what is visible would be delivered
  // to a session the user cannot see. Bound once the entries are up, or once the
  // replay reports completion (a genuinely empty session binds too).
  // Tabs whose replay reported completion while they were NOT the active tab.
  // The status handler used to bail on that mismatch and the server never
  // re-sends, so a session that finished a beat before the switch committed
  // could never bind — cover up and Send dead for the rest of the session.
  const replayDoneTabs = useRef<Set<string>>(new Set())

  useEffect(() => {
    if (!currentTabId) { boundTabId.value = null; return }
    markOpen(currentTabId, 'activate')
    // Bind ONLY on real on-screen content. The old `replayingTab !== currentTabId`
    // clause was TRUE in the stale window right after a switch — the new tab's
    // `setReplayingTab` hasn't committed yet — so the viewer reported it was
    // "showing" the new session while the OLD transcript was still painted,
    // lifting the host switch-cover for a beat (the "previous chat flashes for a
    // couple seconds" report). The empty-but-complete case binds from the
    // replay-complete handler below instead, so a genuinely empty session still
    // clears the cover.
    if (currentEntries.length > 0) {
      markOpen(currentTabId, 'first-entries')
      boundTabId.value = currentTabId
      markOpen(currentTabId, 'bound')
      return
    }
    // …or the replay already finished while this tab was in the background (see
    // `replayDoneTabs`): the completion will never be re-sent, so bind on entry.
    if (replayDoneTabs.current.has(currentTabId)) {
      boundTabId.value = currentTabId
      markOpen(currentTabId, 'bound')
    }
  }, [currentTabId, currentEntries.length])

  // Tell the host the same thing. It repaints its session list and titlebar the
  // instant a session is clicked, while this iframe still holds the previous
  // transcript until the replay lands — and until now nothing carried that fact
  // out of the frame, so no host surface could show the gap or notice a
  // divergence that never closed. Only the chat reports; the tools panels do not
  // display a transcript and would just contradict it.
  const bound = boundTabId.value
  useEffect(() => {
    if (!bound) return
    try { bridge.reportBoundTab?.(bound) } catch { /* older host without the channel */ }
  }, [bound])

  // jsonl:status replay-complete → trigger a few extra rAFs of snap. The hook
  // owns the scroll behaviour but doesn't subscribe to bridge events; we keep
  // that subscription here so renderer-side concerns (replay) stay co-located.
  useEffect(() => {
    const unsub = bridge.onJsonlStatus((tabId: string, status: any) => {
      // Replay byte-progress → percent. Kept per tab (not gated on the active
      // one) so a background restore's bar is right the instant the user opens
      // it. Server floors it; the display caps at 99 until replayComplete so the
      // bar never sits at a full 100 while history is still arriving.
      markOpen(tabId, 'replay-status')
      const p = status?.replayProgress
      if (p && typeof p.sent === 'number' && p.total > 0) {
        markOpen(tabId, 'replay-progress')
        setReplayProgress(tabId, Math.min(99, Math.floor((p.sent / p.total) * 100)))
      }
      if (!status?.replayComplete) return
      markOpen(tabId, 'replay-complete')
      // Completion is sent ONCE. The old `tabId !== activeTabId.value` early
      // return above dropped it whenever it landed before the switch committed,
      // and nothing ever re-sent it — so record it per tab and let the bind
      // effect pick it up when the tab does become current.
      replayDoneTabs.current.add(tabId)
      // Replay finished — drop the loading cue (an empty result now correctly
      // shows the "empty session" placeholder, not a spinner).
      setReplayingTab((cur) => (cur === tabId ? null : cur))
      setReplayProgress(tabId, null) // history fully arrived — retire the bar
      clearRefillDebt(tabId) // topped up (or proven genuinely empty)
      if (tabId !== activeTabId.value) return
      // This tab is now genuinely on screen (even if empty), so bind it: a
      // zero-entry session never trips the entries>0 bind above, and without
      // this the host switch-cover would hold until its 2.5s timeout.
      boundTabId.value = tabId
      markOpen(tabId, 'bound')
      // Respect a reader who scrolled up mid-load — replay finishing is not a
      // reason to yank them back to the end.
      ;(window as any).__scrollChatToBottom?.({ onlyIfPinned: true })
    })
    return unsub
  }, [])

  // Watchdog: never leave "Loading conversation…" spinning forever if a
  // replay-complete status never arrives (host stall / empty session that
  // didn't re-emit). After a grace period, fall back to the empty placeholder.
  useEffect(() => {
    if (replayingTab !== currentTabId || currentEntries.length > 0) return
    const t = window.setTimeout(() => {
      markOpen(currentTabId, 'watchdog-giveup')
      setReplayingTab((cur) => (cur === currentTabId ? null : cur))
      if (currentTabId) {
        setReplayProgress(currentTabId, null) // a stalled replay must not leave the bar frozen
        // Giving up on the replay means this tab IS what the viewer is showing,
        // so bind it. Clearing only the cover left `sessionBindingPending` true
        // forever — the composer accepted text but Send stayed dead with nothing
        // on screen to explain why (the reported "can type, cannot send").
        boundTabId.value = currentTabId
        markOpen(currentTabId, 'bound')
      }
    }, REPLAY_WATCHDOG_MS)
    return () => window.clearTimeout(t)
  }, [replayingTab, currentTabId, currentEntries.length])

  // Expose entries for input history (lazy seed)
  if (currentTabId && currentEntries.length > 0) {
    ;(window as any).__jsonlEntries = (window as any).__jsonlEntries || {}
    ;(window as any).__jsonlEntries[currentTabId] = currentEntries
  }

  // Compact-segment tab state lives in signals/compact-segments.ts so the
  // tab strip can render above ChatHeader (outside the scrolling .chat-panel)
  // instead of inside this container.
  const segs = compactSegments.value
  const segIdx = activeSegmentIdx.value
  // NO auto-jump effect here any more. It pinned the "latest" index into a map
  // AFTER the commit, so the render that had just used the stale index painted
  // the wrong conversation for a frame — and during reverse-load (older segments
  // stream in behind the current one) that fired on every batch, which is the
  // "chat jumps back to the previous conversation while loading" the user saw.
  // `activeSegmentIdx` now defaults to the latest DURING render, so the end of
  // the chat is correct on the first paint and history landing behind it can't
  // move the view.

  // The viewer only ever RENDERS the active compact segment (older segments are
  // behind the tab strip). The heavy pipeline — merge/group over the segment
  // slice, the whole-history tool-result map, the chain walk, AND the whole
  // visibility pass (uuids-in-range, interrupt/skip flags, chain-membership
  // filter) — is built and cached by `getRenderModel`, keyed on a per-tab
  // STRUCTURE version. While structure is unchanged (a 30×/s streaming text
  // delta), the cached model is reused and only the live stub objects are
  // re-pointed (O(tail)); a real shape change (batch append) recomputes. This
  // is what stops a huge session freezing mid-answer — the recompute no longer
  // runs over the whole segment on every stream frame.
  const structureVersion = currentTabId ? getJsonlStructureVersion(currentTabId) : 0
  const { toolResults, visibleMerged, ver } = getRenderModel(
    currentTabId ?? '', currentEntries, segs, segIdx, structureVersion,
  )

  const replayPct = currentTabId ? replayProgressByTab.value.get(currentTabId) : undefined
  // ONE loading rule: the cover shows only when there is NOTHING to paint.
  // Content — the retained per-session store or the disk mirror's tail — always
  // wins; the network replay then tops it up silently (the store dedups on
  // uuid/_ord), so an already-painted conversation is never hidden behind a
  // loader again. The old behaviour covered the whole panel for the entire
  // replay to hide a mis-ordered mirror tail; readTail is timestamp-ordered now,
  // so the tail it paints IS the real end of the chat.
  const mcpUp = mcpLoading.value
  const loadingUnderway = replayPct !== undefined || replayingTab === currentTabId
  const showCover = loadingUnderway && currentEntries.length === 0
  // History still streaming in behind visible content — show only a thin
  // unobtrusive progress strip (rendered outside the scroller below).
  const silentTopUpPct = currentEntries.length > 0 ? replayPct : undefined
  // Live diagnostics for a stuck cover — readable from outside via the shell's
  // wvjs probe, because this webview has no devtools in the CEF port.
  ;(window as any).__coverDiag = {
    showCover, replayingTab, currentTabId, replayPct,
    entries: currentEntries.length,
    // peek(): диагностике подписка не нужна — .value ререндерил самый
    // тяжёлый компонент на каждую запись tabs (реконнект-флап, смена title).
    tabs: tabs.peek().map(t => t.id.slice(0, 8)),
  }

  useEffect(() => {
    if (!showCover) { setCoverSecs(0); return }
    const id = window.setInterval(() => setCoverSecs((n) => n + 1), 1000)
    return () => window.clearInterval(id)
  }, [showCover, currentTabId])

  // Read the tick so a new mark re-renders the timeline below.
  openTimelineTick.value
  const openLine = formatOpenTimeline(currentTabId)
  const waitingOn = lastOpenPhase(currentTabId)
  const stalledSecs = Math.round(stalledForMs(currentTabId) / 1000)

  return (
    <>
      {silentTopUpPct !== undefined && (
        // Silent top-up: history is still arriving behind content that is
        // already on screen. Anchored to .chat-panel (position:relative), not
        // the scroller, so it doesn't scroll away.
        <div style="position:absolute;top:0;left:0;right:0;height:3px;z-index:6;pointer-events:none">
          <div style={`width:${String(silentTopUpPct)}%;height:100%;background:var(--accent-primary);opacity:.5;transition:width .25s`} />
        </div>
      )}
    <div ref={containerRef} class={styles.container}>
      {showCover ? (
        // The single loading state: shown only while the store is EMPTY. The
        // instant anything paints — mirror tail (ts-ordered, so it is the real
        // end of the chat) or the first replay batch — the cover yields to
        // content and the rest of the history tops up silently behind it.
        <div class={styles.empty}>
          <LoadingCue
            title="Loading conversation…"
            pct={replayPct}
            sub={mcpUp ? 'Connecting MCP servers and replaying this session' : "Replaying this session's messages and tool calls"}
          >
            {/* After a few seconds the spinner alone is no longer information: it
                looks the same whether MCP took 40s, the replay never reported
                completion, or the view never bound. Show the phase timeline and
                which phase we are still waiting to leave, so a screenshot of the
                hang is a diagnosis and nobody needs devtools to report it. */}
            {coverSecs >= 3 && (
              <div style="margin-top:12px;max-width:420px;text-align:center">
                <div style="color:var(--text-disabled);font-size:11px;font-family:var(--font-mono,monospace);line-height:1.6;word-break:break-word">
                  {openLine}
                </div>
                <div style="color:var(--text-muted);font-size:11px;margin-top:4px">
                  waiting on <b>{waitingOn}</b> for {stalledSecs}s
                </div>
              </div>
            )}
          </LoadingCue>
        </div>
      ) : currentEntries.length === 0 ? (
        <div class={styles.empty}>
          <i class="fas fa-comments" style="font-size:32px;color:var(--text-disabled)" />
          <span style="color:var(--text-muted);font-size: var(--fs-md);font-weight:500">Session Log</span>
          <span style="color:var(--text-disabled);font-size:12px;text-align:center;max-width:240px;line-height:1.5">
            Messages and tool calls will appear here as the conversation flows
          </span>
        </div>
      ) : (
        (() => {
          type Prepared = { entry: JsonlEntryData; rendered: any; isAssistant: boolean }
          const prepared: Prepared[] = []

          // `visibleMerged` (segment-scoped, visibility-filtered, interrupt/skip
          // flagged) and `ver` come from the cached render model above. Mount only
          // the most-recent `renderCap` entries (the rest stay behind a "show
          // earlier" affordance) so a huge session doesn't build thousands of DOM
          // nodes in one synchronous pass and freeze the thread. The tail (recent
          // turns + any live stream) is always inside the window.
          const hiddenCount = Math.max(0, visibleMerged.length - renderCap)
          hiddenCountRef.current = hiddenCount // for the scroll-up auto-loader
          const shown = hiddenCount > 0 ? visibleMerged.slice(hiddenCount) : visibleMerged

          // Per-entry vnode cache (see vnodeCacheRef). A committed row's rendered
          // output only changes when `ver` (tool-result text sum + interrupt/skip
          // counts, computed in the render model) changes.
          const cache = vnodeCacheRef.current
          // The in-flight turn — last assistant + any trailing attachment/system
          // rows the CLI appends after it — churns through the stub→canonical
          // hand-off and must never be cached. Keep a generous uncached tail that
          // comfortably exceeds the trailing-metadata a single turn appends.
          const LIVE_TAIL_UNCACHED = 12
          const liveCutoff = shown.length - LIVE_TAIL_UNCACHED
          const usedCacheKeys = new Set<string>()

          // Collapse consecutive system/api_error retries — only the last is
          // interesting (latest attempt + most recent timestamp). Earlier
          // ones are pure noise during rate-limit back-off storms.
          for (let i = 0; i < shown.length; i++) {
            const e = shown[i]
            if (isApiErrorEntry(e) && i + 1 < shown.length && isApiErrorEntry(shown[i + 1])) {
              continue
            }
            const cacheKey = e.uuid ?? e.message?.id
            const cacheable = cacheKey != null && i < liveCutoff && !e.__streaming
            let rendered: JSX.Element | null
            if (cacheable && cacheKey != null) {
              usedCacheKeys.add(cacheKey)
              const hit = cache.get(cacheKey)
              if (hit && hit.ver === ver) {
                rendered = hit.vnode
              } else {
                rendered = JsonlEntry({ entry: e, toolResults })
                if (rendered != null) cache.set(cacheKey, { vnode: rendered, ver })
              }
            } else {
              rendered = JsonlEntry({ entry: e, toolResults })
            }
            if (rendered == null) continue
            // tool_group renders as an assistant-styled card, so treat it the
            // same for the merge-chain logic below.
            prepared.push({ entry: e, rendered, isAssistant: e.type === 'assistant' || e.type === 'tool_group' })
          }

          // Evict cache keys not rendered this pass. Bounds the cache to the
          // render window (fixes unbounded growth on long sessions) AND drops
          // stale entries: multi-block fusion adopts a new uuid per JSONL line so
          // intermediate steps leave dead keys, and an entry cached while hidden
          // is dropped so a later "show earlier" reveal re-renders it fresh.
          if (cache.size > usedCacheKeys.size) {
            for (const k of cache.keys()) if (!usedCacheKeys.has(k)) cache.delete(k)
          }

          // Keep the in-flight turn opted OUT of content-visibility so it keeps
          // painting through the stub→canonical hand-off (auto would collapse it
          // to its 80px intrinsic placeholder while off-screen). Cover the last
          // assistant entry AND everything after it — the CLI appends trailing
          // attachment/system rows, so the response isn't necessarily the very
          // last entry.
          const liveEntries = new Set<unknown>()
          {
            let li = -1
            for (let i = prepared.length - 1; i >= 0; i--) { if (prepared[i]!.isAssistant) { li = i; break } }
            const from = li >= 0 ? li : prepared.length - 1
            for (let i = Math.max(0, from); i < prepared.length; i++) liveEntries.add(prepared[i]!.entry)
          }

          // Group consecutive assistant-like entries into a single container
          // — a real DOM wrapper that owns the bg + shadow, so the run reads
          // as one fused card instead of N siblings juggling merge classes.
          const out: JSX.Element[] = []
          let runStart = -1
          for (let i = 0; i <= prepared.length; i++) {
            const p = prepared[i]
            if (p?.isAssistant) {
              if (runStart < 0) runStart = i
              continue
            }
            if (runStart >= 0) {
              const run = prepared.slice(runStart, i)
              out.push(
                <div key={`asst-run-${run[0]?.entry.message?.id ?? run[0]?.entry.uuid ?? (run[0]?.entry as { _ord?: number })?._ord ?? `i${runStart}`}`} class="asst-merge-container">
                  {run.map((rp, ri) => {
                    const mergeTop = ri > 0
                    const mergeBottom = ri < run.length - 1
                    const cls = [
                      styles.entryWrapper,
                      (rp.entry.__streaming || liveEntries.has(rp.entry)) ? styles.liveEntry : '',
                      mergeTop ? 'asst-merge-top' : '',
                      mergeBottom ? 'asst-merge-bottom' : '',
                    ].filter(Boolean).join(' ')
                    return (
                      // Key on message.id (stable through the stub→canonical
                      // merge) not uuid — the merge swaps the synthetic
                      // `stream-<id>` uuid for the real one, and a key change
                      // would REMOUNT the bubble (unmount+mount), making the
                      // finished answer visibly vanish then reappear.
                      <div key={rp.entry.message?.id ?? rp.entry.uuid ?? (rp.entry as { _ord?: number })._ord ?? ri} class={cls}>
                        {rp.rendered}
                      </div>
                    )
                  })}
                </div>,
              )
              runStart = -1
            }
            if (p) {
              const wrapCls = liveEntries.has(p.entry)
                ? `${styles.entryWrapper} ${styles.liveEntry}`
                : styles.entryWrapper
              out.push(
                // _ord стабилен в сторе — индексный фолбэк ремаунтил uuid-less строки при
                // prepend скролл-апа ровно в момент якорения вьюпорта (аудит #70 D8)
                <div key={p.entry.uuid ?? (p.entry as { _ord?: number })._ord ?? i} class={wrapCls}>
                  {p.rendered}
                </div>,
              )
            }
          }
          // Passive "more above" hint for the windowed-off older entries. No
          // button to click — the scroll host auto-loads the next chunk as the
          // user scrolls near the top (useChatScrollPin → onNearTop). Kept as a
          // muted spinner so the user knows scrolling up will reveal more.
          if (hiddenCount > 0) {
            out.unshift(
              <div key="__more-above" style="display:flex;align-items:center;justify-content:center;gap:8px;padding:10px 0 6px;color:var(--text-disabled);font-size:11px">
                <i class="fas fa-spinner fa-spin" style="font-size:12px" />
                <span>{hiddenCount} earlier {hiddenCount === 1 ? 'message' : 'messages'} — scroll up to load</span>
              </div>,
            )
          }
          return out
        })()
      )}
      {/* Tail spacer — gives the user a few extra pixels of scroll past the
          last message so the bottom card edge isn't pinned to the input bar
          and feels less cramped. */}
      <div aria-hidden="true" style="height:20px;flex-shrink:0" />
    </div>
    </>
  )
}

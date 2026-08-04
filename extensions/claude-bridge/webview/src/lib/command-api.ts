// `window.bridgeCmd` — drive the Bridge chat by COMMAND instead of by clicking.
//
// Reproducing a report ("send a /compact, check a new segment appears") through
// the UI means finding a button in an iframe inside a webview, and every such
// step is its own way to fail — so a failure never cleanly says whether the
// FEATURE or the driving broke. These commands call the same functions the
// buttons do (`sendMessageToTab`, `openDisplaySegment`), so a scripted run and a
// human click exercise one code path.
//
// Reads that can lie are the other half. `segments()` and `entries()` are empty
// or half-built while the view rebuilds (a compaction clears the store and
// re-replays), so every read carries `settling` and the waiters below refuse to
// resolve inside that window — see `lib/command-wait.ts`.
//
// `raw()` is the general case: every bridge channel by name, so the surface
// covers all of Bridge without this file having to enumerate it.
import type { ElectronBridge } from '../../shared/types'
import { activeTabId, tabs, switchTabLocal } from '../signals/tabs'
import { jsonlEntriesByTab } from '../signals/jsonl'
import { activeSegmentIdx, displaySegments } from '../signals/compact-segments'
import { formatOpenTimeline, getOpenTimeline, lastOpenPhase, stalledForMs } from '../signals/open-timeline'
import { sendMessageToTab } from './send-message'
import { openDisplaySegment } from './open-segment'
import { snapshot } from './command-state'
import { previewEntry, type BridgeState, type EntryPreview } from './command-model'
import { waitFor, waitForIdle, waitForChange, type StatePredicate, type IdleOptions } from './command-wait'

/** A read that says whether it can be trusted, instead of returning `[]` during
 *  a rebuild and letting the caller read that as "there are none". */
export interface Reading<T> {
  settling: boolean
  items: T[]
}

export interface SegmentInfo {
  idx: number
  key: string
  ts: string
  count: number
  latest: boolean
  active: boolean
}

export interface BridgeCommandApi {
  help(): string[]
  state(): BridgeState
  timeline(): { line: string; waitingOn: string; stalledMs: number; marks: unknown[] }
  tabs(): Array<{ id: string; label: string; title: string; status: string; active: boolean }>
  switchTab(id: string): Promise<BridgeState>
  send(text: string): string
  sendAndWait(text: string, opts?: IdleOptions): Promise<BridgeState>
  stop(): void
  entries(n?: number): Reading<EntryPreview>
  segments(): Reading<SegmentInfo>
  openSegment(idx: number): Promise<BridgeState>
  raw(method: string, ...args: unknown[]): unknown
  channels(): string[]
  waitFor(check: StatePredicate, timeoutMs?: number): Promise<BridgeState>
  waitForIdle(opts?: IdleOptions): Promise<BridgeState>
  waitForChange(field: keyof BridgeState, timeoutMs?: number): Promise<BridgeState>
}

export function installCommandApi(bridge: ElectronBridge): void {
  const requireTab = (): string => {
    const id = activeTabId.value
    if (!id) throw new Error('no active tab')
    return id
  }

  const api: BridgeCommandApi = {
    help: () => [
      'state() — …/working/settling/busy. `settling` = the view is mid-rebuild; reads are NOT trustworthy',
      'waitForIdle({stableMs=1500}) — the ONE to await after send(): not busy AND state held still',
      'waitFor(pred, ms) — pred is a function over the state snapshot',
      'waitForChange(field, ms) — resolve when a field actually moves',
      'sendAndWait(text) — send() then waitForIdle(); use for /compact',
      'send(text) — same path as the Send button; returns the tab it went to',
      'stop() — interrupt the current turn',
      'tabs() / switchTab(id) — switchTab awaits the new tab binding',
      'entries(n=10) / segments() — return {settling, items}; ignore items when settling',
      'openSegment(idx) — awaits the segment load',
      'raw(method, ...args) — call ANY bridge channel by name; channels() lists them',
    ],

    state: snapshot,

    timeline: () => {
      const id = activeTabId.value
      return {
        line: formatOpenTimeline(id),
        waitingOn: lastOpenPhase(id),
        stalledMs: stalledForMs(id),
        marks: getOpenTimeline(id),
      }
    },

    tabs: () =>
      tabs.value.map((t) => ({
        id: t.id,
        label: t.label ?? '',
        title: t.sessionTitle ?? '',
        status: t.status ?? '',
        active: t.id === activeTabId.value,
      })),

    /** Awaits the switch: the new tab's transcript has to replay and bind, and
     *  until it does every read still describes the OLD session. */
    switchTab: (id: string) => {
      switchTabLocal(id)
      return waitForIdle()
    },

    /** Returns the tab it went to, so a script can assert the target rather than
     *  assume the active tab didn't move under it. */
    send: (text: string) => {
      const id = requireTab()
      sendMessageToTab(bridge, id, text)
      return id
    },

    /** Send, then wait for the view to settle — not merely for the turn to end.
     *  A compaction ends its turn and THEN rebuilds the store and the segment
     *  strip; stopping at the turn boundary reads the gap. */
    sendAndWait: async (text: string, opts?: IdleOptions) => {
      const id = requireTab()
      sendMessageToTab(bridge, id, text)
      // Let the send register before demanding stillness, or the pre-send calm
      // satisfies the quiescence window immediately.
      await waitFor((s) => s.working || !s.promptReady, 15_000).catch(() => snapshot())
      return waitForIdle(opts)
    },

    stop: () => { bridge.interruptSession(requireTab()) },

    entries: (n = 10) => {
      const s = snapshot()
      const id = activeTabId.value
      const all = id ? (jsonlEntriesByTab.value.get(id) ?? []) : []
      return { settling: s.settling, items: all.slice(-n).map(previewEntry) }
    },

    segments: () => {
      const s = snapshot()
      return {
        settling: s.settling,
        items: displaySegments.value.map((seg, i) => ({
          idx: i,
          key: seg.key,
          ts: seg.ts ?? '',
          count: seg.count ?? 0,
          latest: !!seg.isLatest,
          active: seg.residentIdx === activeSegmentIdx.value,
        })),
      }
    },

    openSegment: (idx: number) => {
      const segs = displaySegments.value
      const seg = segs[idx]
      if (!seg) throw new Error(`no segment ${idx} (have ${segs.length})`)
      openDisplaySegment(bridge, seg, segs.length)
      return waitForIdle()
    },

    raw: (method: string, ...args: unknown[]) => {
      const fn = (bridge as unknown as Record<string, unknown>)[method]
      if (typeof fn !== 'function') throw new Error(`no bridge method "${method}"`)
      return (fn as (...a: unknown[]) => unknown)(...args)
    },

    channels: () =>
      Object.keys(bridge as unknown as Record<string, unknown>)
        .filter((k) => typeof (bridge as unknown as Record<string, unknown>)[k] === 'function')
        .sort(),

    waitFor,
    waitForIdle,
    waitForChange: (field, timeoutMs) => waitForChange(field, timeoutMs),
  }

  ;(window as unknown as Record<string, unknown>).bridgeCmd = api
}

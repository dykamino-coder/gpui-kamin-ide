import { signal, computed } from '@preact/signals'
import { noteCompactionStream } from './compaction'
import { activeTabId } from './tabs'
import type { JsonlEntryData } from '../types/jsonl'
import { orderEntries } from './order-entries'
import { mergeAssistantEntry, foldAssistantEntries } from './merge-assistant'
import { setReplayProgress } from './replay-progress'
import { dropRenderModelCache } from '../components/jsonl-viewer/derived-cache'
import { trimHeavyToolResults } from './heavy-tool-result'
import { appendStreamingText } from '../../../extension/src/shared/streaming-delta'
import { CHAT_STORED_ENTRY_TYPES, isChatStoredEntry } from '../../../extension/src/shared/chat-stored-entries'

// Global per-tab JSONL entries store — single source of truth.
// Updated by useBridgeListeners (onJsonlEntries handler).
export const jsonlEntriesByTab = signal<Map<string, JsonlEntryData[]>>(new Map())

// Per-tab STRUCTURE version — bumped ONLY when a tab's entry set changes shape
// (entry added/removed/reordered/merged, or an entry replaced by a structurally
// different one). A pure streaming TEXT delta (in-place stub growth + the per-rAF
// ref-refresh clone) does NOT bump it. The render pipeline (derived-cache) keys
// its heavy merge/group/chain/visibility memo on this: while it's unchanged, a
// 30×/s stream flush reuses the cached model and only re-points the live stub
// objects (O(tail)) instead of recomputing over the whole segment (the huge-
// session freeze). Non-reactive: it's read synchronously during the render that
// the entries-signal write already triggers, never subscribed to on its own.
const structureVersionByTab = new Map<string, number>()
export function getJsonlStructureVersion(tabId: string): number {
  return structureVersionByTab.get(tabId) ?? 0
}
function bumpStructureVersion(tabId: string): void {
  structureVersionByTab.set(tabId, (structureVersionByTab.get(tabId) ?? 0) + 1)
}
// INVARIANT: a tab's seen set holds exactly the uuids its store holds (including
// every uuid folded into a merged assistant bubble). Every path that removes
// rows rebuilds it from what is left (`reindexSeen`), so a row that left the
// store is re-admitted when a replay or a page brings it back. A uuid that
// stays "seen" without its row rejects the whole replayed conversation as
// `uuid-dup`, and the chat stays blank while the console shows everything.
export const jsonlSeenUuidsByTab = signal<Map<string, Set<string>>>(new Map())
// Per-tab dedup keys of the uuid-less rows (mode / permission-mode /
// queue-operation). The key is the row's file position `_pos`, which is the same
// on every delivery of that line. `_ord` is only a fallback: every server replay
// re-tags it from a global counter, so an `_ord` key let each reattach or
// reconnect replay store the same uuid-less rows again. Same invariant as the
// uuid set. Non-reactive: pure bookkeeping, reset in clearJsonlEntries.
const seenRowKeysByTab = new Map<string, Set<string>>()

/** Stable dedup key for a uuid-less row, or undefined if it has none. */
function rowKey(e: JsonlEntryData): string | undefined {
  if (typeof e._pos === 'number') return `p${String(e._pos)}`
  const ord = (e as { _ord?: number })._ord
  return typeof ord === 'number' ? `o${String(ord)}` : undefined
}

function entryUuids(e: JsonlEntryData): string[] {
  return e.__uuids ?? (e.uuid ? [e.uuid] : [])
}

/** Rebuild both dedup sets from the rows the store holds now. */
function reindexSeen(tabId: string, entries: readonly JsonlEntryData[]): void {
  const seen = new Set<string>()
  const keys = new Set<string>()
  for (const e of entries) {
    const uuids = entryUuids(e)
    if (uuids.length > 0) { for (const u of uuids) seen.add(u); continue }
    const key = rowKey(e)
    if (key) keys.add(key)
  }
  const nextSeen = new Map(jsonlSeenUuidsByTab.value)
  nextSeen.set(tabId, seen)
  jsonlSeenUuidsByTab.value = nextSeen
  seenRowKeysByTab.set(tabId, keys)
}

// ── Store memory cap (the OOM backstop) ──────────────────────────────────
// The resident-store bounds. Canonical home is here, in the signal layer that
// OWNS the store — the JsonlViewer imports them. A single WebView2 renderer
// hosts every role's iframe AND every tab's store; a marathon transcript held
// whole is what drove it past its ~4GB heap ceiling (OUT_OF_MEMORY).
//
// STORE_WINDOW  — entries the active tab keeps resident; older pages come back
//                 from the disk mirror on scroll-up.
// STORE_SLACK   — trim hysteresis, so a live dialog trims once every SLACK turns
//                 instead of re-running the render model on every append.
// SCROLL_UP_MAX — the active tab's scroll-up ceiling (also the runaway backstop).
export const STORE_WINDOW = 4000
export const STORE_SLACK = 500
export const SCROLL_UP_MAX = 16000

// Tab → the boundary ts of the ARCHIVED compact segment the user is currently
// viewing (loaded from the mirror, out of the live window). While set, live
// entries for that tab are NOT appended: they belong to the CURRENT segment, not
// the archived snapshot on screen, and folding them in would corrupt the view
// (and re-grow it toward the OOM). They aren't lost — they stay in the server +
// mirror and reappear when the user returns to Current (which clears this and
// re-requests the live tail). Non-reactive: read as a gate, set by the loader.
const archivedViewByTab = new Map<string, string>()
const liveWindows = new Map<string, JsonlEntryData[]>()
const archivedLiveEntries = new Map<string, JsonlEntryData[]>()
const archivedStreams = new Map<string, Map<string, JsonlEntryData>>()
export function setArchivedView(tabId: string, ts: string | null): void {
  if (ts === null) {
    archivedViewByTab.delete(tabId)
    const live = liveWindows.get(tabId)
    const pending = archivedLiveEntries.get(tabId)
    liveWindows.delete(tabId)
    archivedLiveEntries.delete(tabId)
    if (live) replaceWindowWithSegment(tabId, live)
    if (pending) appendJsonlEntries(tabId, pending)
    const streams = archivedStreams.get(tabId)
    archivedStreams.delete(tabId)
    if (streams) for (const entry of streams.values()) applyStreamingEntry(tabId, entry)
  } else {
    if (!archivedViewByTab.has(tabId)) liveWindows.set(tabId, jsonlEntriesByTab.value.get(tabId) ?? [])
    archivedViewByTab.set(tabId, ts)
  }
}
export function archivedViewTs(tabId: string): string | undefined {
  return archivedViewByTab.get(tabId)
}

// ── #78 delivery diagnostic ──────────────────────────────────────────────
// A capped ring of entries the store DROPPED (dedup) — so "message showed in
// the console but not the chat" can be root-caused: the header bug button dumps
// this + the current store, and the dropped row + reason are right there.
interface DiagRec { t: number; reason: string; uuid?: string; ord?: number; msgId?: string; type?: string; role?: string; preview?: string }
const diagLog: DiagRec[] = []
const DIAG_CAP = 800
function entryPreview(e: JsonlEntryData): string {
  const c = e.message?.content as unknown
  if (typeof c === 'string') return c.slice(0, 80)
  if (Array.isArray(c)) {
    const t = c.find((b) => (b as { type?: string })?.type === 'text') as { text?: string } | undefined
    if (t?.text) return t.text.slice(0, 80)
  }
  return ''
}
function logDrop(reason: string, e: JsonlEntryData): void {
  diagLog.push({
    t: Date.now(), reason,
    uuid: e.uuid, ord: (e as { _ord?: number })._ord,
    msgId: e.message?.id, type: e.type,
    role: (e.message as { role?: string } | undefined)?.role,
    preview: entryPreview(e),
  })
  if (diagLog.length > DIAG_CAP) diagLog.splice(0, diagLog.length - DIAG_CAP)
}

/** Snapshot for the #78 diagnostic button: the current chat store (row
 *  summaries) + the recent drop log. Copy it when a message shows in the
 *  console but not the chat — the dropped row + reason pinpoint the cause. */
export function collectSessionDiag(tabId: string): unknown {
  const entries = jsonlEntriesByTab.value.get(tabId) ?? []
  return {
    ts: new Date().toISOString(),
    tabId,
    storeCount: entries.length,
    seenUuids: jsonlSeenUuidsByTab.value.get(tabId)?.size ?? 0,
    store: entries.map((e) => ({
      uuid: e.uuid, ord: (e as { _ord?: number })._ord, type: e.type,
      // parentUuid + these flags are what make the dump ANSWER a blank chat
      // instead of hinting at one: the viewer hides a user/assistant row when
      // the parentUuid chain walk (computeChainUuids) doesn't reach it, and
      // without the parent links a dump can't replay that walk offline. A real
      // "chat is empty but the store has 16k entries" report cost a whole
      // investigation that ended at "I can't tell from this dump".
      parentUuid: (e as { parentUuid?: string }).parentUuid ?? null,
      isSidechain: (e as { isSidechain?: boolean }).isSidechain ?? false,
      isMeta: (e as { isMeta?: boolean }).isMeta ?? false,
      subtype: (e as { subtype?: string }).subtype,
      role: (e.message as { role?: string } | undefined)?.role,
      msgId: e.message?.id, streaming: e.__streaming !== undefined,
      preview: entryPreview(e),
    })),
    drops: diagLog.slice(-400),
  }
}

// Message ids the user INTERRUPTED (Esc / Stop → \x03 SIGINT to the CLI). The
// console (raw PTY) stops instantly on SIGINT, but the server's MITM proxy keeps
// flushing already-buffered stream frames → the chat "kept spewing the whole
// answer" past the stop. Drop further STREAM frames for these ids so the chat
// freezes where the console did. The canonical JSONL (the partial answer the CLI
// actually committed + the `[Request interrupted by user]` marker) still lands
// via onJsonlEntries → appendJsonlEntries (a DIFFERENT channel, not blocked) and
// reconciles by message.id. Keyed by msgId (unique per turn) so the NEXT turn
// streams normally; capped so it can't grow unbounded.
const abortedStreamIds = new Set<string>()
const ABORTED_STREAM_IDS_CAP = 100

/** Freeze the tab's in-flight streamed answer(s) on user interrupt: mark the
 *  currently-streaming message ids so subsequent MITM stream frames are dropped.
 *  Idempotent; safe to call when nothing is streaming. */
export function abortStreamingForTab(tabId: string): void {
  const entries = jsonlEntriesByTab.value.get(tabId)
  if (!entries) return
  for (const e of entries) {
    if (e.__streaming !== undefined && e.message?.id) abortedStreamIds.add(e.message.id)
  }
  while (abortedStreamIds.size > ABORTED_STREAM_IDS_CAP) {
    const first = abortedStreamIds.values().next().value
    if (first === undefined) break
    abortedStreamIds.delete(first)
  }
}

/** Apply or replace a single streaming assistant entry by message.id. Used
 *  by the bridge MITM proxy stream — emits `streaming:entry` updates that
 *  carry a synthetic `stream-${messageId}` uuid. We replace any existing
 *  entry that carries the same `message.id` (a previous streaming snapshot
 *  OR a real watcher-delivered entry that arrived first). */
/** Поиск записи по message.id С ХВОСТА: цель стрима (стаб/свежий ход) живёт
 *  у конца стора, а findIndex с фронта на 16k-сторе сканировал почти всё
 *  30 раз в секунду (аудит #70 B3/B5). Корректность та же — id уникален. */
function findByMsgIdFromTail(entries: JsonlEntryData[], msgId: string): number {
  for (let i = entries.length - 1; i >= 0; i--) {
    if (entries[i]!.message?.id === msgId) return i
  }
  return -1
}

export function applyStreamingEntry(tabId: string, entry: JsonlEntryData): void {
  if (noteCompactionStream(tabId, entry)) return // a status, not an answer
  const msgId = entry.message?.id
  if (!msgId) return
  if (abortedStreamIds.has(msgId)) return // user interrupted this turn — freeze
  if (archivedViewByTab.has(tabId)) {
    const streams = archivedStreams.get(tabId) ?? new Map<string, JsonlEntryData>()
    streams.set(msgId, entry)
    while (streams.size > 100) streams.delete(streams.keys().next().value!)
    archivedStreams.set(tabId, streams)
    return
  }
  const existing = jsonlEntriesByTab.value.get(tabId) ?? []
  const idx = findByMsgIdFromTail(existing, msgId)
  // Canonical always wins: once the watcher delivered the real entry
  // (`__streaming` stripped by mergeAssistantEntry), a late throttle flush of
  // the proxy stub must NOT resurrect it — the next watcher merge saw the
  // stub marker, reset the content base and wiped the already-merged blocks
  // (their uuids are burned in the seen-set, so they never came back).
  if (idx >= 0 && existing[idx]!.__streaming === undefined) return
  // Stamp LAST-ACTIVITY time so the TTL sweep can retire an orphaned stub — one
  // whose canonical JSONL entry never arrives (API abort → CLI retries with a
  // new msg_id; or a subagent turn whose canonical goes to the subagent store,
  // not this one). Refresh it on EVERY write (not first-seen): otherwise a long
  // answer that streams past STUB_TTL_MS is swept WHILE STILL STREAMING — the
  // text vanishes mid-turn and later deltas drop (no stub) until the final
  // snapshot recreates it (#73). A truly orphaned stub stops being written and
  // ages out normally.
  const prev = idx >= 0 ? existing[idx]! : undefined
  const stamped: JsonlEntryData = {
    ...entry,
    __streamStartedAt: Date.now(),
    __streamAnchorTs: prev?.__streamAnchorTs ?? newestCanonicalTs(existing),
    // Canonical rows held under the stub (holdStreamedProse) survive a snapshot.
    ...(prev?.__assistantParts ? { __assistantParts: prev.__assistantParts, __uuids: prev.__uuids } : {}),
  }
  let next: JsonlEntryData[]
  if (idx >= 0) {
    next = existing.slice()
    next[idx] = stamped
  } else {
    next = existing.concat(stamped)
  }
  const nextMap = new Map(jsonlEntriesByTab.value)
  nextMap.set(tabId, next)
  // Structural: this either appends a brand-new stub or swaps a stub for a
  // fresh snapshot (block count can change). Always a shape change.
  bumpStructureVersion(tabId)
  jsonlEntriesByTab.value = nextMap
  ensureStubSweeper()
  if (entry.uuid) {
    let seen = jsonlSeenUuidsByTab.value.get(tabId)
    if (!seen) {
      seen = new Set()
      const nextSeen = new Map(jsonlSeenUuidsByTab.value)
      nextSeen.set(tabId, seen)
      jsonlSeenUuidsByTab.value = nextSeen
    }
    seen.add(entry.uuid)
  }
}

// Coalesce streaming-delta signal writes to ONE per animation frame. Deltas
// fire ~30x/sec; each used to reassign the signal, re-running the JsonlViewer's
// O(N) merge/group/chainUuids memos + a reconcile per delta. Now the block text
// is mutated in place immediately (cheap) and a single new-ref snapshot is
// flushed per frame, so a burst of deltas costs one re-render instead of N.
const dirtyStreamTabs = new Set<string>()
let streamFlushHandle: ReturnType<typeof requestAnimationFrame> | ReturnType<typeof setTimeout> | null = null
const rafAvailable = typeof requestAnimationFrame === 'function'

function scheduleStreamFlush(): void {
  if (streamFlushHandle !== null) return
  const run = (): void => {
    streamFlushHandle = null
    if (dirtyStreamTabs.size === 0) return
    const nextMap = new Map(jsonlEntriesByTab.value)
    let changed = false
    for (const tabId of dirtyStreamTabs) {
      const entries = nextMap.get(tabId)
      if (!entries) continue
      // Fresh refs ONLY for the in-place-mutated streaming stubs so Preact
      // re-renders them; canonicalized entries (no __streaming) are left alone.
      // Сначала дешёвый скан индексов (без аллокаций): клон всего стора через
      // map на каждый rAF давал 16k-массив + 16k вызовов замыкания + МБ/с GC
      // ради 1-2 стабов (аудит #70 B4).
      const stubIdx: number[] = []
      for (let i = entries.length - 1; i >= 0; i--) {
        const e = entries[i]!
        if (e.__streaming !== undefined && e.message) stubIdx.push(i)
      }
      if (stubIdx.length === 0) continue
      const cloned = entries.slice()
      for (const i of stubIdx) {
        const e = cloned[i]!
        const content = Array.isArray(e.message!.content) ? e.message!.content.slice() : e.message!.content
        cloned[i] = { ...e, message: { ...e.message!, content }, __streamStartedAt: Date.now() }
      }
      nextMap.set(tabId, cloned)
      changed = true
    }
    dirtyStreamTabs.clear()
    if (changed) { jsonlEntriesByTab.value = nextMap; ensureStubSweeper() }
  }
  streamFlushHandle = rafAvailable ? requestAnimationFrame(run) : setTimeout(run, 16)
}

/** Apply an incremental text/thinking append to an existing streaming stub
 *  (protocol>=1). The opening boundary snapshot (content_block_start) always
 *  precedes a block's deltas, so a missing stub or missing block means the
 *  entry was already canonicalized (watcher won) or suppressed (side-quest /
 *  thinking filter) — drop the delta; the next boundary snapshot self-heals. */
export function applyStreamingDelta(
  tabId: string,
  d: { msgId: string; blockIdx: number; appendText: string; kind: 'text' | 'thinking' },
): void {
  if (abortedStreamIds.has(d.msgId)) return // user interrupted this turn — freeze
  const existing = jsonlEntriesByTab.value.get(tabId)
  if (!existing) return
  const idx = findByMsgIdFromTail(existing, d.msgId)
  if (idx < 0) return
  const stub = existing[idx]!
  // Canonical wins — never mutate a watcher-delivered entry (mirrors the
  // applyStreamingEntry guard at line 31).
  if (stub.__streaming === undefined) return
  // INVARIANT (load-bearing): a delta only ever appends text to an ALREADY-
  // EXISTING block — it never creates, removes, or reorders one. This is what
  // lets scheduleStreamFlush stay TEXT-ONLY (no structureVersion bump): the
  // render-model cache reuses the whole segment and only re-points stub objects.
  // Materializing a missing block here would silently change structure with no
  // bump → stale-cache frozen chat. New blocks arrive via applyStreamingEntry
  // (a snapshot, which DOES bump). So: bail if the block isn't there yet.
  if (!appendStreamingText(stub, d)) return
  // Block mutated IN PLACE above; defer the ref-snapshot + signal write to a
  // coalesced rAF flush (was a per-delta reassign → per-delta O(N) memo+reconcile).
  dirtyStreamTabs.add(tabId)
  scheduleStreamFlush()
}

// TTL for an unreplaced streaming stub. Real canonical entries arrive within
// ~100ms–2s of the stream finishing; 20s is comfortably past that, so anything
// still marked `__streaming` after it is orphaned and gets swept.
const STUB_TTL_MS = 20_000
let stubSweeper: ReturnType<typeof setInterval> | null = null

function ensureStubSweeper(): void {
  if (stubSweeper) return
  stubSweeper = setInterval(sweepOrphanStubs, 5_000)
}

/** Does a stub carry text the user has already seen? An empty stub (block
 *  started, no delta yet) is disposable; a stub with real text/thinking is a
 *  rendered answer we must not erase. */
export function stubHasContent(e: JsonlEntryData): boolean {
  const c = e.message?.content
  if (!Array.isArray(c)) return false
  for (const b of c as { type?: string; text?: string; thinking?: string }[]) {
    if (b?.type === 'text' && typeof b.text === 'string' && b.text.trim()) return true
    if (b?.type === 'thinking' && typeof b.thinking === 'string' && b.thinking.trim()) return true
  }
  return false
}

/** A stub is SUPERSEDED once a later entry starts a new turn — a committed
 *  assistant with a different message.id (an API-abort retry) or a user entry.
 *  A superseded stub whose canonical never merged is a true orphan and gets
 *  swept. A stub that is still the tail is simply awaiting its canonical, which
 *  can lag well past the TTL on a busy/marathon session or when the CLI defers
 *  the final flush — erasing THAT is the "answer streamed then vanished" bug. */
export function stubSuperseded(entries: JsonlEntryData[], i: number): boolean {
  const stub = entries[i]!
  const msgId = stub.message?.id
  for (let j = i + 1; j < entries.length; j++) {
    const e = entries[j]!
    if (e.type === 'user') return true
    if (e.type === 'assistant' && e.__streaming === undefined && e.message?.id && e.message.id !== msgId) return true
  }
  // orderEntries sorts every stub after all canonical rows (it has no timestamp
  // or position), so the loop above never sees a successor in a sorted store.
  // A turn the CLI wrote AFTER the stub appeared has a later CLI timestamp than
  // anything the store held then (D6: an aborted attempt under its retry).
  const anchor = stub.__streamAnchorTs
  if (typeof anchor !== 'number') return false
  for (const e of entries) {
    if (e.__streaming !== undefined || !startsTurn(e, msgId)) continue
    const ts = e.timestamp ? Date.parse(e.timestamp) : NaN
    if (ts > anchor) return true
  }
  return false
}

/** A canonical row that begins a new turn: a real user message (not a tool
 *  result or meta row, which the CLI writes inside a turn), or another API message. */
function startsTurn(e: JsonlEntryData, msgId: string | undefined): boolean {
  if (e.type === 'assistant') return !!e.message?.id && e.message.id !== msgId
  if (e.type !== 'user' || e.isMeta) return false
  const c = e.message?.content
  return !(Array.isArray(c) && c.length > 0 && c.every((b) => b?.type === 'tool_result'))
}

/** Newest CLI timestamp among the canonical rows near the end of the store. */
function newestCanonicalTs(entries: readonly JsonlEntryData[]): number {
  let max = Number.NEGATIVE_INFINITY
  for (let i = entries.length - 1, seen = 0; i >= 0 && seen < 64; i--) {
    const e = entries[i]!
    if (e.__streaming !== undefined) continue
    seen++
    const ts = e.timestamp ? Date.parse(e.timestamp) : NaN
    if (ts > max) max = ts
  }
  return max
}

function textLength(e: JsonlEntryData): number {
  const c = e.message?.content
  if (!Array.isArray(c)) return 0
  let n = 0
  for (const b of c) if (b?.type === 'text' && typeof b.text === 'string') n += b.text.trim().length
  return n
}

/** D5 / INC-2026-0088: the CLI writes a turn's thinking row before its text
 *  row. Folding that row into the stub replaced the streamed prose with an
 *  empty canonical body until the text row landed. While the canonical rows
 *  carry no text yet, keep the stub on screen and hold them under it; the
 *  first canonical text takes over, byte-exact. */
function holdStreamedProse(target: JsonlEntryData, merged: JsonlEntryData): JsonlEntryData {
  if (target.__streaming === undefined || textLength(target) === 0 || textLength(merged) > 0) return merged
  return { ...target, __assistantParts: merged.__assistantParts, __uuids: merged.__uuids }
}

/** The canonical entry a held stub stands for, built from its held rows. */
function foldHeldParts(stub: JsonlEntryData): JsonlEntryData | null {
  const parts = stub.__assistantParts
  if (!parts || parts.length === 0) return null
  return parts.slice(1).reduce((acc, p) => mergeAssistantEntry(acc, p), parts[0]!)
}

function sweepOrphanStubs(): void {
  const now = Date.now()
  let changed = false
  const nextMap = new Map(jsonlEntriesByTab.value)
  let anyStubsLeft = false
  for (const [tabId, entries] of nextMap) {
    let replaced = false
    // flatMap, not in-place: committed store arrays are immutable.
    const kept = entries.flatMap((e, i): JsonlEntryData[] => {
      // A canonical entry has __streaming === undefined (stripped on merge);
      // only stubs carry a defined flag + a start stamp.
      if (e.__streaming === undefined) return [e]
      const started = e.__streamStartedAt
      if (typeof started !== 'number') return [e]
      if (now - started <= STUB_TTL_MS) { anyStubsLeft = true; return [e] }
      // TTL expired. Only sweep a stub that a later turn has SUPERSEDED — that is
      // a genuine orphan (retry / interrupt). A tail stub that still carries
      // content is kept: its canonical merges when it finally arrives, and until
      // then a shown answer beats a vanished one. Keep the sweeper alive
      // (anyStubsLeft) so the moment a successor appears the orphan is swept.
      if (stubHasContent(e) && !stubSuperseded(entries, i)) { anyStubsLeft = true; return [e] }
      // #78 telemetry: record WHY a streamed bubble was dropped so the next
      // "streamed then vanished" report is root-caused from the bug-dump instead
      // of showing an empty drop log.
      logDrop(stubHasContent(e) ? 'orphan-stub-superseded' : 'orphan-stub-empty', e)
      // Canonical rows held under the stub are real transcript rows: they stay.
      const held = foldHeldParts(e)
      if (held) { replaced = true; return [held] }
      return []
    })
    if (kept.length !== entries.length || replaced) {
      nextMap.set(tabId, kept)
      changed = true
      bumpStructureVersion(tabId)
      reindexSeen(tabId, kept)
    }
  }
  if (changed) jsonlEntriesByTab.value = nextMap
  // Stop the timer once no stubs remain — restarts on the next stream.
  if (!anyStubsLeft && stubSweeper) { clearInterval(stubSweeper); stubSweeper = null }
}

/** The store keeps only the types the chat renders or reads; the list is shared
 *  with the host replay tail (see CHAT_STORED_ENTRY_TYPES). A production session
 *  whose store held only `cost-state` and telemetry rows showed an empty chat
 *  while the console showed the whole conversation. */
export const STORED_ENTRY_TYPES = CHAT_STORED_ENTRY_TYPES
const isStoredEntry = (e: JsonlEntryData): boolean => isChatStoredEntry(e)

export function appendJsonlEntries(tabId: string, entries: JsonlEntryData[]): boolean {
  if (entries.length === 0) return false
  // Drop the rows nothing reads before they enter the store, or the archived
  // view's pending buffer. They used to reach the renderer and be filtered at the
  // last moment, so each one still took a slot, a vnode and a place in every
  // derived model.
  entries = entries.filter(isStoredEntry)
  if (entries.length === 0) return false
  // Viewing an archived compact segment: live entries belong to the CURRENT
  // segment, not this snapshot — dropping them here keeps the archived view
  // intact (they persist server-side and return when the user goes back to
  // Current). See archivedViewByTab.
  if (archivedViewByTab.has(tabId)) {
    const pending = archivedLiveEntries.get(tabId) ?? []
    const next = pending.concat(entries)
    // Overflow loses live rows. Going back to Current re-requests a replay
    // (openDisplaySegment), and the debt makes sure that refill is owed.
    if (next.length > STORE_WINDOW) needsRefill.add(tabId)
    archivedLiveEntries.set(tabId, next.slice(-STORE_WINDOW))
    return false
  }
  // Offload the bytes of fat tool_result bodies before they enter the store —
  // the UI never shows past 2000 chars of one, and holding whole multi-hundred-KB
  // results is what drove the renderer to OUT_OF_MEMORY. Image payloads untouched.
  for (const e of entries) trimHeavyToolResults(e)

  let seen = jsonlSeenUuidsByTab.value.get(tabId)
  let seenIsNew = false
  if (!seen) { seen = new Set<string>(); seenIsNew = true }
  let keySeen = seenRowKeysByTab.get(tabId)
  if (!keySeen) { keySeen = new Set<string>(); seenRowKeysByTab.set(tabId, keySeen) }
  const existing = jsonlEntriesByTab.value.get(tabId) ?? []

  // Reconcile UUIDs and split messages first; order their source rows and the
  // resulting bubbles below, independently of replay/page delivery order.
  const ordered = entries

  // Walk the batch building a NEW list lazily. Track per-batch
  // `message.id → index` so multiple watcher entries with the same id
  // (CLI's per-content-block split) merge into one entry instead of
  // racing each other into the same `replacements[]` slot.
  let mutated = false
  let nextEntries: JsonlEntryData[] = existing
  const msgIdToIdx = new Map<string, number>()
  for (const e of ordered) {
    // uuid first; uuid-less rows by file position (see seenRowKeysByTab).
    // `_ord` is re-tagged on every server replay, so it is only the fallback
    // for a row that has no position.
    if (e.uuid) {
      if (seen.has(e.uuid)) { logDrop('uuid-dup', e); continue }
      seen.add(e.uuid)
    } else {
      const key = rowKey(e)
      if (key !== undefined) {
        if (keySeen.has(key)) { logDrop(key.startsWith('p') ? 'pos-dup' : 'ord-dup', e); continue }
        keySeen.add(key)
      }
    }
    const incomingMsgId = e.message?.id
    if (incomingMsgId && e.type === 'assistant') {
      let targetIdx = msgIdToIdx.get(incomingMsgId) ?? -1
      if (targetIdx < 0) {
        // Bridge from existing list: look for a streaming stub OR an
        // already-stored real entry that we should fold into. С ХВОСТА:
        // цель — стаб/свежий ход у конца, скан с фронта на реплее давал
        // миллионы сравнений суммарно (аудит #70 B3).
        for (let i = existing.length - 1; i >= 0; i--) {
          const x = existing[i]!
          if (x.type === 'assistant' && x.message?.id === incomingMsgId) { targetIdx = i; break }
        }
      }
      if (targetIdx >= 0) {
        if (!mutated) { nextEntries = existing.slice(); mutated = true }
        const target = nextEntries[targetIdx]!
        nextEntries[targetIdx] = holdStreamedProse(target, mergeAssistantEntry(target, e))
        msgIdToIdx.set(incomingMsgId, targetIdx)
        continue
      }
    }
    if (!mutated) { nextEntries = existing.slice(); mutated = true }
    nextEntries.push(e)
    if (incomingMsgId && e.type === 'assistant') {
      msgIdToIdx.set(incomingMsgId, nextEntries.length - 1)
    }
  }
  if (!mutated) return false

  // Batch-only ordering preserves file positions when complete, timestamp
  // chronology for mixed legacy history, and parent-before-child edges.
  nextEntries = orderEntries(nextEntries)

  const nextMap = new Map(jsonlEntriesByTab.value)
  nextMap.set(tabId, nextEntries)
  bumpStructureVersion(tabId) // entries added / merged / reordered — shape change
  jsonlEntriesByTab.value = nextMap

  if (seenIsNew) {
    const nextSeen = new Map(jsonlSeenUuidsByTab.value)
    nextSeen.set(tabId, seen)
    jsonlSeenUuidsByTab.value = nextSeen
  }

  enforceTabCap(tabId)

  // Best-effort reconcile of the local visual queue: pops items that the CLI
  // has now recorded as user entries. Lazy-imported to avoid circular deps.
  try {
    import('./queue').then(q => q.reconcileQueueWithEntries?.(tabId)).catch(() => {})
  } catch { /* skip on SSR edge */ }

  return true
}

/** Tabs whose store we dropped ourselves (memory eviction, compaction clear).
 *
 *  A cleared tab OWES a refill. Requesting one only when the store is EMPTY was
 *  not enough: a live entry arriving after the clear (a `/reload-skills` echo is
 *  enough) makes the store non-empty, so the refill was never asked for and the
 *  session stayed stripped of its history with no way back. A diagnostic from
 *  such a session showed 3 entries, all post-clear, and — decisively — an empty
 *  drop log: nothing had been rejected, nothing had arrived. */
const needsRefill = new Set<string>()

export function tabNeedsRefill(tabId: string): boolean {
  return needsRefill.has(tabId)
}

/** Does the tab still owe its conversation? It does when we dropped its store
 *  and no user or assistant row has come back since.
 *
 *  "The store is non-empty" is not the same thing. After a compaction clear the
 *  live tail keeps writing bookkeeping rows (mode, queue operations). They made
 *  the store non-empty within seconds, the post-compaction watchdog read that as
 *  "the replay landed", and a replay that had died mid-flight was never asked
 *  for again. */
export function tabOwesHistory(tabId: string): boolean {
  if (!needsRefill.has(tabId)) return false
  const entries = jsonlEntriesByTab.value.get(tabId) ?? []
  return !entries.some((e) => e.type === 'user' || e.type === 'assistant')
}

/** Called once the tab has been topped up (or proven genuinely empty). */
export function clearRefillDebt(tabId: string): void {
  needsRefill.delete(tabId)
}

export function clearJsonlEntries(tabId: string): void {
  liveWindows.delete(tabId)
  archivedLiveEntries.delete(tabId)
  archivedStreams.delete(tabId)
  archivedViewByTab.delete(tabId)
  needsRefill.add(tabId)
  const nextMap = new Map(jsonlEntriesByTab.value)
  nextMap.delete(tabId)
  bumpStructureVersion(tabId) // wiped → invalidate the render-model cache
  dropRenderModelCache(tabId) // …and free its retained per-segment models
  jsonlEntriesByTab.value = nextMap
  const nextSeen = new Map(jsonlSeenUuidsByTab.value)
  nextSeen.delete(tabId)
  jsonlSeenUuidsByTab.value = nextSeen
  seenRowKeysByTab.delete(tabId)
  // Drop the input-history seed reference too — JsonlViewer stashes the full
  // entries array on `window.__jsonlEntries[tabId]` and it was NEVER deleted, so
  // a closed/compacted huge session's ~100MB object graph stayed reachable
  // forever, defeating GC and pushing the shared WebView2 renderer toward OOM.
  try {
    const store = (window as unknown as { __jsonlEntries?: Record<string, unknown> }).__jsonlEntries
    if (store) delete store[tabId]
  } catch { /* window unavailable */ }
  setReplayProgress(tabId, null) // a cleared/compacted tab's old bar is meaningless
}

export function getJsonlEntries(tabId: string): JsonlEntryData[] {
  return jsonlEntriesByTab.value.get(tabId) ?? []
}

/** Trim the store to its last `keep` entries — the windowed store's memory cap.
 *
 *  Called only when the reader is at the BOTTOM, so the entries dropped (the
 *  oldest) are far above the viewport and their removal is invisible. The whole
 *  point is that the active tab, which eviction never touches, no longer holds
 *  an entire marathon transcript in the renderer heap (the OOM) — the tail is
 *  kept, older pages come back from the disk mirror on scroll-up.
 *
 *  CRITICAL: the dedup sets are rebuilt from the kept rows. Otherwise the
 *  scroll-up prepend or the next replay, which dedup on seen, would silently drop
 *  the very records they bring back — the history would be gone with no way
 *  back. Rebuilding (instead of deleting the dropped uuids) also covers every
 *  uuid folded into a merged bubble, and costs one pass per trim, not per row. */
export function trimTabToWindow(tabId: string, keep: number): boolean {
  const existing = jsonlEntriesByTab.value.get(tabId)
  if (!existing || existing.length <= keep) return false
  const kept = existing.slice(existing.length - keep)
  reindexSeen(tabId, kept)
  const nextMap = new Map(jsonlEntriesByTab.value)
  nextMap.set(tabId, kept)
  bumpStructureVersion(tabId)
  jsonlEntriesByTab.value = nextMap
  return true
}

/** The OOM backstop, enforced in the signal layer on every append.
 *
 *  The reader-aware trim in JsonlViewer only ever runs for the ACTIVE tab — a
 *  mounted viewer, at the bottom. But `appendJsonlEntries` fires for EVERY tab:
 *  a background session mid-response keeps accumulating live entries with no
 *  viewer to trim it, so its store grew without bound. Two tabs like that (one
 *  active, one background) is exactly the reported crash — 10.6k entries across
 *  2 tabs, shared heap ~4GB, renderer dead with OUT_OF_MEMORY.
 *
 *  - Background tab: no reader, no viewport to disturb — trim straight to the
 *    window on every append. Switching to it later refills from the mirror.
 *  - Active tab: the viewer owns the reader-aware trim; here we only catch a
 *    genuine runaway past the scroll-up ceiling (live appends while parked far
 *    up), well above anything the viewer itself would hold. */
function enforceTabCap(tabId: string): void {
  const list = jsonlEntriesByTab.value.get(tabId)
  if (!list) return
  if (activeTabId.value === tabId) {
    if (list.length > SCROLL_UP_MAX + STORE_SLACK) trimTabToWindow(tabId, SCROLL_UP_MAX)
  } else if (list.length > STORE_WINDOW + STORE_SLACK) {
    trimTabToWindow(tabId, STORE_WINDOW)
  }
}

/** Replace the whole resident window with one archived compact segment loaded
 *  from the mirror (the "jump to an out-of-window conversation" path). Unlike
 *  prepend, this DROPS the live tail — the user asked to look at a different
 *  segment — so it resets the seen-sets to exactly these records. The mirror's
 *  stored `_ord` is from a different replay, so re-tag ascending for a stable
 *  order, and trim heavy tool bodies like the live ingest does. Returns the
 *  count stored. Pair with setArchivedView to gate live appends off. */
export function replaceWindowWithSegment(tabId: string, records: JsonlEntryData[]): number {
  const kept = records.filter(isStoredEntry)
  kept.forEach((e, i) => { (e as { _ord?: number })._ord = i; trimHeavyToolResults(e) })
  const ordered = orderEntries(foldAssistantEntries(orderEntries(kept)))

  const nextMap = new Map(jsonlEntriesByTab.value)
  nextMap.set(tabId, ordered)
  reindexSeen(tabId, ordered)
  dropRenderModelCache(tabId)
  bumpStructureVersion(tabId)
  jsonlEntriesByTab.value = nextMap
  return ordered.length
}

/** Reconcile an older mirror page through the live ingest path. A page can
 *  begin/end inside a split assistant message; raw concatenation would leave
 *  its prefix outside the canonical bubble. Rebase replay-local ordinals only
 *  for legacy rows without persistent file positions or timestamps. */
export function prependJsonlEntries(tabId: string, older: JsonlEntryData[]): boolean {
  if (older.length === 0) return false
  const existing = jsonlEntriesByTab.value.get(tabId) ?? []
  const seen = jsonlSeenUuidsByTab.value.get(tabId)
  // Dedup against the store's seen-set AND within the page itself: a page that
  // ever contained two lines with the same uuid (a duplicate write) must not
  // double the row — the outer `seen` check alone would pass both, as neither is
  // in `seen` at filter time.
  const inPage = new Set<string>()
  const fresh = older.filter((e) => {
    if (!e.uuid) return true
    if (seen?.has(e.uuid) || inPage.has(e.uuid)) return false
    inPage.add(e.uuid)
    return true
  })
  if (fresh.length === 0) return false

  // Offload fat tool_result bodies on THIS ingest path too — scroll-up loads
  // OLDER history back into the store, so without this a read/grep/bash-heavy
  // session re-grows the renderer toward the OOM the trim exists to prevent, just
  // via scroll-up instead of live append (matches appendJsonlEntries).
  for (const e of fresh) trimHeavyToolResults(e)

  // Rebase the legacy ordinal fallback below the current window.
  let minOrd = Number.MAX_SAFE_INTEGER
  for (const e of existing) { const o = (e as { _ord?: number })._ord; if (typeof o === 'number' && o < minOrd) minOrd = o }
  const base = (minOrd === Number.MAX_SAFE_INTEGER ? 0 : minOrd) - fresh.length
  fresh.forEach((e, i) => { (e as { _ord?: number })._ord = base + i })

  // Use the same reconciliation as live/replay ingestion. In particular, the
  // page boundary can cut through a split assistant message already in-store.
  return appendJsonlEntries(tabId, fresh)
}

// ── Last thinking preview for ActivityIndicator ─────────────────────────
// Returns the most recent `thinking` text from the active assistant stream.
export const activeThinkingPreview = computed<string>(() => {
  const id = activeTabId.value
  if (!id) return ''
  const entries = jsonlEntriesByTab.value.get(id) ?? []
  for (let i = entries.length - 1; i >= 0; i--) {
    const e = entries[i]
    if (!e || e.type !== 'assistant') continue
    const content = e.message?.content
    if (!Array.isArray(content)) continue
    for (let j = content.length - 1; j >= 0; j--) {
      const b: any = content[j]
      if (b && b.type === 'thinking' && typeof b.thinking === 'string' && b.thinking.trim()) {
        return b.thinking.trim()
      }
    }
    // Stop at first non-thinking assistant — older thoughts aren't current
    return ''
  }
  return ''
})

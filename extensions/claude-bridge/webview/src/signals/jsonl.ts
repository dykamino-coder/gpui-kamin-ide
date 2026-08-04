import { signal, computed } from '@preact/signals'
import { activeTabId } from './tabs'
import type { JsonlEntryData } from '../types/jsonl'
import { orderEntries } from './order-entries'
import { setReplayProgress } from './replay-progress'
import { dropRenderModelCache } from '../components/jsonl-viewer/derived-cache'
import { trimHeavyToolResults } from './heavy-tool-result'

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
export const jsonlSeenUuidsByTab = signal<Map<string, Set<string>>>(new Map())
// Per-tab set of replay `_ord`s (file-position tags the server puts on reverse-
// replay entries). The reverse loader + the extension's cache re-send can deliver
// the SAME entry twice; entries WITHOUT a uuid (system / tool rows) slip past the
// uuid dedup and — because we sort by `_ord` — would interleave as duplicates.
// Dedup on `_ord` too. Non-reactive: pure bookkeeping, reset in clearJsonlEntries.
const seenOrdsByTab = new Map<string, Set<number>>()

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
export function setArchivedView(tabId: string, ts: string | null): void {
  if (ts === null) archivedViewByTab.delete(tabId)
  else archivedViewByTab.set(tabId, ts)
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
  const msgId = entry.message?.id
  if (!msgId) return
  if (abortedStreamIds.has(msgId)) return // user interrupted this turn — freeze
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
  const stamped = { ...entry, __streamStartedAt: Date.now() }
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
  const content = Array.isArray(stub.message?.content) ? stub.message!.content : []
  const block: any = content[d.blockIdx]
  // INVARIANT (load-bearing): a delta only ever appends text to an ALREADY-
  // EXISTING block — it never creates, removes, or reorders one. This is what
  // lets scheduleStreamFlush stay TEXT-ONLY (no structureVersion bump): the
  // render-model cache reuses the whole segment and only re-points stub objects.
  // Materializing a missing block here would silently change structure with no
  // bump → stale-cache frozen chat. New blocks arrive via applyStreamingEntry
  // (a snapshot, which DOES bump). So: bail if the block isn't there yet.
  if (!block) return
  if (d.kind === 'text' && block.type === 'text') block.text = (block.text ?? '') + d.appendText
  else if (d.kind === 'thinking' && block.type === 'thinking') block.thinking = (block.thinking ?? '') + d.appendText
  else return
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
  const msgId = entries[i]!.message?.id
  for (let j = i + 1; j < entries.length; j++) {
    const e = entries[j]!
    if (e.type === 'user') return true
    if (e.type === 'assistant' && e.__streaming === undefined && e.message?.id && e.message.id !== msgId) return true
  }
  return false
}

function sweepOrphanStubs(): void {
  const now = Date.now()
  let changed = false
  const nextMap = new Map(jsonlEntriesByTab.value)
  let anyStubsLeft = false
  for (const [tabId, entries] of nextMap) {
    const kept = entries.filter((e, i) => {
      // A canonical entry has __streaming === undefined (stripped on merge);
      // only stubs carry a defined flag + a start stamp.
      if (e.__streaming === undefined) return true
      const started = e.__streamStartedAt
      if (typeof started !== 'number') return true
      if (now - started <= STUB_TTL_MS) { anyStubsLeft = true; return true }
      // TTL expired. Only sweep a stub that a later turn has SUPERSEDED — that is
      // a genuine orphan (retry / interrupt). A tail stub that still carries
      // content is kept: its canonical merges when it finally arrives, and until
      // then a shown answer beats a vanished one. Keep the sweeper alive
      // (anyStubsLeft) so the moment a successor appears the orphan is swept.
      if (stubHasContent(e) && !stubSuperseded(entries, i)) { anyStubsLeft = true; return true }
      // #78 telemetry: record WHY a streamed bubble was dropped so the next
      // "streamed then vanished" report is root-caused from the bug-dump instead
      // of showing an empty drop log.
      logDrop(stubHasContent(e) ? 'orphan-stub-superseded' : 'orphan-stub-empty', e)
      return false
    })
    if (kept.length !== entries.length) { nextMap.set(tabId, kept); changed = true; bumpStructureVersion(tabId) }
  }
  if (changed) jsonlEntriesByTab.value = nextMap
  // Stop the timer once no stubs remain — restarts on the next stream.
  if (!anyStubsLeft && stubSweeper) { clearInterval(stubSweeper); stubSweeper = null }
}

/** Merge a watcher-delivered assistant entry into a previously-stored one
 *  that carries the same `message.id`. CLI v2.1.x splits a single model
 *  turn into separate JSONL lines per content block (thinking → text →
 *  tool_use), each with a fresh uuid but the shared message.id. The
 *  Anthropic API model treats them as ONE message with multiple blocks,
 *  and the streaming proxy renders them as one bubble — we want the same
 *  view after the watcher catches up. Without merging, the previous code
 *  pushed all three watcher entries into `replacements[]` at the same
 *  streaming-stub index and last-write-wins clobbered text + thinking,
 *  leaving only the tool_use bubble visible (the bug fixed here). */
function mergeAssistantEntry(target: JsonlEntryData, incoming: JsonlEntryData): JsonlEntryData {
  const targetContent = Array.isArray(target.message?.content) ? target.message!.content : []
  const incomingContent = Array.isArray(incoming.message?.content) ? incoming.message!.content : []
  // First real watcher entry replacing a streaming stub: drop the
  // proxy-synthesized blocks (they were partial guesses) and adopt the
  // canonical CLI-persisted ones. Subsequent in-batch merges concat to
  // the already-canonical content.
  const wasStreaming = target.__streaming !== undefined
  const baseContent = wasStreaming ? [] : targetContent
  // Always adopt the incoming (latest) uuid. Subsequent CLI entries point
  // their `parentUuid` at the LAST piece of the model turn (tool_use or
  // trailing text), so keeping the first piece's uuid would orphan the
  // chain walk in `computeChainUuids` and `entryIsVisible` would drop the
  // merged bubble entirely. The synthetic `stream-<msgId>` uuid was
  // ephemeral anyway. ...incoming spread already does this, but make it
  // explicit so the reasoning isn't buried in object-spread order.
  // Carry EVERY folded uuid forward, so trim can forget all of them from `seen`.
  // `.uuid` below is only the LAST one; the earlier blocks' uuids are already in
  // `seen` and would otherwise orphan there and drop those blocks on a re-replay.
  const uuids = new Set<string>(target.__uuids ?? (target.uuid ? [target.uuid] : []))
  if (incoming.uuid) uuids.add(incoming.uuid)
  return {
    ...target,
    ...incoming,
    message: {
      ...target.message,
      ...incoming.message,
      content: baseContent.concat(incomingContent),
    },
    __streaming: undefined,
    __uuids: [...uuids],
  } as JsonlEntryData
}

/** Types the chat stores but nothing ever reads.
 *
 *  Measured on a real 30 971-record session: 3569 records, 11.5% of the store —
 *  though only 0.6MB of 85.2MB, because the weight lives in tool results, not in
 *  bookkeeping rows. The win is not memory, it is that every pass over the store
 *  (merge, group, chain walk, visibility) has 11.5% less to walk.
 *
 *  Deliberately NOT here: `mode` / `permission-mode` feed the permissions
 *  dropdown, and `queue-operation` reconstructs the pending queue. They render
 *  nothing but are read, and dropping them would have broken both silently. */
const UNREAD_ENTRY_TYPES: ReadonlySet<string> = new Set([
  'last-prompt', 'ai-title', 'custom-title', 'agent-name',
  'summary', 'todo-snapshot', 'compact-summary', 'tool-call-error',
  'file-history-snapshot',
])

export function appendJsonlEntries(tabId: string, entries: JsonlEntryData[]): boolean {
  if (entries.length === 0) return false
  // Drop the unread rows before they ever enter the store. They used to reach
  // the renderer and be filtered at the last moment, so each one still took a
  // slot, a vnode and a place in every derived model.
  // Viewing an archived compact segment: live entries belong to the CURRENT
  // segment, not this snapshot — dropping them here keeps the archived view
  // intact (they persist server-side and return when the user goes back to
  // Current). See archivedViewByTab.
  if (archivedViewByTab.has(tabId)) return false
  entries = entries.filter((e) => !UNREAD_ENTRY_TYPES.has(e.type))
  if (entries.length === 0) return false
  // Offload the bytes of fat tool_result bodies before they enter the store —
  // the UI never shows past 2000 chars of one, and holding whole multi-hundred-KB
  // results is what drove the renderer to OUT_OF_MEMORY. Image payloads untouched.
  for (const e of entries) trimHeavyToolResults(e)

  let seen = jsonlSeenUuidsByTab.value.get(tabId)
  let seenIsNew = false
  if (!seen) { seen = new Set<string>(); seenIsNew = true }
  let ordSeen = seenOrdsByTab.get(tabId)
  if (!ordSeen) { ordSeen = new Set<number>(); seenOrdsByTab.set(tabId, ordSeen) }
  const existing = jsonlEntriesByTab.value.get(tabId) ?? []

  // Trust JSONL file order — that's the canonical source of truth. CLI
  // sometimes back-dates timestamps on synthetic attachments (e.g. an
  // edited_text_file attach gets ts(parent_user) - 1ms), and a naive
  // ts-based sort then floats the attachment ABOVE the parent user
  // message in the chat. Watcher always reads JSONL files linearly so
  // batches arrive pre-ordered; trust that and don't second-guess.
  const ordered = entries

  // Walk the batch building a NEW list lazily. Track per-batch
  // `message.id → index` so multiple watcher entries with the same id
  // (CLI's per-content-block split) merge into one entry instead of
  // racing each other into the same `replacements[]` slot.
  let mutated = false
  let nextEntries: JsonlEntryData[] = existing
  const msgIdToIdx = new Map<string, number>()
  for (const e of ordered) {
    const ord = (e as { _ord?: number })._ord
    // uuid dedup FIRST: `_ord` values are fresh on every server-side replay
    // (monotonic per-process counter — restarts/reattaches re-tag the same
    // entries with new ords), so uuid is the only stable cross-replay key.
    // The ord-dedup only covers uuid-less rows within one replay stream.
    if (e.uuid) {
      if (seen.has(e.uuid)) { logDrop('uuid-dup', e); continue }
      seen.add(e.uuid)
    } else if (typeof ord === 'number') {
      if (ordSeen.has(ord)) { logDrop('ord-dup', e); continue }
      ordSeen.add(ord)
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
        nextEntries[targetIdx] = mergeAssistantEntry(nextEntries[targetIdx]!, e)
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

  // Order the merged array: a STABLE topological sort (order-entries.ts) —
  // `_ord` primary key (correct for reverse-emitted replay segments + compaction
  // boundaries) tie-broken by arrival, with the parentUuid→uuid chain used only
  // to keep a child after its parent. This fixes the two live-append drifts (a
  // streaming stub sitting above its own prompt; live entries shoved below newer
  // `_ord` entries after a reattach) WITHOUT the per-token full re-sort that
  // froze large sessions — appendJsonlEntries runs on batch delivery only (a few
  // times/sec), never on streaming deltas.
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

/** Called once the tab has been topped up (or proven genuinely empty). */
export function clearRefillDebt(tabId: string): void {
  needsRefill.delete(tabId)
}

export function clearJsonlEntries(tabId: string): void {
  needsRefill.add(tabId)
  const nextMap = new Map(jsonlEntriesByTab.value)
  nextMap.delete(tabId)
  bumpStructureVersion(tabId) // wiped → invalidate the render-model cache
  dropRenderModelCache(tabId) // …and free its retained per-segment models
  jsonlEntriesByTab.value = nextMap
  const nextSeen = new Map(jsonlSeenUuidsByTab.value)
  nextSeen.delete(tabId)
  jsonlSeenUuidsByTab.value = nextSeen
  seenOrdsByTab.delete(tabId)
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
 *  CRITICAL: the evicted uuids are removed from the seen-set. Otherwise the
 *  scroll-up prepend, which dedups on seen, would silently drop the very records
 *  it just refetched — the history would be gone with no way back. */
export function trimTabToWindow(tabId: string, keep: number): boolean {
  const existing = jsonlEntriesByTab.value.get(tabId)
  if (!existing || existing.length <= keep) return false
  const drop = existing.length - keep
  const dropped = existing.slice(0, drop)
  const kept = existing.slice(drop)

  const seen = jsonlSeenUuidsByTab.value.get(tabId)
  const ordSeen = seenOrdsByTab.get(tabId)
  for (const e of dropped) {
    // Forget EVERY uuid folded into this entry (a merged assistant turn holds N,
    // in `__uuids`; `.uuid` alone leaves the earlier blocks' ids in `seen`, which
    // then drop those blocks as duplicates on a re-replay → the middle-of-chat
    // gap). Falls back to `.uuid` for un-merged entries.
    const uuids = e.__uuids ?? (e.uuid ? [e.uuid] : [])
    if (uuids.length > 0) { for (const u of uuids) seen?.delete(u) }
    // Symmetric with the uuid set — uuid-less rows are ord-deduped; leaving their
    // ords here would grow the set for the life of the tab across trim cycles.
    else { const o = (e as { _ord?: number })._ord; if (typeof o === 'number') ordSeen?.delete(o) }
  }
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
  const kept = records.filter((e) => !UNREAD_ENTRY_TYPES.has(e.type))
  kept.forEach((e, i) => { (e as { _ord?: number })._ord = i; trimHeavyToolResults(e) })
  const ordered = orderEntries(kept)
  const seen = new Set<string>()
  for (const e of ordered) { if (e.uuid) seen.add(e.uuid) }

  const nextMap = new Map(jsonlEntriesByTab.value)
  nextMap.set(tabId, ordered)
  const nextSeen = new Map(jsonlSeenUuidsByTab.value)
  nextSeen.set(tabId, seen)
  seenOrdsByTab.set(tabId, new Set())
  dropRenderModelCache(tabId)
  bumpStructureVersion(tabId)
  jsonlEntriesByTab.value = nextMap
  jsonlSeenUuidsByTab.value = nextSeen
  return ordered.length
}

/** Prepend an OLDER page (from the scroll-up loader) to the front of the store.
 *
 *  The page comes from the mirror keyed on `_pos` — every record in it is
 *  strictly older than the window's current oldest, and it is already in file
 *  order. So it belongs at the front as-is; no full re-sort.
 *
 *  BUT the store's ordering key is `_ord`, and the mirror's stored `_ord` is from
 *  the session that wrote it — inconsistent with the window's fresh `_ord` from
 *  this session's replay. Left alone, the next append's `orderEntries` would
 *  interleave the page into the middle. So the page is RE-TAGGED with `_ord`
 *  values placed just below the window's minimum: legitimate (`_ord` is only ever
 *  a sort key, re-tagged on every replay) and it keeps the array valid for every
 *  later sort. Dedup on uuid so a page overlapping the window can't double a row. */
export function prependJsonlEntries(tabId: string, older: JsonlEntryData[]): boolean {
  if (older.length === 0) return false
  const existing = jsonlEntriesByTab.value.get(tabId) ?? []
  let seen = jsonlSeenUuidsByTab.value.get(tabId)
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

  // Place the page's `_ord` just below the window's current minimum, ascending,
  // so the whole store stays `_ord`-ordered and orderEntries' fast path holds.
  let minOrd = Number.MAX_SAFE_INTEGER
  for (const e of existing) { const o = (e as { _ord?: number })._ord; if (typeof o === 'number' && o < minOrd) minOrd = o }
  const base = (minOrd === Number.MAX_SAFE_INTEGER ? 0 : minOrd) - fresh.length
  fresh.forEach((e, i) => { (e as { _ord?: number })._ord = base + i })

  if (!seen) {
    seen = new Set<string>()
    const nextSeen = new Map(jsonlSeenUuidsByTab.value)
    nextSeen.set(tabId, seen)
    jsonlSeenUuidsByTab.value = nextSeen
  }
  for (const e of fresh) if (e.uuid) seen.add(e.uuid)

  const nextMap = new Map(jsonlEntriesByTab.value)
  nextMap.set(tabId, fresh.concat(existing))
  bumpStructureVersion(tabId) // the segment/render model must recompute over the grown range
  jsonlEntriesByTab.value = nextMap
  return true
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

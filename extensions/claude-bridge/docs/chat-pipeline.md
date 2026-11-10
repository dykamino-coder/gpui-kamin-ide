# Claude Bridge chat pipeline: from CLI JSONL to rendered rows

Status: audit of `origin/main` at 2026-11-10 (after #679). Tests:
`webview/src/signals/chat-render-matrix.test.ts`,
`chat-render-property.test.ts`, `chat-streaming-audit.test.ts` (fixtures in
`webview/src/signals/chat-audit/`), host: `extension/src/shared/jsonl-cache-order.test.ts`.
Structural proposal: [adr-chat-transcript-projection.md](adr-chat-transcript-projection.md).

All fixtures are synthetic. No text, id or timing comes from a real session.

## 1. Stages and the state each one owns

| # | Stage | Code | State it owns |
|---|---|---|---|
| 1 | CLI writes the session JSONL | Claude Code CLI | the canonical transcript (file) |
| 2 | Server watcher / pages / replay | `server/src/core/pty/jsonl-watcher.ts`, `jsonl-page.ts`, `jsonl-replay.ts` | consumed byte offset, segment index |
| 3 | Host cache + mirror | `extension/src/main/ws/connection-manager.ts`, `handle-server-message.ts`, `transcript-mirror.ts`, `jsonl-batcher.ts` | `cachedJsonlEntries` (cap 20 000), `cachedStreamingEntry`, disk mirror + cursor |
| 4 | Webview listeners | `webview/src/hooks/useBridgeListeners.ts` | per-tab live flags, compaction clear |
| 5 | Store | `webview/src/signals/jsonl.ts` | `jsonlEntriesByTab`, seen-uuid set, seen-ord set, structure version, refill debt, archived gate |
| 6 | Render model | `components/jsonl-viewer/derived-cache.ts`, `utils.ts`, `compute-chain-uuids.ts`, `prepare-jsonl-entries.ts` | per (tab, segment, structure version) memo |
| 7 | Drawable window + viewer | `render-window.ts`, `JsonlViewer.tsx`, `useChatScrollPin.ts` | `renderCap`, page cursor, `reachedStart`, vnode cache |

Every stage after (1) keeps its own copy or index of the transcript. Each
historical defect of the "message missing / not drawn / empty chat with a
spinner" class is one of these copies diverging from (1) (§10).

## 2. Ingestion

- **Paged resume (production path).** On session start or reattach the client
  sends `history:{mode:'paged'}` with the mirror cursor. A valid cursor (file
  fingerprint, boundary, uuid match, ≤512 KB growth) makes the server stream
  forward from it; otherwise it sends one tail page (≤200 records, ≤512 KB) and
  `compacted:true` if the cursor was rejected. Then `replayComplete` and tailing.
- **Legacy reverse replay** (clients without `history`): newest batch first
  within the current segment, then older segments; ≤200 records / ≤1 MiB per
  batch; `replayProgress`; `replayComplete` last.
- **Record tags.** `_pos`/`_posEnd` = exact byte offsets, stable while the file
  is not rewritten. `_ord`: `= _pos` on pages, a process-global counter on legacy
  replay (changes on every replay and restart). **Live-tail records carry no
  tag at all** (`jsonl-watcher.ts`, "Live records stay untagged").
- **Host cache.** Every batch goes into `cachedJsonlEntries` (dedup key
  `_pos ?? uuid`; rows with neither are always appended), capped by
  `trimJsonlCache`. `requestJsonlReplay` from the webview never reaches the
  server: the host re-sends its newest 4 500 cached rows in 150-row IPC chunks,
  then status, tree and the cached streaming stub.
- **Older pages.** `loadOlderPage(beforePos)` → server `readHistoryPage`
  (≤200 records before `beforePos`; `nextPos` = oldest returned `_pos`); offline
  it reads the mirror. The mirror only receives tagged rows, so it lags the
  cache by the whole live session.

## 3. Dedup and ordering (store)

- `appendJsonlEntries` drops `UNREAD_ENTRY_TYPES`, trims heavy tool results,
  then dedups: by uuid if present, else by `_ord`. Assistant rows sharing a
  `message.id` with a stored row fold into it (`mergeAssistantEntry`, source
  rows kept in `__assistantParts`, all uuids in `__uuids`).
- `orderEntries` sorts the whole store after every structural append: by `_pos`
  when every canonical row has one, else by timestamp (fallback `_ord`, then
  last); parent-before-child edges override; stable on ties. Streaming stubs have
  neither key and always sort **last**.
- `trimTabToWindow` drops the oldest rows and forgets their uuids/ords so a
  re-delivery is accepted again.

## 4. Merges and streaming

- CLI 2.1.x writes one row per content block, same `message.id`, chained by
  `parentUuid`. The store folds them in source order; the render model folds
  again per segment (`mergeAssistantEntriesByMessageId`) and splits prose from
  tool calls into a bubble plus a tool group.
- The MITM proxy sends `streaming:entry` snapshots (`uuid: stream-<msgId>`,
  `__streaming`) and text deltas appended in place (coalesced to one signal write
  per frame). Canonical wins: once a canonical row for the id is stored, later
  stub snapshots and deltas are ignored. A canonical row merging into a stub
  replaces the stub's blocks with the canonical blocks, except while those carry
  no text yet (the thinking row comes first): then the rows are held under the
  stub (`__assistantParts`) and the streamed prose stays until the first
  canonical text takes over (D5).
- Interrupt (`abortStreamingForTab`) freezes the in-flight ids; the committed
  partial plus the interrupt marker arrive canonically.
- Orphan stubs: a 20 s TTL sweep removes a stub only when a later turn
  supersedes it (`stubSuperseded`: a turn row after it, or one with a later CLI
  timestamp than the store's newest when the stub appeared, since stubs sort
  last) or it is empty. Rows held under a swept stub become a canonical entry.
- The render-model cache is keyed on a structure version that text deltas do not
  bump; stub objects are re-pointed in O(tail).

## 5. Sidechains, meta, bookkeeping, attachments, queue

- `isSidechain` rows (canonical or stub) never show in the main chat; subagent
  transcripts travel separately (`jsonl:subagent-entries`, own cache, cap 5 000).
- `isMeta` user rows, caveat-only rows, tool-result-only rows whose results
  merged into the tool group, interrupt/skip markers (turned into badges), and
  synthetic texts ("No response requested.") do not render.
- `NON_RENDERING_ENTRY_TYPES` (mode, permission-mode, queue-operation, …) and any
  type outside `RENDERABLE_ENTRY_TYPES` (cost-state, api-request*, …) are hidden
  were stored and counted by the store cap until #698; since then the store keeps only `CHAT_STORED_ENTRY_TYPES` (`extension/src/shared/chat-stored-entries.ts`).
- Attachments render only for `queued_command` with a prompt and
  `edited_text_file`. `queue-operation` rows rebuild the pending queue.
- Pending user bubbles come from submission receipts (`signals/submissions.ts`,
  host copy in `workspace/src/presentation.ts`) and normally disappear by the
  stored UUID of the read receipt. A receipt the server can no longer settle
  stays `uncertain` and is settled from the transcript by the shared
  `matchReceiptsToTranscript`: one row per receipt, never a row older than the
  receipt (store watermark; server `createdAt` against the CLI `timestamp`,
  same clock). On reattach the server lists the ids it still tracks
  (`known`); pending receipts missing from it (server or CLI restart) turn
  `uncertain` instead of a Queued bubble whose Cancel reaches nothing.

## 6. Compaction

- `compact_boundary` (parentUuid null, `logicalParentUuid`) starts a segment.
  `compactSegments` splits the resident store at boundaries; the latest segment
  is shown by default; older resident segments are selectable.
- An out-of-window segment is loaded from the mirror by `replaceWindowWithSegment`
  with `setArchivedView(tab, ts)`. While archived, live rows and stream snapshots
  are buffered (capped) and replayed into the live window on return to Current.
- `compacted` status clears the store (`clearJsonlEntries`) and owes a refill.
- The CLI's own summary request (manual `/compact` and auto-compact) is
  recognised by the server MITM at its system prompt
  (`server/src/core/proxy/compaction-request.ts`). Clients get only a
  contentless `__compaction` marker. The chat shows "Compacting
  conversation…", as the CLI TUI does, never the `<analysis>` summary text
  as an answer, and no stub is left behind. The final snapshot closes the
  indicator; if that snapshot is lost, the compact boundary or summary row
  closes it (`signals/compaction.ts`; the host stub cache also drops the marker).

## 7. Window, cap, eviction, refill, virtualized view

- `STORE_WINDOW` 4 000 (+`STORE_SLACK` 500): the active tab trims at the bottom,
  background tabs on every append; `SCROLL_UP_MAX` 16 000 is the active-tab
  ceiling. Eviction (`jsonl-eviction.ts`) clears whole background tabs over a
  60 000-row budget, keeping the 3 most recent.
- A cleared tab owes a refill (`needsRefill`) until `replayComplete`.
- The viewer mounts the last 150 **drawable** rows (`selectDrawableWindow`),
  grows by 400 on scroll-up, then pages older rows from the server by
  `store[0]._pos`, and stops (`reachedStart`) when that row has no `_pos`.

## 8. Invariants

- **I1 exactly once.** Every user/assistant message the CLI shows appears in
  the chat's projection at most once at any time, and exactly once after the
  owed refill. Tested by the property test under random chunking, duplicate
  and out-of-order delivery, live/paged mixing, trims and clears.
- **I2 CLI order.** The projection lists messages in CLI order.
- **I3 suffix.** Over the cap, the resident projection is a contiguous suffix
  ending at the CLI's last message, and the mounted window ends there too.
- **I4 recoverable.** Rows above the window can be paged back (scroll-up) or
  replayed (refill) until the projection equals the CLI view.
- **I5 stream once.** A live turn is on screen once, after the last canonical
  message; the hand-off to the canonical rows happens once, and the final text
  is byte-equal to the transcript.
- **I6 canonical wins.** No late stub snapshot or delta changes a canonical row.
- **I7 archived isolation.** Live rows never enter an archived snapshot and are
  not lost while it is open.

## 9. CLI reference behaviour and comparison

Reference: the differential run below used Claude Code **2.1.177**; the image
now pins **2.1.322** (audit in `server/CLAUDE_CODE_COMPATIBILITY.md`), and the
run has not been repeated on it. Sources: code.claude.com
`sessions` and Agent SDK `sessions`/`getSessionMessages` docs; the
`anthropics/claude-code` CHANGELOG; and a differential run. The run used an
isolated `CLAUDE_CONFIG_DIR`, a local intercepting endpoint and a dummy key: no
credentials, no network, no cost. It ran `claude --bare --resume <id> -p`
over synthetic transcripts and captured the conversation the loader rebuilds
and sends to the model.

**Limitation.** The terminal rendering itself was not captured: the
interactive TUI in a clean config stops at onboarding, and a Bridge console
capture needs an authenticated test container, which was not used. The loader
result is strong evidence for what resume shows. It is not a screenshot
comparison.

Observed or documented CLI behaviour:

- One branch: the leaf is `last-prompt.leafUuid`, or else the last main-chain
  line **in file order** (not timestamp). The CLI walks `parentUuid` back and
  drops everything off that branch. Sidechain lines never become the leaf.
- Resume starts at the last compact boundary. The walk does not follow
  `logicalParentUuid`, and pre-compact history is not loaded (CHANGELOG 2.1.301).
- Rows of one `message.id` merge into one assistant message.
- `isMeta` and command rows merge into the neighbouring user turn.
  `system/informational` and the metadata types (`summary`, `custom-title`,
  `last-prompt`, `queue-operation`, `file-history-snapshot`) are not part of
  the chain.
- For a duplicated uuid, the **last** copy wins. Line order does not matter
  (child-before-parent works).
- Assistant text is not truncated. Tool output collapses (ctrl+o expands it).

| Topic | CLI | Bridge chat | Verdict |
|---|---|---|---|
| Branch selection | single branch from the leaf | union of every leaf minus rewound-away prompt branches (`computeChainUuids`) | match for rewinds (D4 fixed); a non-prompt fork (hook attachment) still shows both sides, deliberately |
| Compaction | post-boundary only; earlier history not reachable after resume | current segment by default; earlier segments selectable / loadable | intended, better |
| Split blocks | one message | one message (store + render fold) | match |
| Sidechains | hidden from main view | hidden | match |
| Meta, synthetic texts, bookkeeping | hidden | hidden | match |
| Interrupt marker | marker line | badge on the interrupted prompt | intended |
| Duplicate uuid | last copy wins | first copy wins (seen set) | intended; differs only if a rewritten copy changes content |
| Out-of-order lines | chain rebuild | `_pos`/timestamp order plus parent edges | match (tested) |
| Long assistant text | full | full (200 KB tested); tool bodies capped at 2 000 chars | match / intended |
| History size | full branch, virtual scroller | windowed store + 150-row window + pages | intended, but the window must satisfy I3/I4 (D3, D7 break it) |
| Streaming | the TUI renders one stream, then the committed message | stub + canonical rows from two channels, merged by `message.id` | design difference; source of D5/D6, see the ADR |

## 10. History of the defect class

The defect class: a message goes missing, new messages are not drawn, or the
chat is empty while a scroll-up "loading" never fills it. Sources: commit and
PR history, INC cards, code comments, and `CLIENT_AUDIT_2026_07_31.md`.

| Desync | Count | Examples |
|---|---|---|
| Dedup memory (seen uuid/ord) vs store | 4 | trim left merged uuids in `seen`; replay `_ord` reset; stale stub plus burned uuids; in-page duplicates |
| Lifecycle flags (refill debt, replay-complete, cached status, generation) | 8 | owed refill skipped once non-empty; completion lost on tab switch; cached `compacted` wipe loop; status overtaking data; #377; Agents #36/#192 |
| Order/fold vs source order | 4 | host cache trimmed by arrival order; #679 (unpositioned rows behind the tail, fold in arrival order); #331 |
| Cursor/offset vs source | 5 | #378 shrink; #536 mirror offsets; audit C9/C10; BR-16 anchor |
| Cap/window counting the wrong rows | 3 | #536 newest evicted; #388 (INC-2026-0071) tip and empty wrappers |
| Visibility/grouping predicate vs renderer | 4 | #388, #667, #331, chain walk (#78 dumps) |
| Streaming stub vs canonical | 3 | stub wiped canonical; mid-turn TTL sweep (#73); INC-2026-0088 |
| Derived/vnode cache staleness | 3 | #679 vnode suffix; #388; audit D8 |
| Conversation identity | 2 | #387; audit C1 |
| Viewport only (data intact) | 2 | #674; BR-16 |

Every fix except #536, #388 and the Agents generation staging was a point fix
to one copy's bookkeeping.

## 11. Defects found by this audit

| ID | Defect | Where | Status |
|---|---|---|---|
| D1 | `trimJsonlCache` ranked untagged live rows as 0; past the 21 000 cap the cutoff 0 dropped **every** live row at once (a test reproduced 21 700 → 199 rows), so the next renderer replay showed only the first page | host `handle-server-message.ts` | **fixed here** (`shared/jsonl-cache-order.ts`) |
| D2 | the renderer replay took the newest 4 500 by `_pos ?? _ord ?? 0`, putting the tagged first page in front and cutting the middle of a live session; scroll-up then pages before the first page, so the gap never fills | host `shared/chat-stored-entries.ts` (`chatReplayTail`), `connection-manager.ts` (view snapshot) | **fixed here** (`newestInFileOrder`) |
| D3 | uuid-less bookkeeping (cost-state, api-request, …) is stored and counted by the store cap; a flood after the last message evicts every real row → empty chat while the console shows everything | webview `jsonl.ts`, host replay tail | fixed by #698; regression test now a plain `it` |
| D4 | multi-leaf chain union keeps a rewound-away branch visible; the CLI shows one branch | `compute-chain-uuids.ts` (`abandonedBranches`) | fixed: the subtree of every prompt whose sibling prompt is on the newest leaf's chain is hidden; non-prompt forks keep the multi-tip union |
| D5 | the thinking-only canonical row replaces the stub's blocks, so streamed prose disappears until the text row lands | `jsonl.ts` (`holdStreamedProse`) | fixed: canonical rows without text are held under the stub until the first canonical text takes over |
| D6 | stubs always sort last, but `stubSuperseded` looks for a successor after the stub, so an aborted API attempt (new `message.id` on retry) is never swept and stays under the retry's answer | `jsonl.ts` (`stubSuperseded`) | fixed: a successor is a later CLI timestamp than the store's newest when the stub appeared |
| D7 | live rows carry no `_pos`; after a bottom trim of a live session the oldest resident row has no page cursor, so scroll-up stops (`reachedStart`) with history above unreachable; the mirror also never gets live rows | server `jsonl-watcher.ts` (tail contract) | expected-fail; open (tag live rows with `_pos`/`_posEnd`, ADR step 1) |
| D8 | the host cache can hold the same record twice (untagged live copy keyed by uuid, tagged page copy keyed by `_pos`); the webview dedups by uuid, but the cache cap counts both | host `connection-manager.ts` | open, low |
| D9 | a pending receipt never settles when the read receipt is lost: the server forgets ids on restart (the client keeps a Queued bubble, Cancel does nothing), `uncertain` waits for a UUID that never comes, a stopped session sends no receipt at all | `submissions.ts`, `presentation.ts`, `cli-submissions.ts`, `session-io.ts` | fixed: reattach `known` snapshot, `createdAt`, transcript settling of `uncertain` receipts, stopped-session receipt |
| D10 | the CLI's compaction request streamed into the chat as an assistant answer (the `<analysis>` summary text), and its stub had no canonical twin, so it stayed as a ghost until the TTL sweep; the CLI TUI shows only "Compacting conversation…" | server `native-mitm.ts`, webview `jsonl.ts` | fixed: the request is recognised by its system prompt; clients get a contentless `__compaction` marker that drives an indicator |

## 12. Test matrix

| Case | Test |
|---|---|
| one replay / live chunks / legacy reverse / out-of-order / duplicate batches / reattach with fresh ords | matrix › delivery order and duplication |
| clear + post-clear echo + owed refill; trim + reconnect replay | matrix › reload, eviction and refill |
| over the cap (background), bottom trim + scroll-up pages, D7, D3 flood | matrix › long sessions |
| compaction current, archived view with live rows | matrix › compaction |
| split blocks out of order, parallel tools, sidechains, interrupt, queue, meta + bookkeeping, 200 KB message, D4 rewind | matrix › turn shapes |
| stream + hand-off byte-equal, D5, late stub, 2 000-delta answer, interrupt, reconnect, reload, parallel subagents, compaction mid-stream, D6 retry | streaming audit |
| I1/I2 under random schedules (60 seeds) | property |
| D1/D2 | host `shared/jsonl-cache-order.test.ts` |
| D9 | `signals/submissions.test.ts` › stale pending receipts; workspace `presentation.test.ts`, `submission-state.test.ts`; server `cli-submissions.test.ts`, `session-cli-control.test.ts` |
| D10 | server `native-mitm.test.ts` (compaction marker); `chat-streaming-audit.test.ts` › compaction request stream; host `streaming-cache.test.ts`; workspace `compaction-stream.smoke.mjs` (IDE chat.html and browser) |

`it.fails` cases are known defects. When a fix lands the `it.fails` case turns
red (it unexpectedly passes); the fixing PR flips it to `it`.

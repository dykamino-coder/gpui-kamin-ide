# ADR: the chat is a projection of a host-owned, indexed transcript

Status: **proposed** (2026-11-10). Implementation is not part of the PR that adds
this record; the decision to proceed is the owner's.
Evidence: [chat-pipeline.md](chat-pipeline.md) §10 (history), §11 (defects),
§9 (CLI comparison) and the tests named there.

## Context

The chat keeps losing messages, failing to draw new ones, or showing an empty
screen with a scroll-up "loading" that never fills it. Each instance was fixed
on its own, and the class keeps returning. The history (chat-pipeline.md §10)
shows one cause behind about 38 cases: **five components each keep their own
copy or index of the transcript, and one of them drifts from the file.**

The five components are:
- the server's byte cursor;
- the host cache and mirror;
- the webview store, with its seen-uuid set, seen-ord set, refill debt,
  replay-complete flags and archived buffer;
- the render-model and vnode caches;
- the viewer's window, page cursor and `reachedStart`.

Rows are identified by four different keys, depending on the path and the era:
uuid, a replay-local `_ord`, `_pos`, or no key at all (live rows). Each bugfix
patched the bookkeeping of one copy.

The CLI does not have this class. It is one process: it reads the file into a
map by uuid, selects one leaf, walks `parentUuid`, and renders the chain. The
stream it draws is its own API stream, and it commits that stream into the same
in-memory message. There is a single copy, so nothing can diverge. The Bridge
cannot be a single process: the CLI runs on the server, the file is read on
the server, and the view runs in a webview with a memory ceiling. So it needs
to get the same property without a single copy.

## Decision

1. **The host owns one ordered display transcript per conversation.** The host
   is the extension host, fed by the server, and builds it from the CLI file. It
   applies the CLI's rules once, in one place:
   - the leaf is `last-prompt.leafUuid`, otherwise the last main-chain line;
   - the chain is walked back to the last compact boundary;
   - split rows are folded per `message.id`;
   - sidechains, `isMeta` rows and bookkeeping are classified.

   Each displayable item gets a **dense display index** within a
   **generation**. A file rewrite, a compaction file switch or a branch switch
   (rewind) starts a new generation. Items carry `(index, rev)`: the revision
   rises when a later split row of the same message folds in.

2. **The client is a pure range cache.** It asks for `[from, to)` of
   generation G and renders the items it gets, in index order. Dedup is index
   equality. Order is index order. A trim is "drop a range". A refill is
   "request the missing range". The start of history is `from == 0`. The host
   reports `total` with every response. These client structures disappear:
   - the seen-uuid and seen-ord sets;
   - refill debt;
   - `replayDoneTabs`;
   - the archived live buffer (an archived segment is just another range);
   - the `_pos` page cursor;
   - client-side chain walks and folds.

3. **Windows are counted in display items, not raw records.** Bookkeeping never
   reaches the chat, so neither the store cap nor the 150-row window can fill
   up with rows that draw nothing. This closes the cap and visibility
   categories, which the index alone would not (D3, INC-2026-0071).

4. **Empty screen ⇒ fetch, not wait.** If the window holds no items and
   `total > 0`, the client requests the last N items immediately. Scroll-up only
   ever extends a non-empty window. It cannot spin over an empty screen,
   because "load more" is a range request with a known `from`, not the side
   effect of a scroll event.

### The live turn overlay (streaming)

The live assistant turn is drawn from proxy deltas **before** the CLI writes it.
In this model it is a **provisional overlay anchored at `highWater`**, the
position after the last committed item. It is keyed by `(agentId, msgId)` and is
not part of any range.

- **Commit.** When the folded canonical item with the same `msgId` gets index
  `k`, the host sends `commit {k, msgId, rev}`. The client inserts `k` and
  removes the overlay in **the same frame**. The replacement happens exactly
  once, and there is no frame with both or with neither.
- **No lost text (fixes D5, the INC-2026-0088 prefix loss).** The overlay
  stays visible until the canonical item's visible text is at least as long as
  the text the overlay showed, or until the host marks the canonical message
  complete. After that the canonical text wins byte-exactly. A thinking-only
  first row therefore never blanks the streamed prose.
- **Deltas carry their offset** (`appendAt`). A delta that does not start at the
  overlay's current length is dropped, and the client requests a snapshot. This
  makes reconnects and duplicated frames safe.
- **Interrupt.** The overlay is frozen at the user's stop. The committed partial
  plus the marker replace it as above. If the CLI commits nothing for that
  message before the turn ends, the host retires the overlay with a
  diagnostic, so the client never decides this by itself.
- **Retry with a new `msgId` (fixes D6).** The host sees the new
  `message_start` and cancels the old overlay explicitly. There is no
  client-side TTL heuristic.
- **Connection drop and reconnect mid-stream.** The client requests
  `[from, highWater)` and the overlay snapshot (full text so far), then resumes
  deltas at the snapshot length.
- **Parallel subagents.** Overlays are per `agentId`. The main chat draws only
  the main agent's overlay, and subagent panels draw theirs.
- **Compaction during a stream.** The overlay is anchored after `highWater` and
  does not belong to a generation. It survives the generation switch, and its
  canonical item commits into the new generation.
- **Invariant.** The final text of a turn in the chat is byte-equal to the
  transcript's (tested today in `chat-streaming-audit.test.ts`).

### Built-in self-check (production)

Every range response carries `count` and a cheap digest: FNV over
`(index, msgId|uuid, rev)` of the items in the range. After commit, the client
computes the same digest over the items it actually **drew** for that range.
When it is pinned to the bottom, it also checks that the last drawn index is
`highWater - 1`.

On a mismatch, the client re-requests the range and records a diagnostic: the
mismatching indices, the drop reasons and the window. Three mismatches in a row
trigger a full window reload and an incident record.

The cost is one hash per item per structural change and nothing per text
delta. A defect of this class then shows up as a self-healing event with a
dump, instead of a silent empty chat.

**Interim check for the current pipeline (before migration).** It applies two
rules:
- the store holds at least one user or assistant row, but the drawable window
  is empty;
- the host's last main-chain uuid is not in the projection, and no refill is in
  flight.

When either rule fires, the client calls `clearJsonlEntries` and
`requestJsonlReplay` with backoff, and attaches `collectSessionDiag`.

## Comparison and why this is better

- **Versus the CLI.** It keeps the CLI's single-owner semantics (one leaf, one
  chain, one fold, last copy wins) by computing them once on the host. On top
  of that it adds what the CLI does not need:
  - bounded client memory;
  - gap detection by index;
  - reachable pre-compaction history: after resume the CLI cannot show it,
    and here it is another generation range;
  - a self-check.
- **Versus browser and editor list views.** VS Code's list view
  (`splice(start, deleteCount, items)`) and the DevTools console's
  virtualization use the same pattern: the model owns an ordered list, and the
  view requests and renders ranges by index. That is the proven shape. The
  addition here is the generation and the revision, because the source (an
  append-only file that can be rewritten) changes underneath.
- **Versus the lead's hypothesis.** It is confirmed with three corrections:
  1. The index must be over **display items on the active branch**, not raw
     records. Otherwise categories 5 and 6 survive.
  2. It needs a **generation** for rewrites and compaction (#378).
  3. Streaming needs the **overlay at `highWater`** with an explicit commit
     and cancel. An index alone does not order a message that has no index yet.

## Consequences

- **Removed:** the categories "dedup memory", "order/fold", "client cursor",
  most "lifecycle flags", "cache staleness" (keys become `(gen, index, rev)`),
  and identity.
- **Remains:** viewport anchoring (BR-16, #674) and renderer predicates such as
  tool grouping, which now run on display items only.
- **Host memory:** an index of `pos`, `uuid`, `msgId` and class per record. That
  is tens of bytes for each of ~100k records, plus the folded items of the
  ranges recently served.
- **Latency:** scroll-up becomes a host round-trip per page, as today. Prefetch
  one page ahead.

## Migration (each step shippable and tested against `chat-audit` fixtures)

1. **Tag live tail rows with `_pos`/`_posEnd`** on the server (fixes D7 and the
   mirror lag). Key the host cache on `_pos` only (D8). Rank by file order
   (D1/D2 are already fixed).
2. **Host display index:** leaf, chain, fold and classification, as a pure
   module. Its oracle is `cliView` from `webview/src/signals/chat-audit/views.ts`,
   moved to shared code.
3. **Range and overlay protocol** (`jsonl:range`, `commit`, `overlay`,
   `appendAt`) alongside the old messages. The webview store becomes a range
   cache. Then delete the structures listed in Decision 2.
4. **Production self-check** and diagnostics. The interim check can ship
   before step 1.

## Alternatives considered

- **Keep patching the client bookkeeping.** This is how ~38 cases were handled,
  and each patch left the next copy free to drift.
- **Ship the whole transcript to the client.** It would have a single copy,
  but this is what caused the 4 GB renderer OOMs, so it is rejected.
- **Index raw records rather than display items.** This keeps the cap and
  window desyncs (INC-2026-0071, D3), so it is rejected.

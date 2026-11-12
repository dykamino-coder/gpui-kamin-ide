# Dashboard analytics metric contract (BR-21)

Contract version: **`metrics-v1`** (decision 2026-11-13). Task:
[BR-21](../RUNTIME_RELIABILITY.md#br-21--define-and-reconcile-dashboard-analytics-semantics).
Source baseline: `origin/main` `5624a330a`.

This document fixes what every dashboard analytics number means, for both
engines (Claude Code and Codex), before the BR-21A–E child PRs change code. It
is a decision artifact: it does not claim that current code already follows it.
The table in [§9](#9-current-gaps-on-the-baseline) lists where the baseline
still differs. A child PR cites the section it implements and adds the fixture
named there; it does not redefine a metric locally.

Plan quota (BR-20 Claude `/usage` windows, Codex `account/rateLimits/read`) is
authoritative percentage data from the provider. It is **never** derived from
local tokens, and local analytics are never shown as quota.

## 1. Sources and engine

| Engine   | Source rows                                                                                           | Usage record                                                                                                           |
| -------- | ----------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------- |
| `claude` | Claude Code JSONL under `~/.claude/projects/**`, plus `proxy_usage` side calls that never reach JSONL | `type:'assistant'` rows with `message.usage`                                                                           |
| `codex`  | Bridge projection of a Codex thread into the same projects tree; every row carries `engine:'codex'`   | one `{type:'system', subtype:'api_usage', engine:'codex'}` row per model response (`thread/tokenUsage/updated` `last`) |

- `engine` is a **row** attribute (`jsonl_events.engine`, `NULL` = `claude`),
  not a session attribute: a chat that switches engine splits exactly.
- Every endpoint accepts `engine=claude|codex`; absent means all engines. For
  every additive metric, `all` equals the sum of the per-engine values.
  Distinct counts (sessions, active days) are recomputed, not summed.
- Codex item rows (thinking, text, tool_use) carry no usage and are not turns.
  Their tool_use still counts as tool activity.

## 2. Canonical relations

### 2.1 Assistant turn

One **turn** = one billed model response.

- Claude: rows grouped by `(session_id, message_id)`; when `message_id` is
  empty the row `uuid` is the key. Usage per turn takes the per-field maximum
  over split/stream rows and over the matching `proxy_usage` record (the
  existing `jsonl_accounting_events` rule). Proxy-only side calls are turns.
- Codex: the `api_usage` row; its key is `usage:<turnId>:<total.totalTokens>`,
  so resume replays are idempotent.
- `<synthetic>` model rows are not turns.

Every metric that counts assistant messages or sums tokens reads this relation,
never raw JSONL rows.

### 2.2 Human user message

A row of `type:'user'` written for a human prompt in a **top-level** session.
Excluded: `isMeta`, tool_result carriers, compact summaries, local command
output, interrupt/caveat markers and every subagent/teammate prompt (those are
delegated prompts, counted only as `delegatedPrompts` if a surface needs them).
Codex: the user item of a turn's input.

### 2.3 Session and subagent attribution

- `session_key = COALESCE(parent_session_id, session_id)`. Subagent and
  teammate rows are attributed to the parent session exactly once.
- Subagents never add to `Sessions`. Their turns and tokens **are** included in
  turn and token metrics (they are real API usage).
- Ownership (token, user) of a subagent row is the parent's `session_tokens`
  row; a subagent without a resolvable parent is reported as
  `unattributedTurns`, not dropped silently.
- Engine switch inside one logical chat: the logical conversation id from
  `conversation.json` maps native ids to one `session_key`; the unified log
  copy of a row with the same `uuid` is a duplicate (follow-up of #792).

### 2.4 Identity

The identity key is the **API token UUID** (`token_id`). `user_name` is
presentation only. Renaming a token keeps one history; two tokens with the same
display name stay separate slices. `?token=` accepts the UUID; an API-token
caller is pinned to its own UUID by middleware. Invariant: `all` equals the sum
of the token-UUID slices plus `unattributed`.

### 2.5 Model

`model_canonical` comes from a versioned alias table (`model-aliases-v<N>`,
for example dated and undated ids of the same model). Grouping, shares and
favorite use the canonical id; the raw id remains available for drill-down. An
id missing from the table is its own canonical id (never folded into a family).

## 3. Time

- Timestamps are stored in UTC. The client sends the viewer zone as an IANA
  name `tz=Europe/Moscow`; `tzOffset=<getTimezoneOffset minutes>` is a legacy
  fallback. Responses echo `tzApplied` and whether DST-correct conversion was
  used.
- Day and hour buckets are computed **server-side in the viewer zone** before
  aggregation. UTC buckets relabelled by the browser are not acceptable:
  events near local midnight, DST transitions and :30/:45 offsets misattribute.
- Ranges are **local calendar days**: `7d` = today plus the previous 6 local
  days, `30d` = today plus the previous 29; `all` has no lower bound. The
  window is `[start of first local day, now)`. Every number in one response
  uses the same window, including `Session tokens` and the heatmap.
- Day arithmetic (streaks, grids) steps calendar dates, never `86 400 000 ms`.

Rollup tables are allowed when they return the same result: a bucket grain of
UTC 15 minutes is exact for every real offset; a UTC-hour grain is exact only
for whole-hour offsets and must fall back otherwise. Distinct counts per day
need a `(session_key, local_day)` or equivalent distinct relation; summing
hourly distinct counts is wrong.

## 4. Token measures

All measures are per turn (§2.1) and summed over the window.

| Measure              | Definition                                                                                                 |
| -------------------- | ---------------------------------------------------------------------------------------------------------- |
| `input`              | uncached input tokens (Codex: inputTokens − cached − cacheWrite)                                           |
| `cacheWrite`         | cache creation tokens (Claude 5m + 1h; Codex cache write)                                                  |
| `cacheRead`          | cache read tokens, always reported separately                                                              |
| `output`             | output tokens; Codex `reasoning_output_tokens` are already inside `output` and shown only as a sub-measure |
| **new-token flow**   | `input + cacheWrite + output` — the Usage chart and "tokens" without qualifier                             |
| **API throughput**   | `input + cacheWrite + cacheRead + output` — only where labelled "incl. cache reads"                        |
| **context snapshot** | `input + cacheRead + cacheWrite` of one turn                                                               |

## 5. Surface definitions

| Surface                                   | Definition                                                                                                                                                    | Tie / empty                                           |
| ----------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------- |
| Sessions                                  | distinct `session_key` with ≥1 turn or human user message in the window                                                                                       | 0                                                     |
| User msg                                  | count of human user messages (§2.2)                                                                                                                           | 0                                                     |
| Assistant msg                             | count of turns (§2.1), subagents included                                                                                                                     | 0                                                     |
| Session tokens → label **Context in use** | sum over sessions of the context snapshot of the session's **last top-level turn inside the window**                                                          | sessions without a top-level turn add 0               |
| Usage chart                               | new-token flow per local day (or hour) per canonical model                                                                                                    | —                                                     |
| Models `in` / `out` / share               | `in` = `input + cacheWrite`, `out` = `output`, share of new-token flow; cache reads shown as a separate column                                                | —                                                     |
| Favorite model                            | canonical model with the greatest new-token flow                                                                                                              | more turns, then lexicographic id; none when no turns |
| Active days                               | local days with ≥1 human user message                                                                                                                         | —                                                     |
| Longest streak                            | longest run of consecutive active local days inside the window                                                                                                | 0                                                     |
| Current streak                            | run ending today, or yesterday when today has no activity yet                                                                                                 | 0                                                     |
| Peak hour                                 | local hour of day (0–23) with most human user messages                                                                                                        | earliest hour                                         |
| Heatmap                                   | human user messages per local day over the selected window; for `all` the grid shows the last 53 local weeks and is labelled so, while counters stay all-time | —                                                     |
| Top Sessions / Users                      | resident server PTY sessions and distinct token UUIDs among them; detached sessions count during grace (live state, not analytics)                            | —                                                     |
| Top `MCP Calls`                           | initiated relay attempts in resident PTYs, including failures/timeouts; label "attempts"                                                                      | resets with the PTY                                   |
| Errors                                    | API error turns per engine (`api_errors.engine`)                                                                                                              | —                                                     |

Top live counters (`userMessages`, MCP fields) are patched by
`session:updated` for every field the card shows, so a newly created session
never stays at its first snapshot.

## 6. Cost

- Cost is an **API-equivalent estimate**, labelled `≈ $X API`, never a bill.
  Claude and Codex are computed from separate provider catalogs and shown
  separately; the combined total is shown only when both are fully priced.
- Prices come from a versioned catalog (`prices-v<N>`: provider, canonical
  model, effective date, source URL, per-measure prices incl. long-context
  thresholds and cache-write tiers). Each response names the catalog version.
- A model without a price contributes `unpricedTokens` and is listed in
  `unpricedModels`; it gets no `$` and no family fallback (an unknown or new
  model is never priced as Sonnet or any other family).

## 7. Freshness

- The ingest layer keeps a monotonic `dataRevision` and `ingestedThrough`
  (newest ingested source timestamp). Every analytics response returns both.
- Response caches are keyed by revision (and window/tz/engine/token); a cache
  hit is valid only for the current revision. A time TTL alone is not enough.
- A request that starts a catch-up sweep returns the current revision with
  `sweeping: true`. When the revision changes, an open Usage/Stats page
  refetches (revision event or a visible-page poll ≤ 60 s). The page shows the
  revision timestamp.

## 8. Legacy routes

`/api/dashboard/stats` per-user maps that copy global model totals or fill
placeholder request/start/error values are not part of this contract. They are
removed or return only fields computed by this contract (BR-21C).

## 9. Current gaps on the baseline

Status on `5624a330a`, source reading only; each row is owned by a child.

| # (BR-21 card)               | Baseline                                                                                      | Owner                               |
| ---------------------------- | --------------------------------------------------------------------------------------------- | ----------------------------------- |
| 2 turn dedup                 | done in `jsonl_accounting_events` (max per `message_id`, proxy merge)                         | keep; regression fixtures in BR-21A |
| 1 subagent attribution       | overview joins `st.session_id = e.session_id`; subagent rows have no own `session_tokens` row | BR-21A                              |
| 7 identity                   | slices filter `st.user_name`                                                                  | BR-21A                              |
| 3 Session tokens range       | last-context query has no window filter                                                       | BR-21B                              |
| 8 timezone / DST             | UTC hourly buckets, client re-buckets; streak math by fixed ms                                | BR-21B                              |
| 6 compact payload            | server supports `agg=hm` + `dailySessions`; client consumption unverified                     | BR-21B                              |
| 4/5 freshness, live counters | 30 s TTL cache, no revision                                                                   | BR-21C                              |
| 11 legacy route              | unchanged                                                                                     | BR-21C                              |
| 9/10 aliases, prices         | raw model ids; Codex catalog in progress (`feat/codex-stats-engine`)                          | BR-21D                              |
| labels                       | "Session tokens", "tokens", MCP calls unlabeled                                               | BR-21E                              |

## 10. Acceptance fixtures (every child)

Deterministic DuckDB/JSONL fixtures, extending the BR-21 card list:

- duplicate streaming rows and a proxy-only side call;
- top-level + several subagents, one orphan subagent;
- Claude + Codex in one chat, Codex resume replay of the same usage row;
- token rename and two tokens with one display name;
- `all/30d/7d` with events on both sides of local midnight in UTC−5 and
  UTC+5:30, and a DST transition day;
- model aliases, a Fable id, an unknown id (no price, listed unpriced);
- partial JSONL append followed by a catch-up sweep (revision changes, page
  refetches).

Invariants: no turn counted twice; subagent usage attributed once without a new
session; neighbouring metrics share one window; `all` = Σ engines (additive) and
= Σ token-UUID slices + unattributed; unknown prices never produce `$`.
Production data is read-only validation: record raw-versus-distinct turn
counts and subagent rows before/after each child without mutating data.

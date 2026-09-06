# Client and server runtime research — 2026-09-07

Research snapshot: [`a34ab4892058bcacc00de8d97ae951fb60551780`](https://github.com/dykamino-coder/gpui-kamin-ide/tree/a34ab4892058bcacc00de8d97ae951fb60551780).
Scope: native UI/CEF, session switching and replay, retained client resources,
loading animations, Bridge/PTY lifecycle, server dashboard and statistics.

The four-agent pass produced **16 research-confirmed incident cards**: one P1
and fifteen P2. Each has its own Diagnostic PR and immutable private evidence
link. These are open problems, not delivered fixes. The source audit and
synthetic checks establish specific contracts and failure orderings; they do
not establish production frequency, actual CPU/RSS savings or the root cause
of the earlier OOM investigation.

The preceding merged-work audit is complete in [PR #47](https://github.com/dykamino-coder/gpui-kamin-ide/pull/47)
and [PR #48](https://github.com/dykamino-coder/gpui-kamin-ide/pull/48), documented
in [runtime-closeout-2026-09-06.md](runtime-closeout-2026-09-06.md).
This research does not change those results or the active verification batch.

## Incident queue

| Incident | Priority | Bounded problem | Diagnostic PR |
| --- | --- | --- | --- |
| INC-2026-0004 | P2 | Layout-hidden webviews remain visible to CEF and bypass eviction | [#49](https://github.com/dykamino-coder/gpui-kamin-ide/pull/49) |
| INC-2026-0005 | P2 | Session-switch cover does not finish its timeout fade | [#50](https://github.com/dykamino-coder/gpui-kamin-ide/pull/50) |
| INC-2026-0006 | P2 | Locate Selected File retries forever beyond the directory row cap | [#51](https://github.com/dykamino-coder/gpui-kamin-ide/pull/51) |
| INC-2026-0007 | P2 | Agent reader mixes same-name transcripts across sessions | [#52](https://github.com/dykamino-coder/gpui-kamin-ide/pull/52) |
| INC-2026-0008 | P2 | Subagent replay bypasses retention cap and survives session close | [#53](https://github.com/dykamino-coder/gpui-kamin-ide/pull/53) |
| INC-2026-0009 | P2 | Stale replay completion strands the next Agents generation | [#54](https://github.com/dykamino-coder/gpui-kamin-ide/pull/54) |
| INC-2026-0010 | P2 | Console repeatedly serializes unchanged multi-tab snapshots | [#55](https://github.com/dykamino-coder/gpui-kamin-ide/pull/55) |
| INC-2026-0011 | P1 | Interactive MCP call is marked delivered after send rejection | [#56](https://github.com/dykamino-coder/gpui-kamin-ide/pull/56) |
| INC-2026-0012 | P2 | Session admission does not reserve capacity across startup | [#57](https://github.com/dykamino-coder/gpui-kamin-ide/pull/57) |
| INC-2026-0013 | P2 | Closing a socket during startup leaves a session without detach grace | [#58](https://github.com/dykamino-coder/gpui-kamin-ide/pull/58) |
| INC-2026-0014 | P2 | Dashboard reconnects after an intentional disconnect | [#59](https://github.com/dykamino-coder/gpui-kamin-ide/pull/59) |
| INC-2026-0015 | P2 | Usage chart accepts responses for an obsolete range | [#60](https://github.com/dykamino-coder/gpui-kamin-ide/pull/60) |
| INC-2026-0016 | P2 | Request history API mixes OTel reads and legacy mutations | [#61](https://github.com/dykamino-coder/gpui-kamin-ide/pull/61) |
| INC-2026-0017 | P2 | Native loading animations bypass reduced-motion mode | [#62](https://github.com/dykamino-coder/gpui-kamin-ide/pull/62) |
| INC-2026-0018 | P2 | Dashboard JSONL export reads entire transcripts synchronously | [#63](https://github.com/dykamino-coder/gpui-kamin-ide/pull/63) |
| INC-2026-0019 | P2 | Chat JSONL export bypasses WebSocket backpressure | [#64](https://github.com/dykamino-coder/gpui-kamin-ide/pull/64) |

Priorities describe the bounded impact, not measured prevalence. P1 marks the
loss of an interactive control request that can block the current tool until
lifecycle recovery. Session restart/interrupt and absolute reaping remain
backstops; the whole application is not claimed to become unusable.

## Coverage and method

| Area | Inspected boundaries | Evidence and limitations |
| --- | --- | --- |
| Native shell and CEF | Browser create/close, visibility/sleep/reap, outbox/pump, frame/texture retention, slot render gates, session covers, native loaders and file-tree Locate | Independent source/control-flow review plus deterministic models. No compiled Rust, native app launch, Windows UI or OS permission access. |
| Bridge client state | Preact roots, event listeners, per-tab stores, replay staging, agent readers, chat windowing, terminal lifecycle, snapshot storage and native persistence shim | Actual TypeScript modules and extracted callbacks executed with synthetic signals/timers/dependency stubs; independent caller and counterexample review. No user transcript or CEF heap sampled. |
| Host, extension and server | Host/exthost readiness/restart, RPC pending paths, connection manager, archive activation, PTY admission/reaper, MCP control delivery, watcher replay/backpressure | Actual-module synthetic race and fault-injection checks. Independent checks included the real snapshot lock and an inert CLOSED socket from the installed `ws` package. No real PTY/provider or deployment workload. |
| Dashboard and statistics | Mounted dashboard routes, auth/socket lifecycle, Usage chart, cards, session stats sampling, OTel and JSONL query contracts, history API mutations | Actual-module deferred-response/socket/route checks and independent SQL/caller review. Visual behavior inferred from source, not screenshots or browser automation. Existing analytics defects deduplicated against BR-20/21. |
| Export paths | Dashboard authenticated JSONL download and Chat single-file export, server read/send boundaries, client accumulation and save flow | Source reachability review and extracted WS handler with synthetic content. No real download, production data, latency or memory benchmark. |

The pass mapped the relevant repository surfaces and deeply traced the paths
listed above. It is a bounded audit, not a proof that every path in the repository
is defect-free. Build/release/tooling files were consulted for flow and validation;
they were not subjected to another unrelated exhaustive correctness audit.
No Computer Use, OS permission prompt, production mutation or functional code
change was performed. Temporary diagnostic harnesses live with private evidence;
the public changes are Markdown only.

## Existing backlog and counterchecks

Several broad suspicions had existing protections or existing tasks:

- **Chromium lifetime:** session contents share a Chat view; browsers are keyed
  by view ID. Non-Chat hidden views already have a 20-second eviction policy,
  and a renderer-process-limit is configured. This does not prove an absolute
  process/memory cap. INC-2026-0004 isolates a specific layout-hide path that
  bypasses the policy; “one permanent Chromium per session” was rejected.
- **Frame and transcript bounds:** native latest-frame storage and retired/GPU
  texture bounds exist; main Chat windows its rows. The new Agents retention
  defect is a separate unbounded reader/cache path, not a claim that all views
  mount the complete history.
- **Backpressure:** normal session sends have a 16 MiB guard, and main JSONL
  watcher/replay retries preserve offsets. INC-2026-0011 concerns incorrect
  MCP delivery bookkeeping; INC-2026-0019 concerns a bypass in export. A generic
  “server has no backpressure” task was rejected.
- **Startup and cleanup:** archived tabs are warmed without eagerly connecting
  all of them. Heartbeats, same-conversation resume locks and reaper backstops
  exist. They do not eliminate the admission/startup races in 0012/0013.
- **Dashboard work:** session resource polling reads a cached background process
  sample; it does not perform a full process scan for every request. Some cards
  already cancel stale requests. The Usage chart has a separate missing guard.
- **Terminal work:** hidden resize checks and xterm/timer disposal exist. The
  Console finding concerns periodic whole-map snapshots and retained keys.
  A hidden dashboard terminal sends periodic refresh requests, but a claim that
  same-size PTY resize forces an expensive redraw was not established and was
  not promoted as a confirmed CPU defect.

[RUNTIME_RELIABILITY.md](../extensions/claude-bridge/RUNTIME_RELIABILITY.md)
remains authoritative for existing BR work:

- BR-02/03 and INC-0001 still own actual long-session memory/CPU measurement.
  The new localized workloads provide candidates, not retrospective OOM proof.
- BR-04/08/12/13/17/18/18A/19/24/31 remain distinct host, deployment,
  observability and lifecycle tasks; matching observations were not filed again.
- BR-25/26/27 concern Agents completion/replay/partition. INC-0007 is transcript
  identity, INC-0008 retention and INC-0009 a listener-level generation residual.
  The working helper-level generation check was explicitly verified.
- BR-05/09/10/29 retain their missing native/live acceptance. Static research
  does not close them. BR-16/22 history geometry and BR-23 toast semantics also
  retain their existing scope.

## What dashboard figures mean

The server visual was inspected through its mounted components and routes.
It mixes several data sources with different lifetimes; they must not be treated
as interchangeable totals.

| Surface | Current source/meaning | Remaining work |
| --- | --- | --- |
| Active Sessions / Users | Resident server PTYs, including detached sessions during grace; users are distinct names among those sessions | Admission and cleanup tasks 0012/0013; preserve the distinction from connected browser tabs |
| Session CPU / memory | Cached server-side process samples | Real load/latency measurement; no new per-request scan defect established |
| MCP Calls | Initiated relay attempts in resident PTYs, including failures/timeouts; resets with the PTY | Delivery correctness 0011; do not present this as successful tool completion |
| Account quota | Provider `/usage` parsing/cache, distinct from local analytics | Existing BR-20 parser/model-label/freshness acceptance |
| Usage chart / Stats cards | Local JSONL-derived analytics and caches | Existing BR-21 metric identity, aggregation, parent attribution, range/timezone, prices and freshness; additional chart request race 0015 |
| Request history API | OTel reads mixed with legacy request-table mutations | 0016: choose a coherent supported contract or explicit retirement |

BR-21 already records eleven concrete analytics discrepancies; this pass did not
create copies. The old request modal has no current opening caller, so its
pagination and filtered-delete UI were not claimed as live reproductions.
The API inconsistency is documented at the authenticated route boundary.

## Maintainer handoff

The owner's authorization for this batch is **research only**. These Diagnostic
PRs contain no functional fix, do not mark incidents resolved and do not trigger
a release. Review and integrate research separately from a later authorization
to implement. Each incident already states a bounded deliverable, existing
mitigations, missing evidence and automated/runtime acceptance.

Suggested planning order, without changing `RUNTIME_EXECUTION.md`:

1. Lost MCP delivery (0011), then admission and startup cleanup (0012/0013).
2. Coordinate Agents identity/retention/listener work (0007–0009) because their
   state and callbacks overlap. Keep BR-31 delivery acceptance distinct.
3. Coordinate hidden-view lifecycle and Console restore (0004/0010), and cover
   deadlines with reduced-motion scheduling (0005/0017). Removing animations
   must not stop render-driven retries/deadlines from progressing.
4. Locate (0006), dashboard lifecycle/chart/API (0014–0016), then the two export
   transports (0018/0019), preserving authorization and byte-exact output.

Native fixes require their named Windows gates; server contracts can first be
checked with disposable Linux/API/transport fixtures and synthetic data. No
production credentials or mounts are needed for those contract checks.

Unpromoted leads remain research questions: application readiness versus the
third-party webview fallback/watchdog, stale ChatBound correlation, archived
history response ownership, restart field preservation and cache eviction over
long histories. They lack a completed end-to-end contract/reproduction in this
pass and must not be treated as additional confirmed defects. The related
cards and existing BR work provide the entry points for a subsequent bounded
investigation.

## Publication verification

Each Diagnostic PR changes one incident Markdown file. Private artifacts have
parsed manifests, verified SHA-256 checksums and scoped source/synthetic-data
review. Public cards name the immutable private evidence commit and future
acceptance. Public/private PRs link back to each other. No release versions,
functional code or execution-registry state changed. CI scope/quality results
are available on each linked PR; passing documentation checks is not product
runtime acceptance.

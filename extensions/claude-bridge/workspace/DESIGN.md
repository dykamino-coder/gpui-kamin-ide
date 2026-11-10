# Browser workspace client design

Task: [INC-2026-0087](../runtime-issues/INC-2026-0087.md).
Runtime overview: [README.md](README.md). Podman runbook: [PACKAGING.md](PACKAGING.md).
Status: implementation in progress; the current live suite passed, while full parity and final composed acceptance remain open.

## Host text-area size query follow-up

Terminal applications can query the current character dimensions with `CSI 18 t`.
The browser suppresses replica query responses, but the headless host currently
leaves xterm's `getWinSizeChars` option disabled, so no parser answers this query.
Enable only that existing xterm response in the authoritative `TerminalScreen`.
The reply must use its current rows/columns, including after an ordered resize,
and work with zero pages. Browser replicas remain silent even when their own
window-report option is enabled; user input must remain unaffected.

Acceptance: parser regression before the fix, initial/resized/split queries,
real Linux PTY with zero attached pages and after reopen/resize, and two production
browser xterm replicas. Retain disabled unrelated pixel/window operations and
existing resource limits. This bounded change does not establish full TUI parity,
actual-model acceptance or exhaustive capability proof. Completion is a green
Change PR and READY handoff; the Claude lead reviews and merges implementation.

Refresh validation exposed a Windows-only fixture rename sharing violation while
replacing a watched file. Bound retries to 500 ms for that external editor action
and only Windows `EPERM`; retain a real atomic rename and all watcher assertions.
Other errors and persistent sharing violations fail. No production file operation
or acceptance timeout changes.

## Durable submission receipts follow-up

The host must retain accepted submission identities across a process crash.
Before forwarding text to the shared handler, write an atomic, fsynced snapshot
under `WORKSPACE_STORAGE`, scoped by the workspace/server/account hash. Keep
the existing 10,000-identity / 32 MiB text limits, exact text digests, allowlisted
receipt fields and consumed fallback row identities. Persist receipt transitions
and explicit session removal independently of attached browsers. No connection
credential is stored in this snapshot; prompt text is private workspace data.

Restore only identities for the restored open tabs. Confirmed UUID commitment,
consumed fallback receipts and cancellation survive unchanged. Unconfirmed
delivery becomes noncancelable `uncertain`, with no automatic resend or legacy
text matching after restart. An upstream UUID can still establish commitment.
An identical identity is suppressed, and reuse with different text is rejected.
This guarantees duplicate suppression within the persisted open session, not
exactly-once CLI delivery or survival of in-flight tools. Corrupt/unreadable
state fails startup; a failed write blocks new submissions until storage is
repaired and the host restarted. Observer write failure does not stop existing
agent execution. Snapshot writes are synchronous and bounded; large retained
prompt sets may add ingress latency. Composer drafts remain a separate gap.

Acceptance: a real shared-host process killed after forwarding, restored
receipts and identical-ID suppression, committed/consumed/dismissed metadata,
scope/validation/write failure regressions, and production Chromium in an owned
Linux Podman fixture. Full parity and actual-model composed acceptance remain
open; this increment does not change the strict completion validator.

Executed: Windows workspace 97 passed / three Linux-only skips; Linux Podman
production-browser fixtures 15/15, including SIGKILL/restart with zero pages,
two fresh pages and upstream exactly-once observation for each reused ID. This
observed fixture count is not an exactly-once delivery guarantee. The existing
native replay fixture now waits for snapshot-end before checking receipt state:
the final transcript row is sent before the receipt portion of that snapshot.

## Restart persistence follow-up (INC-2026-0092)

The client host owns the open set and active selection across process restarts,
in addition to the existing renderer pin/order preferences. An atomic snapshot
under `WORKSPACE_STORAGE` stores only tab identity, cwd, label, title and creation
time, scoped by server/account hash. Restore uses the shared TabManager's lazy
background resume and selects the saved active tab, without browser requests or
parallel CLI startup for every saved conversation. Observer writes are paused
during restore/disposal. Explicit close/disconnect updates the durable open set.
History resume uses the existing ConfigStore label before creating the tab.
Real host process restarts and own-container headless tests verify this contract;
this follow-up does not establish full browser parity.

## Problem and completion contract

The Bridge server owns Claude CLI sessions. Its client owns project operations,
shells, executable plugins and UI delivery. A dashboard alone cannot replace that
client. The browser workspace runs the same client host in a Linux container,
with persistent `/workspace` and current webview roots served in a browser shell.

Owner requirement, 2026-11-09 (verbatim): «важно что бы страница в браузере
тонкой была и если ее закрыть и открыть работа с агентом не прервется».
Sessions, connections, tools, queues, prompts, files/watches and terminal state
must belong to the host. Zero attached browsers must not interrupt work.
Reopening must restore current transcript, streaming/tool/prompt state, files
and scrollback without repeating a mutation. Bounded recovery implementations
exist; remaining host-authoritative page state remains an acceptance gate.
FilePanel and explorer are excluded by the corrected browser scope below.

The agent-controlled PREVIEW/HMR adapter is merged and has synthetic browser
coverage; exhaustive actual-model behavior acceptance remains open.

Completion requires reviewed Change PRs with applicable green CI, an exhaustive
capability gate and actual headless browser acceptance in Linux Podman. The
Claude lead merges implementation and final documentation. Registration #638
was merged separately; it did not accept the product. Release, version bumps,
image publication and owner-performed testing are outside this task.

### Strict completion gate follow-up

The existing `test:parity` validates source classifications and four implemented
provider prerequisites. Preserve that CI contract. A separate
`test:parity:complete` must reject absent observable evidence for every supported
inventory item, remaining host-state/lifetime contracts and final composed live
scenarios. Its input is an external sanitized execution receipt for the exact
clean runner/image revision, with individually named passing assertions and
successful cleanup. Missing, skipped, failed, duplicate, stale or unknown proof
IDs fail. Native exclusions retain the reviewed matrix reasons; FilePanel and
explorer UI remain outside the corrected scope.

This bounded follow-up implements receipt validation, negative regression
coverage and a partial receipt from the existing live harness. It preserves
executed scenario outcomes and never invents capability/remaining-contract proof.
No current receipt is sufficient to close INC-0087. Execution harnesses must emit assertions
after exercising behavior; hand-authored PASS receipts and test-path existence
are not acceptance evidence. The lead reviews this Change PR; the full task
remains open until the remaining suites actually run on the composed candidate.

## Browser shell scope correction

The browser entry previously mounted the imported standalone `App`, whereas
KaminIDE registers `chat.html` (`ChatRoot`), `tools.html` (`ToolsRoot`) and the
customize roots. The browser now needs its own small shell around those current
tools. It must not reuse the standalone titlebar, updater or window controls.
The shell follows GPUI's titlebar, activity rail, sessions list and panel slots:
42 px top bar, 48 px rail, 270 px resizable sessions sidebar, 8 px dock gutters,
session tabs and movable Claude Bridge, Bridge Console and Agents panels.
FilePanel and the explorer are outside the corrected browser layout scope.

The shared `ChatRoot` remains mounted once, including while settings or another
dock tool is selected. Its existing listeners initialize and route chat, terminal
and agent state. The console uses the same `TerminalPanel` as `ToolsRoot` and
settings use the current customize section router. Existing BridgeHost APIs own
sessions, saved sessions, pins and tab order. A browser-specific host wrapper owns
validated dock/sidebar preferences and broadcasts revisions to all views; atomic
storage commits precede acknowledgement. Disconnected pages never replay layout
mutations. Layout is checked visually in Linux Podman; tests cover meaningful
state transitions, malformed operations, persistence and multi-view delivery.
The first shell deliverable does not claim pixel parity with an unavailable
full reference capture or completion of the remaining live acceptance matrix.

## Architecture

```text
browser: thin shell + current ChatRoot/Console/Agents + KaminBridgeApi
  | authenticated same-origin WebSocket: invoke/send/event/invoke-reply
workspace HTTP host: browser effects + files/views + host PTYs
  | shared BridgeHost + IPC registry + TabManager + ConnectionManager
  | exact existing toolRegistry + hooks + MCP + plugin LSP + monitors
  | existing authenticated Bridge protocol
Bridge server: Claude CLI PTYs and session-local proxy plugin
```

`src/cli.ts` composes `createBrowserEnvironment`, `createSharedBackend`,
`withBrowserServer`, persisted workspace-shell state, `WorkspaceFiles` and
`TerminalWorkspaceBackend` before
listening. Disposal unwinds terminals, watches and shared runtime. The workspace
build imports the actual extension modules and aliases the small VS Code
environment surface at build time; the native build retains its normal APIs.
Unsupported environment operations throw instead of returning blanket success.
No handler is copied into a second tool implementation.

The native runtime remains the GPUI shell, CEF webview, Node `kamin-host` and
extension host. `@kaminide/host-compat` retains its compatibility names; those
names do not imply an Electron runtime. Shared title regeneration is registered
in BridgeHost; the native SessionsBridge adapter preserves sticky-name metadata
updates. Finished notifications retain native activation when installed and
otherwise use the existing browser toast navigation event.

There is one UI. The browser build defers modules until authentication and
transport installation; it differs from the desktop single-file packaging.
The asset handler snapshots trusted built assets before listen, rejects linked
or oversized/deep trees, and never exposes project files through HTTP routes.
Changes to shipped application assets require a host restart.

There is no deviation from the lead's client-host architecture. Combined mode
means separate client/server containers in one explicitly selected Compose
project. It preserves CLI ownership, the server protocol and separate auth;
it does not install Claude CLI into the workspace client.

## Identity and browser boundary

The workspace has one pinned upstream server/client-token identity. Existing
`/api/dashboard/tokens/resolve` validation supplies authentication. A different
valid token cannot attach to an already bound workspace. Browser connect/resume,
config and Settings requests cannot replace the configured identity or choose
arbitrary administration targets. Dashboard-admin login is a separate remaining
path; an auth-disabled dashboard is never workspace authorization.

Login creates an opaque expiring HttpOnly SameSite cookie; HTTPS origins enable
Secure. HTTP mutations and WebSocket upgrades validate exact Host and Origin.
Revocation, expiry and upstream validation failure deny access. Input, delivery
and slow-client buffers are bounded; lost delivery does not kill host work.

The upstream credential enters through environment, stdin or a read-only mounted
file and stays in memory. Workspace ConfigStore disables credential persistence
before legacy migration, ignores stale stored credentials and omits the token
from JSON/atomic temporary files. Browser-visible config/events redact it;
startup/watch failures use fixed messages. The native policy is unchanged.
There is no secure-erasure claim for credentials written by older versions.

Node binds to loopback by default. Container listening on all interfaces is an
explicit image configuration with loopback host publication by default. Other
binds require an exact origin; remote publication requires a deliberate TLS/proxy
configuration. Authenticated access grants shell access inside this container.

## Host lifetime and recovery

One process owns one shared client runtime. Each browser socket owns only its
delivery subscription and private invoke replies. Closing, reloading, logout,
transport failure or a slow page never calls disconnect/dispose on that runtime.
Explicit End session, tab close, Disconnect and host shutdown are terminal paths.
The shared runtime cancels owned foreground/background shell work and external
MCP requests before dropping tracking; transient upstream reconnect retains work
for acknowledged result delivery. Client shutdown detaches server CLI sessions.

Shared ConnectionManager caches transcripts/streaming state. Presentation state
indexes current tab/global status, prompts and submission receipts. Re-attach
performs read synchronization followed by live events. Request IDs are retained
and duplicate permission responses cannot settle the same retained prompt twice.
Stable submission IDs are deduplicated; the 10,000-receipt/32 MiB limit rejects
new entries rather than evicting identities and allowing duplicate execution.
The host also retains native submission commitment by stored UUID, observing
JSONL even with zero pages. Fresh pages receive this monotonic marker after the
accepted row leaves the bounded replay tail; equal prose never commits another
identity. This extends receipt recovery without completing transcript reduction.
Proof includes both receipt/row arrival orders, an actual shared-host transport
test and Linux production-browser acceptance with zero pages, 4600 subsequent
records and two fresh pages. Host restart persistence of in-flight receipts is
outside this bounded change.
Lost invoke results are unknown outcomes, never automatically retried mutations.
These mechanisms do not complete the host-authoritative transcript reducer and
queued-message migration; the browser still processes substantial page state.

Logical FileViews keep file/tree subscriptions alive with zero sockets, associate
an opaque view identity with the page and refresh snapshots on attach. The host
retains detached view services independently of browser visibility. FilePanel,
its editor state and the explorer are excluded from this browser layout. Shared
file tools and attachments remain available. The inventory classifies native IDE
editor/reveal/openInVscode separately; PREVIEW state and general host-page recovery
retain their own behavioral acceptance requirements.

Host PTYs belong to chat tabs. Concurrent spawn coalesces per tab; re-attach
restores a sequenced headless-xterm screen/scrollback snapshot before live output,
without executing another shell. Explicit kill/tab close/disconnect/shutdown
terminates owned processes. Records, input queues, screen cells and serialized
snapshots are bounded and truncation is reported. Detach preserves a shell;
host replacement does not preserve its running process or screen. Remaining
OSC palette/pixel-window queries have not been accepted as full TUI parity.

## Persistence and container boundary

Initial cwd/project root is `/workspace`. Tools can access ordinary absolute
container paths and symlink destinations; `/workspace` is not a filesystem jail.
Container isolation and explicitly chosen mounts define the machine boundary.
Project and client HOME/global storage persist in separate owned volumes. Client
HOME contains plugin configuration, not Claude CLI login. CLI HOME/auth remain
server-owned, including in combined mode. Never adopt another server's volumes
or mount host HOME, unrelated worktrees or a Podman socket into a normal client.

Persistent files/settings can survive container replacement. Running shells,
presentation snapshots and file-view identities are process-owned. Resume saved
sessions explicitly after restart; do not replay an uncertain pending command.
The packaging runbook describes unprivileged UID/GID 10001, volume access,
separate-server/combined profiles and exact owned-resource lifecycle.

## Capability proof

The exhaustive inventory must derive from KaminBridgeApi, INVOKE/SEND/EVENT
tables, shim specials/dynamic patterns, actual IPC registrations and the client
tool registry. Compare both directions: source additions without tested rows and
stale rows fail. Generic invoke cannot excuse an unimplemented named method.
Source classification or a named test path alone is not behavioral coverage.

| Group | Implementation | Observable acceptance |
| --- | --- | --- |
| Chat, tabs, sessions, streaming, submissions | Shared host/session dispatch and recovery | Create/resume/switch/close, once-only queue, full turn, permissions, two pages |
| File tools and shells | Exact shared Read/Write/Edit/Glob/Grep/NotebookEdit/Bash/PowerShell handlers | Real bytes/search/cwd/env/output, background work, errors/cancellation |
| Agent/team/task/plan/skill/widget/todo tools | Exact tool registry and routing | Actual requests, responses, state and identity isolation |
| Files and attachments | WorkspaceFiles and browser environment | Shared file tools and attachment services; FilePanel/explorer hidden by owner scope |
| Interactive terminals | Host PTY registry/headless screen and same terminal UI | Input/output/resize/exit/kill, zero-view work, scrollback, shared OSC palette state; remaining TUI behavior stays open |
| Hooks | Shared approval/hash store and relay | Inert before approval, real execution, changed hash reapproval |
| MCP and plugin discovery/configuration | Shared manager, IPC, sync and registry | Local stdio/HTTP tools/resources/prompts, OAuth/elicitation/cancellation/cleanup |
| LSP and monitors | Shared plugin managers | Real fixture server diagnostics/hover/definition/references, restart and cleanup |
| Settings and browser effects | Pinned administration proxy and browser adapters | Durable settings, export/upload/clipboard/notifications/navigation/audio |
| Compatibility/generic invoke/events | Explicit environment semantics and event routing | Producer/consumer assertions, unknown-channel errors, no silent stubs |
| PREVIEW with HMR | Shared preview adapter | Synthetic browser HMR/rebuild/zero-page coverage; actual-model acceptance remains open |

Existing plugin boundaries in PLUGIN_HARNESS.md remain unchanged. The image
supplies the executable dependencies it advertises, including PowerShell.
Native editor LSP fallback commands reject explicitly; this does not replace
testing the shared plugin LSP path.

#650 supplies the bidirectional source inventory and mutation tests. The current
`test:parity` checks four implemented provider prerequisites: `host-terminal`,
`title-regeneration`, `finished-toast-route` and `preview-hmr`. Provider/test-file
existence is checked; executing an observable contract for every classified
operation remains outstanding. The separate `test:parity:complete` encodes that
missing proof and the remaining completion contracts. It rejects absent evidence;
the current prerequisite success message does not establish full parity.

## Acceptance and remaining work

Deterministic integration uses the real shared host/handlers and a synthetic
upstream protocol peer. It proves bounded protocol behavior without a model
login. The production-bundle smoke separately tests the real HTTP/WS/assets
with an explicit fixture backend. Neither is live CLI/model acceptance.

Final live acceptance uses the lead-approved server and a separately issued
client token with an isolated Podman client project. Never stop/reconfigure that
server or inspect its volumes. Execute the same browser UI in headless Linux
Playwright; never launch a native Windows browser, including headless Chromium.

Required live scenarios: complete chat turn; real Read/Write/Edit and Bash;
fixture MCP tool and approved hook; session pins and dock synchronization; close during
long Bash/MCP work with zero browsers; reopen with complete nonduplicated
transcript/tool state; retain a permission and answer it once across two pages;
restore service watches, shell layout and terminal scrollback; explicit disconnect and
shutdown cancellation; persistence after owned container replacement; PREVIEW/HMR.

Record source/dirty state, locks, image ID/source, browser version and executed
scenario results. Measure all production JS chunks raw/gzip/Brotli and post-GC
heap at startup, long turn, zero-view continuation and repeated re-attach.
Startup figures do not prove long-turn retention or a thin page. Raw logs,
transcripts, credentials, screenshots and traces stay outside the public repo;
publish only sanitized summaries. FAIL stays failure; BLOCKED means unrun;
XFAIL remains incomplete acceptance. A later source fix needs an executed rerun.

The task stays open until full host state, strict behavioral parity, PREVIEW/HMR
and the final composed live candidate pass. The agent performs acceptance and
reports exact residual blockers to the lead; the owner is never asked to test.

## Historical executed live subset (#653)

The [#653 report](https://github.com/dykamino-coder/gpui-kamin-ide/pull/653)
records **15/15 PASS, exit 0**, run `2026-11-09T14:49:15.369Z`, runner and
image source `a0207c8b53c60cb9d34fb80bd5c88552e824975c`, image
`cae40cb044a48d17aff91d8d82b99eec8eeea13cf77ed49fe0e5c5ade136212d`.
That source tree equals merged main `65fa7467`, including #650's title/toast
integration. This supersedes the older #653 subset report; both former
FilePanel/Allow-click failures passed. The permission test uses the real Allow
button, with no forced click or injected permission response.

Its assertions cover image contract, credential storage, upstream health,
workspace start/login, chat, file tools, Bash, MCP, hook, FilePanel,
close-mid-turn, reopen-permission, two browser tabs and browser metrics.
Chromium `158.0.8127.18`; 34 production JS chunks total 2,268,318 raw bytes,
732,461 gzip and 591,675 Brotli (sums of individual files). Post-GC used heap:
5,071,372 bytes at startup and 11,824,116 bytes after the suite (total heap
5,906,432 and 12,984,320 bytes respectively). These are the suite's recorded
points, not a complete long-turn/repeated-attach retention study.

This is executed subset evidence supplied by bridgeweb2, not a rerun by the
final documentation author. It predates #655 PREVIEW/HMR and #657 fallback-receipt
recovery and does not prove full host-owned page state, host/container restart,
or every desktop tool behavior. The readiness manifest at that snapshot did not
encode the remaining completion requirements. The strict completion gate above
now rejects missing execution proof; it does not extend this historical evidence.

## Bounded follow-up: long-turn browser retention metrics

Implementation and exact-candidate validation:
[#687](https://github.com/dykamino-coder/gpui-kamin-ide/pull/687), pending lead review.

Exercise the production browser bundle and real shared BridgeHost with a
deterministic synthetic upstream: 8,000 distinct transcript records while attached,
another 2,000 with no attached page, then five fresh-page reattachments. Each row
contains roughly 1 KiB of prose. Record every production JS chunk's summed raw,
gzip and Brotli bytes, Chromium version, exact runner source/dirty state, and
post-GC page heap at startup, during the long turn and after every reattachment.

Acceptance checks the existing 4,000–4,500-record attached store window,
200-record browser reattach snapshot and 150-row initial render window, the
newest record and unique ordered UUIDs, a shared tool
response while no page exists, and no extra submit or interrupt frames. Page
heap must stay below 128 MiB at every measured point; equal-tail reattachments
must span no more than 16 MiB of used heap. These deliberately coarse regression
budgets measure renderer JS, not browser RSS, host memory or a universal thin-page
guarantee. Fresh pages test recovered state; they cannot establish absence of
leaks in a destroyed renderer. Preserve measurements even when a budget fails.

This automated Linux Podman fixture adds repeatable load coverage. It does not
emit full-parity receipts or satisfy the remaining actual-model/composed-image
acceptance contract. No owner server or native Windows browser is involved.

## Historical follow-up: host-owned FilePanel controls (2026-11-09)

This records the earlier implementation scope. The owner's later correction
above excludes FilePanel/explorer UI and its unfinished state polish from browser
acceptance; shared file/context services remain available.

The watch list is not the open-file model: it cannot recover per-chat file
order, an explicitly closed file, active editor, selection context, attach
toggle or code/preview choice. Retain a versioned data projection in the
existing logical host file-view, delivered with `workspace:files-state` on
read-only sync. A private `workspace:file-panel` send replaces that projection;
the browser keeps only the opaque view identity. Reuse the existing FilePanel
and file operations; no new public API or second editor is introduced.

Validate/whitelist before replacement: at most 64 chats, 128 files per chat,
256 preview choices and 2 MiB of UTF-8 state. Retain selected context and main
cursor offsets, not editor instances or whole documents. Concurrent pages
clone controls into independent logical views; closing all pages leaves the
host projection available. Recovery reads current disk content and clamps
cursor offsets, without writing content or replaying commands.

Acceptance: schema rejection preserves the previous projection; host tests
cover zero-page recovery, chat mapping/order, closed files and independent
pages; renderer tests cover restoration without feedback sends; Linux Podman
Chromium runs the production App and real file adapter across close/reopen.
This increment does not retain unsaved drafts, undo history, scroll, multiple
selection ranges or state after host restart. Those were gaps in the earlier
FilePanel scope, not requirements of the corrected browser layout.

## Bounded follow-up: Escape queue restoration

The existing Escape hook restores fallback queue text into the current page's
composer, but clears only its local queue. A second or reopened page can recreate
that queue from the host receipt. Extend the existing `interruptSession` call with
optional exact queue IDs, and retain a host-owned display dismissal marker for
those receipts. Keep forwarding the ordinary shared interrupt handler; dismissal
does not claim that already delivered PTY text was cancelled in CLI. Stop without
queue IDs keeps its current behavior. Dismissed receipts must still participate in
JSONL matching so their eventual rows cannot consume a later identical message.

Acceptance for this increment: real host reducer regressions for selected IDs,
tab isolation, late fallback/duplicate receipts and identical-message matching;
shared renderer receipt tests; Linux Podman production-browser Escape with two
pages and a reopened page through the real BridgeHost. Full INC-2026-0087 parity
and actual-model acceptance remain separate, open requirements.

Validated on this increment: the host reducer first failed before the fix;
the Windows workspace suite passes 89 tests (two Linux-only skips), shared
webview passes 469 tests, and Linux Podman Chromium passes all 11 production
browser fixtures including Escape across two pages and zero-page duplicate
replay. The browser upstream is synthetic; this is not actual-model acceptance.

## Bounded follow-up: individual queue removal

The Message Queue Remove button currently mutates only the initiating page's
signal. Reuse the process-owned dismissal metadata introduced for Escape through
a private `workspace:queue-dismiss` invoke; keep the existing generic bridge
transport and shared widget. Only host-owned items use this route. The native
client's local queue continues using its existing local removal behavior.

Validate the session and exact item IDs before changing any receipts. Broadcast
the resulting receipt to all pages and retain dismissal in reconnect snapshots,
including late duplicate fallback receipts with no attached pages. This operation
only hides the delivered queue display: it must not interrupt, resubmit text,
restore composer text or claim that the CLI cancelled delivery. Dismissed items
still reserve their eventual JSONL match, as with Escape.

Acceptance: host reducer regressions for selected IDs, tab isolation, malformed
requests, idempotence and retained matching; Linux Podman Chromium exercises the
real Remove button with two pages, an unrelated remaining item, zero-page late
receipts and a reopened page through the shared BridgeHost and production UI.
The upstream fixture is synthetic. Restart durability, exhaustive capability
proof and composed actual-model acceptance remain open in INC-2026-0087.

## Bounded follow-up: core tool execution evidence

The inventory validates registrations and providers; it does not demonstrate
handler behavior. Add a Linux production-browser fixture using the existing
shared BridgeHost and synthetic upstream. Create a session through the shared
UI, close every page, then execute Read, Write, Edit, Glob, Grep, Bash,
BashOutput, KillShell, NotebookEdit, TodoWrite and TodoList through upstream MCP
frames. Assert independent filesystem bytes, search results, shell output and
process termination, notebook cells and todo round trips. Reopen the browser
and verify that the same session and edited file remain accessible.

A small reviewed mapping ties each asserted tool case to its tool and handler
capability IDs. Record a PASS only after the case's assertions complete; reject
unknown, duplicate, failed or omitted cases. Validate that mapping against the
independent inventory. Emit sanitized partial execution evidence with source,
browser and synthetic-upstream provenance, never filesystem contents or tool
payloads. The fixture runner builds current sources inside an existing image;
it must explicitly disclaim candidate-image acceptance and must not generate
a full-completion receipt. Keep the strict parity validator unchanged.

Acceptance: evidence collector negative tests, inventory checks, existing
workspace tests and the complete Linux Podman production-browser fixture suite.
This is a bounded test Change PR under INC-2026-0087; registration #638 already
exists. Claude lead owns review/merge. Full per-capability coverage and composed
actual-model/image acceptance remain open.

Implementation: [#691](https://github.com/dykamino-coder/gpui-kamin-ide/pull/691).
The partial artifact intentionally has no completion-receipt schema or image
provenance fields. Only an entirely passing fixture run can support its listed
assertions; it cannot establish restart survival or exhaustive input coverage.

Implemented in [#686](https://github.com/dykamino-coder/gpui-kamin-ide/pull/686).
The host ingress regression failed before the fix and now passes; Windows
workspace passes 92 tests (three Linux-only skips), shared webview passes 469,
and Linux Podman Chromium passes all 13 production browser fixtures. The Remove
fixture confirms processing of the zero-page duplicate before reopening through
the next shared tool response. These bounded checks do not close full parity.

## Bounded follow-up: durable composer drafts

Retain each open session's unsent text and attachment metadata in the workspace
host, scoped to workspace root, pinned server and token identity. Persist before
acknowledging a private draft update using an atomic replacement. Keep a monotonic
revision, including empty drafts, so an old page cannot resurrect cleared text.
The browser's existing composer hook uses a browser-installed persistence port;
the native client keeps its existing storage behavior and there is no second UI.

Every user edit sends a bounded update, without replaying writes on reconnect.
Read-only sync restores the host draft after reopening or restarting. Concurrent
pages use compare-and-set revisions: clean pages follow host changes; a page with
local conflicting edits retains them and offers explicit workspace/local choices.
Storage and disconnect failures remain visible as unsaved, never as Saved. Text
or attachments are not put in localStorage or sessionStorage. Sending clears the
draft through the ordinary composer clear path; clearing is acknowledged separately
from PTY delivery and does not assert atomic submission-plus-draft durability.

Bounds: 128 session drafts, 256 KiB text, 32 attachments, 8 MiB per draft and
32 MiB total, with strict JSON shape validation. Draft references never cause file
writes or tool execution. Closing a session prunes its draft. Attachment contents
remain at their existing container paths; image previews may retain bounded data
URIs. Invalid data and write failures must preserve the previously accepted state.

Acceptance: real host restart and two-page production Chromium tests for text,
attachments, per-session isolation, conflict rejection/resolution and sent-text
clearing; unit coverage of scope, malformed/oversized data, stale revisions and
storage failure barriers. Linux Podman uses the real shared BridgeHost and a
synthetic upstream. Full parity and composed actual-model acceptance remain open.

Queue Escape restoration now uses the same per-session durable draft: clean other
pages and reopened pages recover the restored text. Queue dismissal still never
resubmits or claims cancellation; conflicting pages retain their explicit choice.

Implementation: [#690](https://github.com/dykamino-coder/gpui-kamin-ide/pull/690).
The initial real-host draft invoke failed before implementation. The combined
candidate with durable submission receipts passes all 17 Linux Podman production
browser fixtures; this fixture upstream is synthetic. Strict full completion
continues rejecting 545 missing execution-proof gaps.

## Bounded follow-up: browser external MCP behavior

Exercise the production browser transport and shared MCP manager against an
owned loopback HTTP MCP fixture. Assert paginated tools, resources, templates
and prompts; resource reads and parameterized prompts; connection edits,
disable/enable/removal; and two-page server/log/progress notifications. Independently
inspect the fixture's received requests, container files and MCP configuration.
Close every page, execute an upstream external-tool call and resource/prompt
forwarding, then reopen the same session without a repeated create or tool call.

Use the existing shared handlers without alternate implementations or external
network services. The fixture is synthetic and does not invoke a model. Emit
only explicitly executed partial checks after their assertions pass; missing,
duplicate and failed cases must prevent a passing artifact. Cross-check their
API/channel IDs against the independent capability inventory. Do not change the
strict completion validator or claim candidate-image/model acceptance.

Acceptance: evidence accounting negative tests, Windows workspace/acceptance
checks and Linux Podman production-browser fixtures. Registration #638 already
covers this bounded Change PR under INC-2026-0087. Lead reviews/merges; exhaustive
capability coverage and the final composed actual-model run remain open.

The shared MCP serializer currently omits the enabled flag. These assertions
cover live toggling, not disabled-state restoration after host restart; that
persistence gap remains open alongside the broader settings acceptance.

## Bounded follow-up: one initial connection per tab

The retention gate exposed a shared TabManager startup race: a foreground tab is
published before its asynchronous user/project sync completes. An immediate tab
activation sees it as disconnected and connects; sync completion then connects
again, detaching that socket and sending another create/resume. This affects the
shared native and workspace host, so fix it in the shared manager, without a
browser-only workaround or weakening the one-establishment acceptance assertion.

Track foreground initial sync before publishing a tab. Activation may select the
tab while sync is pending but must leave connection ownership to that completion.
Background restored tabs retain lazy activation. Closing, explicit disconnect,
manual reconnect and manager disposal invalidate the pending initial connection;
a late completion must not reconnect cancelled or removed tabs. Preserve the
existing sync failure fallback and manual reconnect behavior.

Acceptance: deferred-sync tests exercise both create and resume, successful and
failed sync, repeated activation, cancellation and background lazy activation.
Linux Podman production-browser retention must still show one establishment,
one submission and no interrupt across zero pages and five reattachments. The
upstream is synthetic; this change does not close full actual-model parity.

## Bounded follow-up: isolate browser authentication from open sessions

Exact-image credentials acceptance exposed two writers using the same private
`workspace-sessions.json`: browser cookie digests (an array) and the open-tab set
(a versioned object). Creating a tab overwrites the authentication file, so the
next CLI startup rejects the browser session data before serving health.

Keep the established open-tab filename and give browser authentication its own
`workspace-browser-sessions.json`, selected by the existing `SESSION_FILE`
constant in the CLI and explicit credential reset. Service credentials and the
open-tab format remain intact. Existing pre-fix cookies need one new browser
login after upgrade; loopback auto-login performs it through normal upstream
authorization. Do not reconstruct overwritten cookies or accept invalid auth
state. Subsequent restart/recreate must retain the same cookie.

Fix the credentials acceptance marker to the shared turn helper's existing
`acceptance-<uuid>` contract, so the real conversation reaches the model.
Acceptance: an integrated auth/open-tab persistence test fails on the shared
filename, then verifies independent formats, old-cookie validation, tab recovery
and reset isolation. Run the strict canonical Linux Podman credentials suite on
an exact production image: registration, actual conversation, unchanged cookie
and credentials after restart, same conversation/model memory, recreate without
bootstrap, and foreign Origin/Host rejection. Preserve the failed original run.
Full capability parity/composed deployment remains open.

Implementation review: [#695](https://github.com/dykamino-coder/gpui-kamin-ide/pull/695).
Refresh by normal merge from main `118beaad`, retaining the independent MCP, LSP
and POSIX teardown follow-ups. Rebuild the exact production image and repeat
credentials/model acceptance after this refresh; older image results do not
verify the combined candidate.

## Bounded follow-up: plugin LSP browser execution evidence

Exercise the production browser transport and shared plugin LSP manager with an
independently instrumented stdio peer installed in an isolated fixture HOME.
Discover the plugin through `listLspServers`, route all four advertised LSP tools
through upstream MCP frames with zero browser pages, and check both returned
results and the peer's received document/position/root parameters. Update a real
file and verify didChange and diagnostics. Same-project tabs must share a process;
another project must use a separate process. Through `restartLspServers`, verify
that old processes exit and subsequent tools initialize fresh peers. Closing the
last project tab must stop only its peer, and host shutdown must stop the rest.

Maintain a reviewed evidence mapping for the two browser API/invoke pairs and
four tool/handler pairs. Emit sanitized partial evidence only after assertions
complete, rejecting unknown, missing, duplicate and failed cases. Validate IDs
against the independent capability inventory. The upstream and language server
are synthetic; the production host, browser bundle, process ownership and tool
handlers are real. The fixture runner builds current sources in Linux Podman,
so these artifacts cannot claim exact candidate-image or actual-model acceptance.
Keep the full-parity validator unchanged and INC-2026-0087 open.

Acceptance: collector integrity tests, both workspace typechecks, declared
capability gate, workspace suite and complete production-browser fixture suite
in Linux Podman. This bounded Change PR is prepared by bridgeweb for Claude
lead review/merge under the existing registration #638.

Implementation review: [#696](https://github.com/dykamino-coder/gpui-kamin-ide/pull/696).

Validation dependency: two source-fixture runs reached the final terminal tests
but retained idle Bash PTYs on base main `f121842e`. Keep the incomplete runs;
do not force a successful test-worker exit. Integrate the existing shared POSIX
disposal candidate [#694](https://github.com/dykamino-coder/gpui-kamin-ide/pull/694)
by normal merge for a combined exact-head gate, rather than copying its logic.
Lead merged #694 and #693 into main during validation. Integrate that fresh main
by normal merge, retaining both existing follow-ups. The LSP additions do not
change the disposal fix; rerun the combined exact-head workspace/browser gates.

## Bounded follow-up: terminate unresponsive owned POSIX terminals

A Linux Podman workspace run printed all assertions but retained three idle Bash
children of its PTY test worker. Current shared disposal sends TERM to the owned
process group and HUP to its shell, without waiting or escalation. Independently
reproduce ignored TERM/HUP before attributing the observed hang to this mechanism.

Keep disposal in the shared native/workspace PtySessions service. Remove a closed
session from writable handles immediately. Preserve the initial TERM/HUP attempt,
then after a short bounded grace period force-kill only its still-owned POSIX
process group and shell. Cancel escalation on that PTY's exit event, so a completed
session never receives a late signal. Repeated disposal must be idempotent; other
live terminals and ordinary browser-page detach must retain their current policy.
Windows retains its existing taskkill tree cleanup. This does not promise cleanup
of descendants that deliberately detach into an independent process session.

Acceptance: a real Linux shell that ignores TERM and HUP must exit after explicit
kill, tab close, tab disconnect, Disconnect and host shutdown. Independently read
fixture-owned PID files, verify another terminal remains live where applicable,
and use only those recorded PIDs for regression cleanup. Fake-clock shared-service
tests verify graceful exit cancellation and one escalation per disposal. Run the
workspace and production-browser fixtures in Linux Podman; synthetic upstream
acceptance and this bounded teardown proof do not close full model/parity gates.

Controlled Linux Podman result: the ignored-signal shell survives the old code's
7-second deadline (exit 1), but exits in approximately 315 ms with escalation.
The first fixture attempt omitted required dimensions and failed validation;
that setup failure is not evidence for the product defect. Full candidate
validation is recorded separately in the linked Change PR.

Implementation: [#694](https://github.com/dykamino-coder/gpui-kamin-ide/pull/694).

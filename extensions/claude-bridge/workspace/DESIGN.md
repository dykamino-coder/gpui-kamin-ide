# Browser workspace client design

Task: [INC-2026-0087](../runtime-issues/INC-2026-0087.md).
Status: implementation in progress; the README records the tested increment.
Source and rules baseline: `9f2db23aa2ce26246d7aee68b80ce68bec97f7f6`.

Implementation sequencing note: merged PRs #639, #640 and #641 provide the
authenticated HTTP/WebSocket boundary, shared BridgeHost composition and browser
entry. An explicit build-time VS Code adapter makes the client testable without
launching KaminIDE. Browser services and host-owned presentation state follow.
These are bounded increments of the architecture below, not a deployable
workspace or a claim of full parity. The README records its exact limits.

## Problem and completion contract

The Bridge server owns Claude CLI PTYs, while the client owns project access
and executable plugin components. Serving the dashboard alone does not provide
a client. Add `extensions/claude-bridge/workspace`, a Node client whose local
machine is a Linux container with persistent `/workspace`. Serve the existing
webview components to an ordinary browser, preserving the Kamin bridge API and
server protocol.

Owner requirement, 2026-11-09 (verbatim): «важно что бы страница в браузере
тонкой была и если ее закрыть и открыть работа с агентом не прервется».
The browser is only a view. All authoritative state and execution belong to the
container host, including sessions, connections, transcript, streaming turns,
tool status, permission prompts, queued messages, files/watches and terminal
scrollback. A page can disappear with zero attached browsers while work proceeds.
Reopening attaches to the same running workspace and renders its current state.
The existing browser entry alone does not yet satisfy this contract.

Completion means reviewed Change PRs with green applicable CI, an exhaustive
capability gate, and real headless browser acceptance in Podman. The Claude lead
merges implementation. Registration may be merged by its author. Release,
version changes, image publication and production deployment are outside scope.

## Architecture

```text
browser: existing webview App + bridge-shim
  | acquireVsCodeApi-compatible browser adapter
  | authenticated same-origin WebSocket: invoke / send / event / invoke-reply
workspace: HTTP host + environment services
  | shared BridgeHost, IPC handlers, TabManager, ConnectionManager
  | shared file/shell tools, hook relay, MCP manager, LSP, monitors, sync
  | existing authenticated Bridge protocol
bridge server: Claude CLI PTYs and session-local proxy plugin
```

The workspace build imports the real `extension/src/bridge-host.ts` and
`extension/src/main` modules. It must not copy handlers into a new package.
An explicit environment adapter supplies the small VS Code-shaped surface the
shared host consumes: workspace folder, extension context/global storage,
dialogs, clipboard, notifications, external URLs and editor documents. The
workspace bundle aliases this boundary at build time, while the desktop build
continues to resolve the real extension APIs. Unsupported adapter accesses throw
an actionable error and fail tests. Avoid a permissive catch-all proxy.

The existing `@kaminide/host-compat` IPC facade remains the shared registration
and dispatch point. HTTP/browser environment services supply functions otherwise
owned by KaminIDE: file tree, editor I/O and watching, selection context, local
PTY, downloads/uploads and browser integration. These are environment adapters,
not alternate Read/Write/Edit/Bash implementations. If common composition needs
a small lifecycle port, extract it once and have both hosts consume it.

There is no architectural deviation from the lead's default. The concrete
adapter choice avoids implementing a second plugin runtime. The server retains
ownership of CLI sessions in both packaging modes; combined mode co-locates
processes without merging their protocols or responsibilities.

## Browser transport and UI

Add a browser entry to the existing webview build that mounts `App`, loading the
browser transport before `bridge-shim`. Reuse chat, customization, tool panels,
file panel and their signals/components. Do not maintain another chat UI.

The browser adapter implements `postMessage`, `getState` and `setState`. It sends
the existing frame shape over WebSocket and delivers inbound frames to the same
transport listener. No user-supplied `window.postMessage` frame may impersonate
the authenticated socket. Request IDs remain scoped to a browser connection;
generation protects document identity. Replies return only to their requester,
while workspace events reach subscribed views. Bound payload sizes, reject
malformed frames, and return errors for unknown invokes instead of null success.

Mount the existing UI after authentication and host snapshot synchronization.
Keep only disposable render state and transport bookkeeping in the page. Move
authoritative reducers, queues, prompt state and heavy processing to the host,
reusing the existing logic rather than introducing a second UI. Measure the
production bundle and page JS heap in acceptance and report both in the PR.

On socket loss, detach delivery only and reconnect with backoff. A new document
or second tab attaches to the same host state. Synchronization delivers a coherent
snapshot of transcript, active turn/tool state, pending prompts, open files and
watches, and terminal scrollback, followed by ordered live updates. A host revision
and event cursor must close the snapshot/live race without gaps or duplicates.
The host owns logical view state independently of a socket or document generation.

Browser reload is never permission to replay a mutation. Use stable operation IDs,
host acknowledgements and deduplicated outcomes to resolve lost replies without
executing a submission twice. Host-owned queued messages survive detachment;
accepted user messages must neither disappear nor be submitted twice. The merged
transport currently rejects lost invokes as unknown outcomes and never replays
them; that is a useful boundary but is not the completed delivery contract.

Browser adaptations must preserve useful behavior: upload bytes for attachments,
download files for save/export, Clipboard API for text, browser audio capture for
microphone, and a workspace path chooser for container directories. Container
paths must never be interpreted as paths on the browser user's computer.
Editor selections are produced by the file panel and flow through the existing
editor context API. Native window/update controls need an explicit browser
behavior or should be absent from the browser entry's controls; an inert API
stub does not establish full parity. Their exact rows remain required in the
capability manifest.

## Authentication and ownership

Default Node bind is `127.0.0.1`. Container listeners may opt into `0.0.0.0`,
while Podman publishes the browser port on host loopback by default. Document
explicit opt-in for remote binds and a TLS reverse proxy.

Reuse an existing server API token, validated through
`POST /api/dashboard/tokens/resolve`, or the server dashboard login/session
endpoints for administrator authentication. An auth-disabled dashboard response
alone is never proof of permission to run container commands. Token identity is
pinned to the workspace instance: a different valid server token must not gain
access to an already bound workspace. Dashboard access is administrative and
must resolve/select the workspace's configured client token before connecting
the shared client. Do not add a separate workspace password database.

After validation, issue an opaque expiring HttpOnly SameSite session cookie;
use Secure behind HTTPS. Keep upstream credentials server-side, out of URLs,
HTML, browser storage and logs. Revalidate expiry/revocation and close sockets
on logout. Check exact allowed Origin and Host on HTTP mutations and WebSocket
upgrade; refuse cross-origin upgrades, absent/invalid auth, and unbounded input.
Tests must cover token isolation, expiry/revocation, forged messages and origin
checks. An authenticated workspace grants shell access to that container; the
filesystem boundary is the container, not a claimed sandbox around `/workspace`.

## Persistent state and lifecycle

Set cwd and the initial project root to `/workspace`. Treat HOME and project
paths as ordinary container paths: file tools and shell have
container-wide access, including absolute paths and symlink destinations outside
`/workspace`. Isolation is supplied by the container and its mounts, not by a
directory jail in the client. Keep workspace user HOME and shared client global
storage in explicit persistent volumes. That HOME holds
client plugin configuration, not a Claude CLI login. The server alone owns CLI
auth, including its `.claude/` and `.claude.json`; separate-server mode never
mounts the server's auth into the workspace. Combined mode preserves the owned
server auth separately from client HOME. Give the unprivileged runtime user
write access without changing arbitrary host directories. Do not mount host
home, Podman socket or unrelated worktrees.

One process owns one client runtime. Multiple browser tabs view that runtime,
not separate client identities. Closing/reloading a page, socket loss, network
failure, logout or opening another tab releases only delivery subscriptions.
It must never call disconnectAll, abort work, clear host view state or dispose
file watches and PTYs. Those resources belong to host-side logical views and
remain active with zero browsers attached. Re-attach refreshes file/tree snapshots
and restores their subscriptions before live updates continue.

Explicit End session/Disconnect and host shutdown are the terminal boundaries.
An explicit Disconnect must dispose the affected local connections and abort
their work before removing them from tracking. Automatic transport reconnects
retain resumable work. Shutdown disposes watchers, PTYs, MCP/LSP/monitor children,
timers and client connections; combined
mode forwards signals to both owned processes and reaps them. Persisted sessions
can be resumed after restart; do not automatically repeat a pending command.
Final client disposal aborts in-flight local tool controllers and owned shell
processes, and aborts the MCP manager's HTTP/SSE requests before tearing down
transports. It detaches from the server without ending its CLI sessions. This
terminal path is distinct from transient upstream disconnect/reconnect, which
continues to retain in-flight work for acknowledged result delivery.

## Packaging and bounded changes

1. Shared-host adapter and workspace HTTP/WS skeleton, identity/origin checks,
   deterministic transport tests and an initial exhaustive inventory that reports
   real gaps. A partial PR must label gaps and must not advertise full parity.
2. Same webview browser entry, container files/editor/PTY/browser adapters and
   executable per-capability fixtures. Rebuild committed desktop artifacts when
   their inputs change. Extend applicable CI/verify scripts to run the new package.
3. Podman-only image/runbook, separate-server compose service and combined target,
   persistent volume smoke tests and complete real acceptance. A final parity
   gate must fail on every remaining gap before claiming the owner request done.

Every Change PR starts from fresh `origin/main`, references this incident, and
states its bounded result/dependencies. No `git worktree add`, publication or
version bump is needed. Use unique project/container/volume names for acceptance;
never run prune or remove other workloads. The existing
`server/docker-compose.yml` is the integration reference; preserve its default
service behavior when adding an opt-in workspace service. Combined packaging
must be separately selectable, not silently start a second server beside an
existing deployment.

## Capability matrix and automated proof

The committed executable manifest will have one row per API member/channel or
client tool: name, kind, channel (including dynamic pattern), implementation,
behavioral fixture and assertions. Generate the inventory from the TypeScript
`KaminBridgeApi` interface, INVOKE/SEND/EVENT maps, bridge-shim specials, actual
IPC registrations, and `toolRegistry`. Do not maintain a hand-counted allowlist.
Compare both directions: new source capability without a tested row fails, as
does a stale row. `invoke` passthrough is exercised separately and cannot excuse
a missing named method. Events require producer/consumer assertions; a subscribed
callback alone is not a test. Test actual environment services and shared
handlers, not a fake blanket success responder. Fixtures must name observable
effects, cancellation, errors and cleanup as appropriate.

| Inventory group | Intended implementation | Required proof |
| --- | --- | --- |
| Tabs, connect, chat, permissions, submissions, sessions | Shared TabManager/session IPC and host state/event pipeline | Create/resume/switch/close; host queue once; pending permission after reopen; complete turn snapshot; two tabs |
| JSONL, streaming, subagents, tree, usage, commands | Shared connection dispatch/event sink | Routed events, pagination/segments, correct tab, export/download |
| Config, layout, notifications, theme, logs, sync, base prompt | Shared config/sync handlers with browser environment | Durable settings, restore, notifications/logs and scoped sync |
| Files, editor selection, path chooser, watches | Host-owned container services and existing file panel | Real read/binary/write/rename/list/watch; edit context; watches and refreshed snapshots after re-attach; no mutation replay |
| Interactive PTY and shell listing | Host-owned container PTY service and scrollback | Input/output, resize, exit/kill; zero-browser lifetime; scrollback after re-attach |
| Read, Write, Edit, Glob, Grep, NotebookEdit | Exact shared toolRegistry handlers | Filesystem mutations/search/errors against temporary workspace |
| Bash, PowerShell, BashOutput, KillShell | Exact shared shell handlers; image supplies required executables | Cwd/env, stdout/stderr, background output, cancellation |
| WebFetch, EnterWorktree, ExitWorktree | Exact shared handlers | Local HTTP/Git fixtures; owned disposable repository only |
| Agent/Team/Task tools, SendMessage | Exact shared registry and bridge routing | Protocol fixtures, task state/results, identity isolation |
| AskUserQuestion, plan tools, Skill, PushNotification, ShowWidget, Todos, ToolSearch | Exact shared handlers and existing UI | Request/response and rendered event assertions |
| Hooks, including approval and plugin relay | Shared hooks and hash approval store | Unapproved command inert; approve/run; changed hash requires review |
| MCP config, OAuth, registry, elicitation, resources/prompts/progress | Shared MCP manager and IPC | Local stdio/HTTP fixture, tool call, resources/prompts and cancellation |
| Skills/agents/styles, plugins/marketplaces/options | Shared discovery/install/sync handlers | Local fixture install/update/remove, namespace/options and secret redaction |
| LSP list/restart and four LSP tools | Shared plugin LSP manager | Real fixture stdio server; diagnostics/hover/definition/references; cleanup |
| Monitors and activity events | Shared monitor manager | Start/output/status/stop; tab/project scope and cleanup |
| Attachments, clipboard, downloads, external links, mic | Browser adapters + shared byte/file operations | Upload/drop/paste, bytes round trip, browser permission/audio fixtures |
| Version, update/window/VS Code compatibility methods, generic invoke | Explicit browser environment semantics | No silent success stubs; controls match capabilities; unknown channel errors |

Existing deliberate plugin boundaries in `PLUGIN_HARNESS.md` remain unchanged
(LSP socket declarations, packaged MCP bundles and channels). Linux image
dependencies must satisfy shell parity, including PowerShell if its API remains
advertised; do not silently claim an installed executable that is absent.

## Acceptance

### Browser transport increment

The independent browser entry reuses `App` and the same shim/channel tables.
The browser build uses deferred HTTP assets instead of the desktop single-file
format, because inlining dynamic imports evaluates renderer initializers before
authentication and transport installation. This is a packaging difference, not a
second UI or a fork of handlers. This merged increment still has document-local
UI state; migrating authoritative state to the host remains required. Mount
currently occurs after socket open, so there is no startup command queue.
Lost invokes reject as unknown outcomes; commands are never buffered or replayed.
Reconnect requests the existing UI read/session resync. Private socket delivery
does not register the native window-message receive path in browser mode.

The production-bundle Chromium smoke proves authentication, shared App composition,
forged-reply rejection, reconnect without document replacement/mutation replay,
revocation and logout against the real HTTP/WS boundary with an enumerated fixture
backend. It intentionally does not count toward the exhaustive handler matrix or
live Podman gate below. Environment adapters/executable composition remain next.

### Full workspace gates

Use an isolated Podman project and headless Playwright, with synthetic content
and private raw captures. Record source SHA, dependency locks, image ID, container
names, commands, browser version, test results and volume names in the acceptance
manifest. Never record credentials. Two layers are required:

- Deterministic integration: real host, real shared handlers, fake upstream
  protocol peer; every capability row, auth/negative cases, reconnect and
  lifecycle cleanup. It can run without a paid model identity.
- Live acceptance: real Bridge server and CLI, browser login, create session,
  complete a chat turn, Read/Write/Edit and Bash in `/workspace`, invoke a local
  fixture MCP tool, run an approved hook, open/edit a file through the file panel,
  attach selection, then reload/reconnect/resume. Assert observable file bytes
  and UI state, rather than relying only on model prose. Recreate owned
  containers with the same volumes and prove files/settings/auth survive.

Both layers must also exercise the thin-page lifecycle contract:

- Start a long streaming turn with a running Bash and external MCP call; close
  the page mid-turn, wait with zero browsers, then reopen. Assert the work
  continued or completed, transcript/tool state are complete, and message IDs
  and transcript entries contain no duplicates or omissions.
- Leave a permission prompt pending, close and reopen, then answer it once and
  verify the original operation proceeds. A second simultaneously attached tab
  receives the same prompt/result and cannot settle it twice.
- Keep two tabs attached and detach either one without interrupting the other
  or agent work. Restore open files, live tree/editor watches and terminal
  scrollback after re-attach; external changes remain observable. No write,
  terminal input, permission answer or user submission is replayed.
- Separately send explicit Disconnect while Bash and external MCP are in flight,
  then shut down. Verify cancellation, owned process termination and natural
  host exit. Also verify shutdown alone with no browser attached.
- Report production JS bundle sizes (raw and compressed) and headless Chromium
  JS heap for initial view, a long turn, zero-browser continuation and re-attach.
  Compare repeated detach/attach runs for retained page resources. All sessions,
  queues and execution must remain host-owned regardless of these measurements.

Local fixture MCP/LSP/hook/monitor processes exercise the executable plugin
boundary without requiring a private marketplace. A mock model or protocol peer
does not count as the live chat gate. Acceptance uses a lead-approved server
with an existing CLI identity and a separately issued client token. The agent
never reads or copies that server's auth volumes. If a candidate needs a changed
server build, ask the lead to select its deployment before running acceptance;
never modify a daily server. Missing server access is reported to the lead as
a concrete prerequisite; it never becomes a request for owner testing.
Complete applicable package typechecks/tests/builds, regenerated artifact checks,
event routing check and GitHub CI on the final candidate. No Windows GUI launch
or focus is necessary for this new browser-only contract.

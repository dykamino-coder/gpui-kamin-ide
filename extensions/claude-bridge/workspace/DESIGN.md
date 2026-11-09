# Browser workspace client design

Task: [INC-2026-0087](../runtime-issues/INC-2026-0087.md).
Status: proposed implementation plan; no capability below is claimed delivered.
Source and rules baseline: `9f2db23aa2ce26246d7aee68b80ce68bec97f7f6`.

Implementation sequencing note: the first Change PR isolates the authenticated
HTTP/WebSocket boundary as a library, before wiring the shared runtime and UI.
This makes the shell-access authentication boundary independently reviewable.
It is a smaller first increment of the architecture below, not a deployable
workspace or a claim of full parity. The README records its exact limits.

## Problem and completion contract

The Bridge server owns Claude CLI PTYs, while the client owns project access
and executable plugin components. Serving the dashboard alone does not provide
a client. Add `extensions/claude-bridge/workspace`, a Node client whose local
machine is a Linux container with persistent `/workspace`. Serve the existing
webview components to an ordinary browser, preserving the Kamin bridge API and
server protocol.

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

Until the socket opens, queue a bounded set of startup calls. On disconnect,
reject pending calls, preserve the shared host and server sessions, reconnect
with backoff and resynchronize tabs/history. Browser reload is a new document,
not permission to replay writes or submissions. Persist only UI state in browser
storage; credentials and host runtime state belong to the host.

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

Set cwd and the initial project root to `/workspace`. Keep user HOME, shared
client global storage and CLI auth in explicit persistent volumes. Preserve both
`.claude/` and `.claude.json` by mounting an appropriate home directory rather
than assuming the former contains all auth. Give the unprivileged runtime user
write access without changing arbitrary host directories. Do not mount host
home, Podman socket or unrelated worktrees.

One process owns one client runtime. Multiple browser tabs view that runtime,
not separate client identities. Disconnecting a browser releases its watchers
and subscriptions without destroying a live conversation. Shutdown disposes
watchers, PTYs, MCP/LSP/monitor children, timers and client connections; combined
mode forwards signals to both owned processes and reaps them. Persisted sessions
can be resumed after restart; do not automatically repeat a pending command.

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
| Tabs, connect, chat, permissions, submissions, sessions | Shared TabManager/session IPC and WS pipeline | Create/resume/switch/close; queued text once; permission decision; reconnect/replay |
| JSONL, streaming, subagents, tree, usage, commands | Shared connection dispatch/event sink | Routed events, pagination/segments, correct tab, export/download |
| Config, layout, notifications, theme, logs, sync, base prompt | Shared config/sync handlers with browser environment | Durable settings, restore, notifications/logs and scoped sync |
| Files, editor selection, path chooser, watches | Container environment services and existing file panel | Real read/binary/write/rename/list/watch; edit context attached to chat |
| Interactive PTY and shell listing | Container PTY service | Input/output, resize, exit/kill, lifecycle ownership |
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

Local fixture MCP/LSP/hook/monitor processes exercise the executable plugin
boundary without requiring a private marketplace. A mock model or protocol peer
does not count as the live chat gate. Missing CLI identity is reported to the
lead as a concrete prerequisite; it never becomes a request for owner testing.
Complete applicable package typechecks/tests/builds, regenerated artifact checks,
event routing check and GitHub CI on the final candidate. No Windows GUI launch
or focus is necessary for this new browser-only contract.

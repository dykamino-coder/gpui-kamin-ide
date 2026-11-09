# Browser workspace client design

Task: [INC-2026-0087](../runtime-issues/INC-2026-0087.md).
Runtime overview: [README.md](README.md). Podman runbook: [PACKAGING.md](PACKAGING.md).
Status: implementation in progress; the current live suite passed, while full parity and final composed acceptance remain open.

## Problem and completion contract

The Bridge server owns Claude CLI sessions. Its client owns project operations,
shells, executable plugins and UI delivery. A dashboard alone cannot replace that
client. The browser workspace runs the same client host in a Linux container,
with persistent `/workspace` and the existing webview App served to a browser.

Owner requirement, 2026-11-09 (verbatim): «важно что бы страница в браузере
тонкой была и если ее закрыть и открыть работа с агентом не прервется».
Sessions, connections, tools, queues, prompts, files/watches and terminal state
must belong to the host. Zero attached browsers must not interrupt work.
Reopening must restore current transcript, streaming/tool/prompt state, files
and scrollback without repeating a mutation. Bounded recovery implementations
exist; full host-authoritative page/FilePanel state remains an acceptance gate.

The tracked agent-controlled **PREVIEW panel with HMR** follow-up remains open
until its source and executed acceptance are incorporated.

Completion requires reviewed Change PRs with applicable green CI, an exhaustive
capability gate and actual headless browser acceptance in Linux Podman. The
Claude lead merges implementation and final documentation. Registration #638
was merged separately; it did not accept the product. Release, version bumps,
image publication and owner-performed testing are outside this task.

## Architecture

```text
browser: existing webview App + same KaminBridgeApi/channel tables
  | authenticated same-origin WebSocket: invoke/send/event/invoke-reply
workspace HTTP host: browser effects + files/views + host PTYs
  | shared BridgeHost + IPC registry + TabManager + ConnectionManager
  | exact existing toolRegistry + hooks + MCP + plugin LSP + monitors
  | existing authenticated Bridge protocol
Bridge server: Claude CLI PTYs and session-local proxy plugin
```

`src/cli.ts` composes `createBrowserEnvironment`, `createSharedBackend`,
`withBrowserServer`, `WorkspaceFiles` and `TerminalWorkspaceBackend` before
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
Lost invoke results are unknown outcomes, never automatically retried mutations.
These mechanisms do not complete the host-authoritative transcript reducer and
queued-message migration; browser App still processes authoritative page state.

Logical FileViews keep file/tree subscriptions alive with zero sockets, associate
an opaque view identity with the page and refresh snapshots on attach. The host
currently adopts detached views into the attached UI. Full per-chat file layout,
drafts, selections, editor context and PREVIEW state still require host projection
and multi-page acceptance. The inventory classifies native IDE
editor/reveal/openInVscode separately; the existing Bridge FilePanel/editor-context
contract remains required.

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
| Files and editor context | WorkspaceFiles and existing FilePanel | Read/binary/write/rename/tree/watch, selections, detached updates/layout recovery |
| Interactive terminals | Host PTY registry/headless screen and same terminal UI | Input/output/resize/exit/kill, zero-view work, scrollback and TUI behavior |
| Hooks | Shared approval/hash store and relay | Inert before approval, real execution, changed hash reapproval |
| MCP and plugin discovery/configuration | Shared manager, IPC, sync and registry | Local stdio/HTTP tools/resources/prompts, OAuth/elicitation/cancellation/cleanup |
| LSP and monitors | Shared plugin managers | Real fixture server diagnostics/hover/definition/references, restart and cleanup |
| Settings and browser effects | Pinned administration proxy and browser adapters | Durable settings, export/upload/clipboard/notifications/navigation/audio |
| Compatibility/generic invoke/events | Explicit environment semantics and event routing | Producer/consumer assertions, unknown-channel errors, no silent stubs |
| PREVIEW with HMR | Unimplemented follow-up | Agent-controlled project preview and observable hot updates |

Existing plugin boundaries in PLUGIN_HARNESS.md remain unchanged. The image
supplies the executable dependencies it advertises, including PowerShell.
Native editor LSP fallback commands reject explicitly; this does not replace
testing the shared plugin LSP path.

At main `4bf69698`, #650 supplies the bidirectional source inventory and
mutation tests. Its readiness manifest declares only `host-terminal`,
`title-regeneration` and `finished-toast-route`, all implemented. Thus the current
`test:parity` passes even though host-page state, full FilePanel state and
PREVIEW/HMR remain open. Provider/test-file existence is checked; executing an
observable contract for every classified operation is a separate outstanding
gate. Extend readiness to encode the missing completion requirements rather
than interpreting the current success message as full parity.

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
fixture MCP tool and approved hook; FilePanel read/edit/selection; close during
long Bash/MCP work with zero browsers; reopen with complete nonduplicated
transcript/tool state; retain a permission and answer it once across two pages;
restore file watches/layout and terminal scrollback; explicit disconnect and
shutdown cancellation; persistence after owned container replacement; PREVIEW/HMR.

Record source/dirty state, locks, image ID/source, browser version and executed
scenario results. Measure all production JS chunks raw/gzip/Brotli and post-GC
heap at startup, long turn, zero-view continuation and repeated re-attach.
Empty-App figures do not prove long-turn retention or a thin page. Raw logs,
transcripts, credentials, screenshots and traces stay outside the public repo;
publish only sanitized summaries. FAIL stays failure; BLOCKED means unrun;
XFAIL remains incomplete acceptance. A later source fix needs an executed rerun.

The task stays open until full host state, strict behavioral parity, PREVIEW/HMR
and the final composed live candidate pass. The agent performs acceptance and
reports exact residual blockers to the lead; the owner is never asked to test.

## Latest executed live subset

The [#653 report](https://github.com/dykamino-coder/gpui-kamin-ide/pull/653)
records **15/15 PASS, exit 0**, run `2026-11-09T13:58:59.676Z`, runner
`fbfdd144ba29a73bcecf09626fa3473f6cbb0abf`, image
`d9c19f3a32b573989f159a17a8b89a5e8b99e22d6d9f0cf7f165001d644f951b`,
image source `33bbb7cc8651824334003a2b615b051cce2e9602`.
The production-source diff against main `91b757d0` is empty across workspace/src,
workspace/Dockerfile, webview and extension. The runner changes only the harness.
The two former FilePanel/Allow-click failures passed in this executed rerun.

Its assertions cover image contract, credential storage, upstream health,
workspace start/login, chat, file tools, Bash, MCP, hook, FilePanel,
close-mid-turn, reopen-permission, two browser tabs and browser metrics.
Chromium `158.0.8127.18`; 34 production JS chunks total 2,268,318 raw bytes,
732,461 gzip and 591,675 Brotli (sums of individual files). Post-GC used heap:
5,072,860 bytes at startup and 11,757,924 bytes after the suite. These are the
suite's recorded points, not a complete long-turn/repeated-attach retention study.

This is executed subset evidence supplied by bridgeweb2, not a rerun by the
final documentation author. It predates the title/toast hooks integrated through
#650 (#654 was closed as covered) and does not prove full host-owned
page/FilePanel state, PREVIEW/HMR, host/container restart, or every desktop tool
behavior. The committed readiness manifest does not yet encode these remaining
completion requirements; the inventory's passing state is a narrower result.

# Browser workspace client

Task: [INC-2026-0087](../runtime-issues/INC-2026-0087.md).
Design and remaining acceptance: [DESIGN.md](DESIGN.md).
Podman build, startup and acceptance: [PACKAGING.md](PACKAGING.md).
Credential storage, automatic loopback access and reset: [CREDENTIALS.md](CREDENTIALS.md).

The workspace is a Node Bridge **client** whose local machine is a Linux
container. Its executable composes the same BridgeHost, client tool handlers,
plugin hooks, MCP/LSP and monitors used by KaminIDE. It serves the existing
current ChatRoot, Console and Agents components inside a thin browser shell,
through an authenticated HTTP/WebSocket adapter implementing KaminBridgeApi. There is one UI and one tool implementation.
Claude CLI and its login remain on the Bridge server.

Status: bounded implementations are available; the full owner request remains
open. Capability classifications and synthetic browser tests are not proof of
full live parity. The final acceptance evidence must identify the executed
source revision and image, including failures and unrun scenarios.

## Runtime composition

`src/cli.ts` builds the shared runtime, pins server administration requests to
its configured upstream, installs browser environment services, then wraps it
with persistent workspace-shell state, `WorkspaceFiles` and `TerminalWorkspaceBackend`. The resulting host serves
the production browser bundle. Shutdown disposes terminals, file watches and
the shared runtime; a lost browser socket only detaches delivery.

`createSharedBackend` sets process cwd and initial project root to `/workspace`
and supports one shared runtime per process. Project files and client settings
persist in project/HOME volumes. Tools can access ordinary container paths,
including absolute paths and symlink destinations outside `/workspace`.
The container and its mounts provide isolation; `/workspace` is a project root,
not a filesystem jail. Client HOME must remain separate from server CLI auth.

New and resumed sessions default missing or blank browser cwd to the configured
workspace root. A selected project must be an existing container directory;
relative selections resolve against that root. Each session explains to the
model that files, shell and client tools live in the workspace container, and
includes the selected directory alongside the saved project instructions.

The asset handler snapshots only the trusted browser build's entry and supported
assets before listening. It rejects linked/special files and bounded-size/depth
violations, and returns 404 for unknown paths, sources, maps and traversal.
Application routes never publish project files. Asset changes require restart.

## Authentication

The configured Bridge token is validated with the existing server
`/api/dashboard/tokens/resolve` endpoint. Login issues an opaque HttpOnly
SameSite cookie. Identity is pinned: browser config, connect and resume requests
cannot substitute another server/token. Host and Origin must match the configured
origin; revocation, expiry or an upstream validation outage closes access.
An auth-disabled dashboard response alone grants no workspace access.

On first start the service registers its own Bridge client token, or imports a
provided bootstrap secret once when registration requires authorization. Its
private HOME volume stores that token and a generated browser secret in a 0600
file. Session cookies survive restart and recreation with the same volume.
Workspace ConfigStore still omits credentials from project settings, and browser
events redact them. Dashboard-admin login remains a separate unimplemented path.

Node defaults to `127.0.0.1:3460`. The image opts into a container-wide listener;
Compose publishes the browser port on host loopback. Loopback visits receive an
HttpOnly SameSite=Strict session cookie automatically after Host/Origin checks.
Other origins require explicit `WORKSPACE_ALLOW_REMOTE=1` and retain token
sign-in; HTTPS origins use Secure cookies. See the runbooks for reset and remote
publication configuration.

## Browser lifetime and host state

Owner requirement (2026-11-09): closing the browser must not interrupt agent work.
The host owns sessions, connections, foreground/background Bash, MCP calls,
pending prompts, submission receipts, file subscriptions and local PTYs with
zero browsers attached. Reopening synchronizes shared transcript/streaming caches
and prompt/status state, then continues live delivery. Socket loss, reload,
logout and a slow page do not terminate work. Explicit tab End, Disconnect and
host shutdown are terminal boundaries.

Lost invokes reject as unknown outcomes; the browser never automatically retries
mutations. Stable-ID user submissions are deduplicated on the host (up to 10,000
receipts/32 MiB, then explicit rejection). Re-attach uses read synchronization,
not replay of writes or commands. Presentation snapshots remain process-owned;
full host-authoritative transcript reducers and queued-message state remain open.

The browser shell retains session/project navigation, tabs and pins, and a
resizable dock for Claude Bridge, Bridge Console and Agents. Dock placement,
active panels, project expansion and sidebar dimensions live in the host and
synchronize across pages; validated layout operations persist across restart.
The owner removed FilePanel and the project explorer from browser scope. The
existing file services remain available for tools and attachments.

Logical FileViews retain file/tree subscriptions with zero sockets and refresh
watched bytes/directories on re-attach. Identity is an opaque sessionStorage value,
not authoritative file content. There are at most 32 retained views, each with
64 file and 8 tree subscriptions. FilePanel drafts, selections, scroll/undo and explorer projection are outside
the revised browser scope.

Terminals belong to host chat tabs. Spawn is coalesced per tab and re-attach
receives a bounded headless-xterm screen/scrollback snapshot before live output.
Detach does not respawn a shell. Explicit kill/tab close/disconnect/shutdown
terminates owned processes. Limits include 32 records (including pending spawns),
5000 scrollback lines and an 8 MiB serialized snapshot budget; omitted history is
reported. Shells/screens survive browser loss, not host restart. Remaining OSC
palette/pixel-window query compatibility has not been accepted as full TUI parity.

The current ChatRoot and terminal components render the shared host services. Browser
clipboard, downloads, uploads, path dialogs, notifications, links and audio use
explicit environment adapters; missing services fail explicitly. Actual plugin
LSP stays shared; unavailable native editor diagnostic commands reject. Native
KaminIDE editor/reveal/openInVscode features are classified as native host
operations in the matrix; browser attachments use the explicit environment adapter.

PREVIEW/HMR adapters have their own behavioral coverage. The retired FilePanel
is hidden in the browser shell; this layout does not expose an explorer or editor.

## Verification and remaining gates

Install repository-root dependencies with `npm ci --no-audit --no-fund` first.
Then run checks from this directory with a Node version allowed by package.json:

```sh
npm ci --no-audit --no-fund
npm --prefix ../extension ci --no-audit --no-fund
npm run typecheck
npm test
npm run test:acceptance
npm run test:capabilities
npm run test:parity
```

The real shared-runtime integration uses authenticated HTTP/WebSocket transport,
actual tools and temporary project/HOME directories. It verifies pinned identity,
Read/Write/Edit/Glob, Bash, external MCP cancellation, zero-page lifetime,
transcript/stream/prompt recovery, once-only submissions and terminal cleanup.
The LSP integration installs an isolated synthetic stdio plugin and exercises
discovery, hover/definition/references/diagnostics, document changes, shared and
separate project processes, zero-page continuation and identified-session cleanup.
Its upstream protocol peer is synthetic; it is not live Claude acceptance.

Production browser smoke uses the real asset handler and boundary with an
explicit synthetic backend. Linux CI/Podman runs headless Chromium and reports
all shipped JS chunks (raw/gzip/Brotli) and post-GC page heap. Startup metrics
are baselines, not long-turn retention or thin-page proof. Never launch Chromium
or Playwright natively on the shared Windows workstation, even headlessly.

The capability inventory must cover every KaminBridgeApi method, mapped/dynamic
channel and client handler in both directions, with observable behavior tests.
Source classification and named test paths alone do not establish behavioral
coverage. The merged #650 inventory checks API/channel/tool mappings in both
directions; see the [matrix manifests](capabilities/api.json),
[providers](capabilities/providers.json), [requirements](capabilities/requirements.json)
and [inventory extractor](test/capability-inventory.ts).
Current `test:parity` passes its declared provider prerequisites:
host terminals, title regeneration, finished-toast routing and PREVIEW/HMR.
Deleting any of these four requirements fails both classification and readiness
checks. A green command cannot close the owner task.

`npm run test:parity:complete` is the separate strict completion gate. With no
execution receipt it exits 1 and lists missing proof; this is the current expected
result. It requires an individually named passing assertion for every supported
inventory row, seven host-state/lifetime contracts and 22 composed live scenarios.
Reviewed native exclusions need no browser proof. Existing file/context services
remain in the inventory; the removed FilePanel/explorer UI is not reintroduced.
The gate tests run in normal `npm test`; CI retains its existing prerequisite gate.

After the missing suites implement and execute those assertions, validate their
sanitized artifact outside the checkout:

```sh
npm run test:parity:complete -- --receipt /private/acceptance/completion.json
```

The [receipt schema and required IDs](test/full-parity.ts) bind a clean full runner
SHA to the same image revision and immutable image ID, Linux/browser execution,
exit 0 and empty cleanup failures. Each check contains `id`, `status`, `test` and
`assertion`; only PASS with nonempty test/assertion names qualifies. Missing,
unknown, duplicate, failed, skipped, blocked and stale evidence rejects completion.
Failure output omits arbitrary receipt text. Receipts are limited to 4 MiB.
The existing live harness emits a partial `completion.json` beside its summary,
preserving each executed scenario's actual status. It supplies no per-capability
or remaining-contract proof, so even its current 16/16 PASS cannot close this gate.
The validator does not authenticate execution artifacts: remaining producing
suites and exhaustive behavioral assertions stay open. Do not hand-author a
PASS receipt. The schema unit fixture proves validation only; historical live
summaries and source/test-path existence cannot establish full acceptance.

Live acceptance requires a clean runner checkout and an image whose
`org.opencontainers.image.revision` label equals that runner's full HEAD SHA.
A mismatch fails before probing the image or issuing a client token. After
inspection the harness uses the immutable image ID so concurrent retagging
cannot substitute another image. Commit first, then build with that SHA.
MCP evidence records only discovery, call counts and the independent result
check; transcript and file-byte assertions remain mandatory.

The Linux Podman live harness exercises chat, file tools, Bash, MCP, hooks,
session pinning, dock synchronization, close-mid-turn continuation, pending permissions after reopen and two
browser tabs. It retains sanitized summaries outside the checkout. The latest
executed source/image/report must be checked before marking these gates passed;
historical failures are not waived by later source changes. The executed
[#653 report](https://github.com/dykamino-coder/gpui-kamin-ide/pull/653) records
15/15 PASS for its listed scenarios on production sources matching main
`65fa7467`, including #650's title/toast integration; see DESIGN.md for exact
runner/image provenance and limitations. That historical run predates the
current browser shell and does not establish its visual parity.

The owner task remains open for remaining host-authoritative page state,
exhaustive behavior proof and final composed live acceptance. FilePanel state
polish is explicitly outside the revised browser scope.
No release, version bump or image publication is part of this work. Claude lead
reviews/merges implementation and the consolidated documentation PR.

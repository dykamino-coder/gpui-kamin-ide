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

`src/headless.ts` (`startHeadlessWorkspace`) builds the shared runtime, pins
server administration requests to its configured upstream, installs browser
environment services, then wraps it with persistent workspace-shell state,
`WorkspaceFiles` and `TerminalWorkspaceBackend`. `src/cli.ts` reads the
container environment, adds Bridge-token sign-in and the production browser
bundle (`WORKSPACE_UI_DIR` overrides its directory) and loads the same
`dist/headless.cjs`; the UI-less package for other applications is described in
[HEADLESS.md](HEADLESS.md). Shutdown disposes terminals, file watches and the
shared runtime; a lost browser socket only detaches delivery. Preview state is
pushed to every socket as `workspace:preview-state`.

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

## Module Federation preview paths

`preview_mount` accepts a container-local entry such as
`http://127.0.0.1:3101/remoteEntry.js`. For a remote named `demo`, use the
documented development configuration:

```js
output: { publicPath: 'auto' },
devServer: {
  webSocketServer: { type: 'ws', options: { path: '/preview/remotes/demo/ws' } },
},
```

The browser loads `/preview/remotes/demo/remoteEntry.js`; the HTTP proxy maps
that entry and its chunks to the upstream asset paths. WebSocket upgrades
retain `/preview/remotes/demo/ws`, exactly as configured on the dev server.
If the upstream also serves HTTP assets under `/preview/remotes/demo/`, pass
that full entry path to `preview_mount`; HTTP forwarding and redirects retain
the prefix once. Routing is deterministic, without retries to another path.
Authentication, credential stripping and socket revocation apply to both
transports. React Refresh uses the host runtime and the remote's normal plugin.

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

Open chat tabs, names, numbered labels and active selection survive host/container
restart using an atomic `workspace-sessions.json` in `WORKSPACE_STORAGE`. Keep
the workspace and HOME volumes: HOME retains the existing history and pin/order
preferences. Restored conversations reconnect lazily when selected. This does
not restart in-flight tools or preserve PTY processes across a host restart.
The first upgrade cannot reconstruct an open set that was never persisted or
recover a historical title already overwritten by the old resume path.

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

Escape restores fallback queue text into the initiating page's composer and
records dismissal of those exact display rows in the host. Other and reopened
pages honor that marker. This does not cancel text already delivered through the
CLI terminal. Stop without restoring queue text leaves the queue display intact.
The queue's individual Remove button records the same host dismissal for that
item, without interrupting the turn or restoring text into a composer.
Accepted identities and allowlisted receipts also survive host restart in the
private `workspace-submissions.json` under `WORKSPACE_STORAGE`. A synced atomic
write precedes submission dispatch. Confirmed commitment and queue dismissal
remain durable; unconfirmed delivery is shown as **Unconfirmed**, with no
automatic resend. The same ID is suppressed after restart, and different text
cannot reuse it. Corrupt state blocks startup; a write failure blocks new
submissions until storage is repaired and the host restarted. Keep this volume
private: it contains prompt text, although no reusable connection token is
stored there. Synchronous snapshots can add latency for large retained prompt
sets. Closing a session clears its identities. In-flight tools/PTY processes
are not restored by this receipt recovery.

Unsent composer text and attachment metadata are saved per session in
`workspace-drafts.json` under `WORKSPACE_STORAGE`. Only **Draft saved** confirms
the host has persisted that revision; keep a page open while it reports unsaved
changes. Reopened pages and restarted hosts restore acknowledged drafts without
submitting them. Preserve uploaded files at their existing workspace paths.
Clean pages follow changes from other pages. Conflicting edits stay local until
you choose **Use workspace draft** or **Keep this draft**. Reconnect reads state
without automatically replaying writes. Empty saved revisions prevent a stale
page from restoring sent text. Submission delivery and draft clearing have
separate acknowledgements: closing before the clear is saved can retain the old
text. Closing a session removes its draft. Limits are 256 KiB text, 32 attachments,
8 MiB per draft, 128 session drafts and 32 MiB overall; storage failures stay
visible and preserve the last accepted revision.

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
reported. Shells/screens survive browser loss, not host restart.

The host answers OSC 4/10/11/12 color queries with zero pages, owns palette
set/reset state (104/110/111/112 and RIS), and includes it in screen sync.
Browser shell replicas apply that same shared state without emitting query
replies as duplicate keyboard input. Their palette is consistently dark,
independent of each page's UI theme. Hexadecimal X11 `#RGB` through
`#RRRRGGGGBBBB` and `rgb:R/G/B` forms use 8-bit renderer precision; named colors
are currently ignored. The host also answers `CSI 18 t` with its current text-area
rows/columns, including after resize and with zero pages. Browser replicas
suppress this reply so multiple pages cannot inject duplicate reports. Pixel
reports and other window/TUI compatibility remain open.

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

`test/core-tools.smoke.mjs` creates a session in the production browser UI,
closes every page and invokes eleven actual shared client tools over upstream
MCP frames. Independent checks cover filesystem bytes, search results, foreground
and background shell output/cursors/termination, notebook edits and todo state.
A fresh browser then reads the edited file through the existing bridge API and
reattaches to the same session. The fixture emits `core-tool-checks.json` outside
the checkout with 22 reviewed tool/handler assertion IDs. Collector tests reject
omitted, unknown, duplicate and failed cases; inventory tests validate bindings.
This is partial synthetic execution evidence. It explicitly records
`candidateImageTested: false`: the fixture runner builds current sources inside
an existing image. It is not a full-completion receipt, candidate-image or
actual-model acceptance, and it does not lower the strict completion gate.

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

`test/browser-retention.smoke.mjs` adds a deterministic load regression to
`test:browser:container`: the real shared host receives 10,000 synthetic
transcript records, continues with zero pages, executes Bash, and restores five
fresh pages without resubmitting or interrupting the session. TAP diagnostics
record all production JS chunks (raw/gzip/Brotli), Chromium/source provenance
and post-GC page heap. The test checks existing store/snapshot/render bounds,
a 128 MiB page-heap ceiling and a 16 MiB spread between equal-tail reopens.
These coarse renderer regression budgets do not measure host memory or browser
RSS and do not replace the actual-model/composed-image acceptance contract.

No release, version bump or image publication is part of this work. Claude lead
reviews/merges implementation and the consolidated documentation PR.

`test/lsp.smoke.mjs` exercises the production browser API and shared plugin LSP
manager against an isolated synthetic stdio peer. It verifies four zero-page
tools, document updates, browser list/restart, project process sharing/isolation,
last-owner cleanup and natural host shutdown. The runner writes sanitized
`lsp-checks.json` with 12 reviewed API/channel/tool/handler assertions after all
checks pass. The fixture builds current sources in the runner image; this
partial artifact explicitly disclaims candidate-image acceptance and is not a
completion receipt. Collector integrity tests run in `test:acceptance`.

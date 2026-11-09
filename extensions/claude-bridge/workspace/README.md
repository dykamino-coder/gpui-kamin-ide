# Browser workspace client

Task: [INC-2026-0087](../runtime-issues/INC-2026-0087.md).
Design and remaining acceptance: [DESIGN.md](DESIGN.md).
Podman build, startup and acceptance: [PACKAGING.md](PACKAGING.md).
Credential storage, automatic loopback access and reset: [CREDENTIALS.md](CREDENTIALS.md).

The workspace is a Node Bridge **client** whose local machine is a Linux
container. Its executable composes the same BridgeHost, client tool handlers,
plugin hooks, MCP/LSP and monitors used by KaminIDE. It serves the existing
webview App through an authenticated HTTP/WebSocket adapter implementing the
same KaminBridgeApi contract. There is one UI and one tool implementation.
Claude CLI and its login remain on the Bridge server.

Status: bounded implementations are available; the full owner request remains
open. Capability classifications and synthetic browser tests are not proof of
full live parity. The final acceptance evidence must identify the executed
source revision and image, including failures and unrun scenarios.

## Runtime composition

`src/cli.ts` builds the shared runtime, pins server administration requests to
its configured upstream, installs browser environment services, then wraps it
with `WorkspaceFiles` and `TerminalWorkspaceBackend`. The resulting host serves
the production browser bundle. Shutdown disposes terminals, file watches and
the shared runtime; a lost browser socket only detaches delivery.

`createSharedBackend` sets process cwd and initial project root to `/workspace`
and supports one shared runtime per process. Project files and client settings
persist in project/HOME volumes. Tools can access ordinary container paths,
including absolute paths and symlink destinations outside `/workspace`.
The container and its mounts provide isolation; `/workspace` is a project root,
not a filesystem jail. Client HOME must remain separate from server CLI auth.

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

Logical FileViews retain file/tree subscriptions with zero sockets and refresh
watched bytes/directories on re-attach. Identity is an opaque sessionStorage value,
not authoritative file content. There are at most 32 retained views, each with
64 file and 8 tree subscriptions. Full per-chat open layout, drafts, selections,
editor context and preview state still need host projection and acceptance.

Terminals belong to host chat tabs. Spawn is coalesced per tab and re-attach
receives a bounded headless-xterm screen/scrollback snapshot before live output.
Detach does not respawn a shell. Explicit kill/tab close/disconnect/shutdown
terminates owned processes. Limits include 32 records (including pending spawns),
5000 scrollback lines and an 8 MiB serialized snapshot budget; omitted history is
reported. Shells/screens survive browser loss, not host restart. Remaining OSC
palette/pixel-window query compatibility has not been accepted as full TUI parity.

The same FilePanel and terminal components render these host services. Browser
clipboard, downloads, uploads, path dialogs, notifications, links and audio use
explicit environment adapters; missing services fail explicitly. Actual plugin
LSP stays shared; unavailable native editor diagnostic commands reject. Native
KaminIDE editor/reveal/openInVscode features are classified as native host
operations in the matrix; Bridge FilePanel and editor context remain required.

The tracked **PREVIEW panel with HMR** follow-up remains unimplemented and
unaccepted. Existing file rendering does not provide an agent-controlled preview.

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
Its upstream protocol peer is synthetic; it is not live Claude acceptance.

Production browser smoke uses the real asset handler and boundary with an
explicit synthetic backend. Linux CI/Podman runs headless Chromium and reports
all shipped JS chunks (raw/gzip/Brotli) and post-GC page heap. Empty-App metrics
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
checks. It does **not** encode the remaining full host-page or FilePanel state
gates. A green command therefore cannot close the owner task. Add these requirements
and exhaustive observable behavior coverage before claiming full parity.

The Linux Podman live harness exercises chat, file tools, Bash, MCP, hooks,
FilePanel, close-mid-turn continuation, pending permissions after reopen and two
browser tabs. It retains sanitized summaries outside the checkout. The latest
executed source/image/report must be checked before marking these gates passed;
historical failures are not waived by later source changes. The executed
[#653 report](https://github.com/dykamino-coder/gpui-kamin-ide/pull/653) records
15/15 PASS for its listed scenarios on production sources matching main
`65fa7467`, including #650's title/toast integration; see DESIGN.md for exact
runner/image provenance and limitations. Pending #655 PREVIEW/HMR and #657
fallback-receipt changes are outside that run. It does not establish full parity.

The owner task remains open for remaining host-authoritative page/FilePanel
state, PREVIEW/HMR, exhaustive behavior proof and final composed live acceptance.
No release, version bump or image publication is part of this work. Claude lead
reviews/merges implementation and the consolidated documentation PR.

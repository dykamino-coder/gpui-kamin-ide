# Browser workspace client

Task: [INC-2026-0087](../runtime-issues/INC-2026-0087.md).
Architecture and remaining acceptance: [DESIGN.md](DESIGN.md).

This increment provides an HTTP/WebSocket boundary library and a headless
composition of the real shared `BridgeHost`, alongside the merged browser entry
for the existing App. It is not yet a runnable browser workspace: executable
composition, environment services, capability matrix and Podman acceptance remain open.
There is no second tool implementation or alternative chat UI in this package.

Owner requirement (2026-11-09): the browser must be a thin view and closing it
must not interrupt agent work. The container host owns all sessions, execution,
queues, pending prompts, file watches and PTYs, including with zero browsers.
Re-attach must deliver transcript/streaming/tool/prompt state, open files/watches
and terminal scrollback, then live updates, without replaying mutations or
duplicating/losing messages. Only explicit End session/Disconnect or host shutdown
tears down work. The merged browser entry does not yet meet this full contract:
host presentation snapshots and authoritative state migration remain open.
Acceptance must cover close/reopen during Bash/MCP, pending permission after
reopen, two tabs, and report production bundle size and page JS heap.

Pending increments remain tracked separately: #643 serves application assets;
#642 needs host-owned file watches and re-attach refresh; #644 needs host-owned
PTY lifetime and scrollback. The explicit Disconnect follow-up precedes #643,
then corrected #642. None of these increments establishes full parity.

Run the boundary checks with Node 22:

```sh
npm ci --no-audit --no-fund
npm --prefix ../extension ci --no-audit --no-fund
npm run typecheck
npm test
npm run build
```

`startWorkspaceServer` accepts one `WorkspaceBackend` and attaches an owned view
per browser socket. Incoming `invoke`, `send` and `invoke-diagnostic` frames keep
the existing BridgeHost message contract. Replies belong to the requesting view;
the backend owns event fan-out. A disconnected view is disposed without closing
the shared runtime. The embedding host must dispose its runtime on shutdown.
The built `./runtime` export provides `createSharedBackend`. Supply an existing
root directory, private global-storage directory, configured server URL/token,
version and explicit `WorkspaceEnvironment` services. It sets process cwd to
the root and composes the same client handlers and tool registry as KaminIDE.
There is one runtime per process, even after disposal, because shared handlers
have process-wide registrations. Run it in an isolated container HOME; shared
plugin configuration uses that HOME. Dispose both server and backend on shutdown.

The default cwd and initial project root are `/workspace`. Files and shell tools
have container-wide access, including absolute paths and symlink destinations
outside that directory. Isolation comes from the container and its mounts;
`/workspace` is a project location, not a filesystem sandbox. Mount only the
project and client state this workspace is intended to access.

The configured upstream identity is pinned: browser config/connect/tab/resume requests
cannot replace its server or token, and outgoing frames redact the configured
credential. Unsupported invokes fail explicitly. The three desktop arbitrary-URL
server helpers (`bridge:create-token`, `bridge:server-version`,
`bridge:server-fetch`) currently fail explicitly pending a pinned browser proxy.
Browser clipboard, dialogs, navigation and editor effects require supplied
services; no default all-success adapter is installed. Headless mode has no
VSIX-native diagnostic providers; actual plugin LSP remains shared, while native
editor command fallback rejects as unavailable. This is not full LSP acceptance.

The deterministic runtime test crosses authenticated HTTP/WS, two real host
views and the shared server connection pipeline. It checks Read/Write/Edit/Glob
against real bytes, pinned identity (including get-config → resume with the
redacted token and forged resume credentials), private replies/event fan-out,
clipboard round trip and persistent layout. Terminal disposal aborts foreground
tool controllers, terminates owned background shells, cancels external HTTP/SSE
MCP requests and closes shared resources without sending `session:end`; the
server's CLI can reattach. Transient desktop disconnect behavior is unchanged.
The test starts long-running foreground/background Bash processes and an actual
loopback HTTP MCP call with an unfinished body, then checks cancellation, owned
process termination and natural client exit. A
synthetic upstream protocol peer supplies session/MCP frames; this does not
prove browser UI, live model, external plugin or container acceptance. The test
uses a disposable HOME and cannot load the operator's Claude profile.

`WorkspaceAuth` pins one configured Bridge token and verifies it through the
existing `/api/dashboard/tokens/resolve` endpoint. `POST /auth/login` accepts
`{"token":"..."}` in JSON and issues an opaque HttpOnly SameSite cookie;
credentials never go into a URL. `GET /auth/session` reports authentication,
and `POST /auth/logout` invalidates the cookie and closes its sockets. Token
revocation and expiry are checked at upgrade and periodically (15 seconds by
default). An upstream outage fails closed. An auth-disabled dashboard response
does not grant access. Dashboard-admin login is still pending.

Binding defaults to `127.0.0.1:3460`. Opting into a different bind requires an
exact `origin` such as `https://workspace.example.test`. The TLS reverse proxy
must preserve that Host and browser Origin; forwarded headers are not trusted.
HTTPS origins set Secure cookies. The future Podman service will publish on
host loopback by default. No public bind or deployment is performed by these
library tests.

The boundary enforces Host/Origin checks, bounded login attempts and sessions,
32 sockets/upgrades, 64 concurrent dispatches per socket, 16 MiB frames and a
32 MiB output buffer. Invalid/forged host frames are rejected. Backend failures
close the view; adapters must return explicit invoke errors for unsupported
channels.

The workspace is a Bridge **client**. Claude CLI and its login belong solely to
the server; never mount or copy a server auth volume into this client. The
workspace needs its own Bridge client token plus persistent project files and
client settings/plugin HOME. Combined packaging must still keep server auth
owned by the server process.

## Browser transport and shared UI

`webview/browser.html` mounts the existing full `App` (sessions, chat, settings
and file panel) through the existing KaminBridgeApi channel tables. Build with
`npm run build:browser` in `webview`; serve the entire `webview/dist/browser`
directory at the HTTP origin root, with `/` mapped to `browser.html`. The
embedding executable/static asset server is still pending. Never serve project
files as application assets. The browser build preserves a deferred renderer
chunk: desktop single-file bundling would execute its module initializers before
the browser transport is installed.

The entry checks `/auth/session`, signs in via `/auth/login`, and uses only the
opaque HttpOnly cookie for the same-origin `/bridge` socket. The token field is
cleared when submitted; there is no token in browser storage, URLs or renderer
state. UI state is document-local memory; durable client settings remain host
responsibilities. Logout and reauthentication reload the document.

Private socket subscriptions deliver replies/events; browser `window.postMessage`
cannot settle invokes. The native VSIX receive path and its single API acquisition
are preserved. A lost socket rejects pending invokes with an unknown-outcome
error: the host may already have executed them. There is no command queue, startup
queue or mutation replay. The UI mounts after connection and becomes inert while
offline. Only best-effort diagnostics are discarded offline. Reconnect emits the
existing `bridge:reconnected` event to refresh read snapshots/session replay
without replacing the UI document. Connection deadlines, exponential backoff,
frame/output byte limits and page-owned cleanup bound transport resources. An
expired session returns to sign-in; protocol/overload closes require reload.

The browser smoke uses production assets, the real HTTP/WS boundary, headless
Chromium and an explicitly enumerated synthetic backend. Unknown fixture invokes
fail; this is startup/transport/UI composition evidence, not shared-handler,
model or Podman parity. Run after building both packages:

```sh
# In workspace; choose a private browser cache on a shared developer machine.
export PLAYWRIGHT_BROWSERS_PATH=/private/task/browser-cache
export PLAYWRIGHT_SKIP_BROWSER_GC=1
npx playwright install chromium --only-shell
npm run test:browser
```

CI repeats this headlessly and captures only synthetic fixture UI. Native window,
file/PTY/editor/browser environment adaptations, dashboard-admin login, the
exhaustive capability gate and live Podman acceptance remain open. The presence
of UI controls in this build is not a claim those adapters already exist.

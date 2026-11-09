# Browser workspace client

Task: [INC-2026-0087](../runtime-issues/INC-2026-0087.md).
Architecture and remaining acceptance: [DESIGN.md](DESIGN.md).

This package provides the HTTP/WebSocket boundary, the existing App browser
entry and file-panel environment services. It is not yet a runnable browser
workspace: shared runtime PR #640 is under revision; executable composition,
remaining environment services, the full matrix and Podman acceptance remain open.
There is no second tool implementation or alternative chat UI in this package.

Run the boundary checks with Node 22:

```sh
npm ci --no-audit --no-fund
npm run typecheck
npm test
npm run build
```

`startWorkspaceServer` accepts one `WorkspaceBackend` and attaches an owned view
per browser socket. Incoming `invoke`, `send` and `invoke-diagnostic` frames keep
the existing BridgeHost message contract. Replies belong to the requesting view;
the backend owns event fan-out. A disconnected view is disposed without closing
the shared runtime. The embedding host must dispose its runtime on shutdown.
The tests use an explicitly synthetic transport peer; they do not prove shared
handler, UI, model, plugin or container acceptance.

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
PTY/editor/browser environment adaptations, dashboard-admin login, the
exhaustive capability gate and live Podman acceptance remain open. The presence
of UI controls in this build is not a claim those adapters already exist.

## Container file-panel services

`WorkspaceFiles` decorates a `WorkspaceBackend`. It intercepts only the existing
`file-viewer:read`, `read-binary`, `write`, `watch`, `unwatch` and
`file-tree:list-dir`, `rename`, `watch`, `unwatch` channels. Every other frame
continues to the underlying shared backend. These are environment services for
the existing file panel; the agent's Read/Write/Edit/Glob tools stay in the
shared client registry.

Set `root` to the container workspace (default `/workspace`). Relative paths
resolve against that root, while absolute paths and symlinks retain container
filesystem semantics. This is not a filesystem sandbox around one directory.
The authenticated container is the user's local machine; never give it host
home or unrelated host mounts. Writes preserve supplied UTF-8 text, truncate
existing files, and create files without creating missing parent directories.
Rename uses the container filesystem's normal replacement semantics.

Reads/writes accept regular files up to 8 MiB. Text additionally has a 12 MiB
JSON-encoded limit so control characters cannot overflow the transport frame;
binary reads return base64. Concurrently changing reads fail with a retry error.
Listings return directories first and reject more than 10,000 entries. Errors
are explicit invoke replies, retaining the request ID and document generation.

Each view owns up to 64 file watches and 8 recursive directory watches. Watching
a file's parent preserves updates across atomic replacement. File events are
debounced; unlink never invents empty file content, and recreation is observed.
Directory events carry the original root and the changed parent/path. Required
`onWatchError` forwards asynchronous failures to the embedding host's logger/UI;
errors never masquerade as successful empty content. A future executable must
wire that callback to the shared logging surface.

Socket disposal releases that view's watchers and pending debounce timers without
affecting other views. Dispose the file decorator before disposing the shared
runtime on application shutdown. It does not keep Node alive by itself.

Tests execute every file channel against real temporary files, including an
authenticated HTTP/WebSocket integration, atomic saves, watcher ownership,
deletion/recreation, limits, explicit errors and lifecycle cleanup. The upstream
fixture rejects unknown invokes. This proves this adapter only; actual browser
file-panel rendering, selection context and live container acceptance remain
required after executable composition. Linux CI also checks FIFO rejection and
symlinks; Windows skips FIFO and skips symlinks if the identity lacks privilege.

# Browser workspace client

Task: [INC-2026-0087](../runtime-issues/INC-2026-0087.md).
Architecture and remaining acceptance: [DESIGN.md](DESIGN.md).

This package provides the HTTP/WebSocket boundary library and a browser entry
for the existing App. It is not yet a runnable browser workspace: executable shared `BridgeHost` composition,
environment services, capability matrix and Podman packaging follow in bounded PRs.
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
directory at the HTTP origin root, with `/` mapped to `browser.html`. Supply the
production asset handler to the HTTP boundary:

```ts
const serveApp = await createAppAssetHandler('/opt/bridge/webview/dist/browser')
const server = await startWorkspaceServer({ auth, backend, serveApp })
```

`createAppAssetHandler` and `startWorkspaceServer` are package exports. Load the
assets before listening; pass a dedicated, trusted browser build directory, never
`/workspace` or client HOME. The handler snapshots `browser.html` and supported
files under `assets/` into memory. It rejects symlinks, special files, missing
entry/assets directories, files above 16 MiB, builds above 64 MiB, more than 4096
asset entries or directory depth above eight. Project files, source maps, unknown
routes and traversal paths return 404 without an HTML fallback. GET and HEAD
preserve byte lengths and MIME types; all responses use `no-store`, so a restart
cannot reuse stale application code. Changes to on-disk assets require a host
restart. Only trusted application bytes are public before sign-in; API/socket
authentication is unchanged. Executable/runtime composition is still pending.

Never serve project files as application assets. The browser build preserves a deferred renderer
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

The browser smoke uses this production asset handler, the real HTTP/WS boundary, headless
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

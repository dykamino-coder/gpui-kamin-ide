# Browser workspace client

Task: [INC-2026-0087](../runtime-issues/INC-2026-0087.md).
Architecture and remaining acceptance: [DESIGN.md](DESIGN.md).

This increment provides an HTTP/WebSocket boundary library and a headless
composition of the real shared `BridgeHost`. It is not yet a runnable browser
workspace: the browser entry, browser environment services, capability matrix
and Podman packaging follow in bounded PRs.
There is no second tool implementation or alternative chat UI in this package.

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

The configured upstream identity is pinned: browser config/connect/tab requests
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
against real bytes, pinned identity, private replies/event fan-out, clipboard
round trip, persistent layout and natural process exit after disposal. A
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
channels. The browser adapter's startup queue, pending-call rejection and
reconnect/replay policy remain part of the next implementation stage.

The workspace is a Bridge **client**. Claude CLI and its login belong solely to
the server; never mount or copy a server auth volume into this client. The
workspace needs its own Bridge client token plus persistent project files and
client settings/plugin HOME. Combined packaging must still keep server auth
owned by the server process.

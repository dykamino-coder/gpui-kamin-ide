# Browser workspace client

Task: [INC-2026-0087](../runtime-issues/INC-2026-0087.md).
Architecture and remaining acceptance: [DESIGN.md](DESIGN.md).

This first implementation is the HTTP/WebSocket boundary library. It is not yet
a runnable browser workspace: the shared `BridgeHost` environment adapter,
browser entry, capability matrix and Podman packaging follow in bounded PRs.
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
channels. The browser adapter's startup queue, pending-call rejection and
reconnect/replay policy remain part of the next implementation stage.

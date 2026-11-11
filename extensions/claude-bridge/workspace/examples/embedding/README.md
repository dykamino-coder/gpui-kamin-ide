# Embedding example

A minimal application that runs the Kamin Bridge workspace engine without the
KaminIDE UI: its own HTTP server, its own sign-in and its own page, on top of
[`@kaminide/bridge-workspace-host`](../../HEADLESS.md) and
[`@kaminide/bridge-browser-client`](../../../browser-client/README.md).

```
browser ──► server.mjs :8080  (PUBLIC_ORIGIN)
             ├─ GET /, /app.js, /client/index.js   own page + browser client
             ├─ POST /site/login                   own sign-in → one-time assertion
             └─ /auth/*, /bridge, /preview/*, /health ──► workspace host 127.0.0.1:3460
                                                        └─► Bridge server (BRIDGE_SERVER_URL)
```

- `auth.mjs` is the sign-in policy. The site checks the user (a demo password
  here; Keycloak or similar in a real application), signs a one-minute,
  single-use assertion, and the browser posts it to the workspace's
  `POST /auth/login`. The host keeps opaque cookie sessions and calls the policy
  on every request, upgrade and periodically for open sockets.
- `server.mjs` starts the host on loopback with `origin` set to the public
  origin, and forwards the workspace routes unchanged, keeping `Host` and
  `Origin`. WebSocket upgrades (`/bridge`, preview HMR) are piped byte for byte.
- `public/app.js` uses only the browser client: `connect`, `sync`, `createTab`,
  `submitPrompt`, `interrupt`, transcript events and `preview.onState`. Render
  the remote itself with `@kaminide/bridge-browser-client/preview` (React 19 and
  the federation runtime are the application's).

## Container

Build from the repository root. The image packs both packages from the
checkout; an application installs the release tarballs
`kaminide-bridge-workspace-host-<version>.tgz` and
`kaminide-bridge-browser-client-<version>.tgz` instead.

```sh
podman build --format docker \
  --ignorefile extensions/claude-bridge/workspace/examples/embedding/containerignore \
  -f extensions/claude-bridge/workspace/examples/embedding/Dockerfile \
  -t localhost/kamin-embedding-example:local .

podman run -d --name kamin-embedding-example -p 127.0.0.1:8080:8080 \
  -e PUBLIC_ORIGIN=http://127.0.0.1:8080 \
  -e BRIDGE_SERVER_URL=ws://bridge-server:3456 \
  -e DEMO_PASSWORD=change-me \
  -v kamin-embedding-workspace:/workspace \
  -v kamin-embedding-home:/home/workspace \
  localhost/kamin-embedding-example:local
```

| Variable | Meaning |
| --- | --- |
| `PUBLIC_ORIGIN` | Exact origin the browser uses (scheme, host, port). Required. Behind TLS use the `https:` origin. |
| `BRIDGE_SERVER_URL` | Bridge server origin or `/ws/session` URL reachable from the container. Required. |
| `DEMO_PASSWORD` | Password of the demo sign-in form. Required. |
| `BRIDGE_TOKEN` / `BRIDGE_TOKEN_FILE` | Optional one-time client token when the server refuses self-registration; stored in `WORKSPACE_STORAGE` and removed from the environment after start. |
| `PORT` | Public listen port inside the container, default `8080`. |
| `WORKSPACE_ROOT` | Project directory, default `/workspace`. |
| `WORKSPACE_STORAGE` | Private host state, default `/home/workspace/.local/state/bridge`. |
| `PREVIEW_ENTRY` | Optional container-local `http://127.0.0.1:<port>/remoteEntry.js` mounted as preview `demo` at start. The agent normally mounts previews itself with `preview_mount`. |

Volumes:

- `/workspace` — the project the agent edits;
- `/home/workspace` — the client HOME: Bridge credential and browser sessions
  (mode 0600, under `.local/state/bridge`), CLI client settings and caches.
  Keep it per workspace and never share it with the Bridge server's volumes.

Open `PUBLIC_ORIGIN`, sign in with any user name and `DEMO_PASSWORD`, then
type a prompt. Without a Claude login on the Bridge server the session connects
but the agent cannot answer.

## Verified

Live Podman run of this image on 2026-11-11 (Windows host, Podman 6.0.2) against
a disposable `dykamino/open-claude-bridge:6.3.167` container on a private
network: start with self-registration, page and client served, demo sign-in
rejected with a wrong password, assertion accepted once and refused on replay,
`/preview/state` 401 without and 200 with the cookie, `/bridge` connected
through the proxy with the packaged browser client (`get-version`, `sync`,
`createTab` → session `connected` on the Bridge server), preview state pushed
over `/bridge` once the dev server answered, the remote entry served through
the authenticated proxy (401 without cookie), logout closing the socket with
`authentication-required`. The page itself was opened in a browser and reached
`Workspace: connected` with the pushed preview state. Not exercised: a model
answer (no Claude login) and rendering a real Module Federation remote, which
the workspace browser smoke tests cover.

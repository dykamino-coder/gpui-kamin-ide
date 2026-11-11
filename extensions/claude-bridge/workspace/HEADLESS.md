# @kaminide/bridge-workspace-host

The Kamin Bridge workspace service without a browser UI: the shared BridgeHost
(the same client engine as KaminIDE), files, shell, terminals, MCP, hooks,
skills, plugins, user tools and the Module Federation preview proxy. An
embedding application brings its own sign-in and its own UI; the UI talks to
the host with [`@kaminide/bridge-browser-client`](../browser-client/README.md).

The bundled container (`dist/cli.cjs`, see [README.md](README.md)) runs the
same `dist/headless.cjs`: there is one composition, in `src/headless.ts`.
A runnable embedding example with a container image is in
[`examples/embedding`](examples/embedding/README.md).

## Install

Every KaminIDE release attaches both packages to its GitHub Release
`kaminide-v<app version>`, with stable asset names:

```
kaminide-bridge-workspace-host-<version>.tgz
kaminide-bridge-browser-client-<version>.tgz
```

`<version>` is the Bridge server version of the same release (the server image
`dykamino/open-claude-bridge:<version>`). Install the tarball directly, e.g.
`npm install ./kaminide-bridge-workspace-host-6.3.170.tgz`.

## Contract

```ts
import { startHeadlessWorkspace, createSessionCookie, sessionCookieHeader,
  type WorkspaceAuthPolicy } from '@kaminide/bridge-workspace-host'

const workspace = await startHeadlessWorkspace({
  root: '/workspace',                          // project directory, must exist
  storage: '/home/workspace/.local/state/bridge', // private state, persistent volume
  serverUrl: 'ws://bridge-server:3456',        // Bridge server origin or /ws/session
  origin: 'https://studio.example',            // exact origin browsers use
  bind: '0.0.0.0',
  port: 3460,
  auth,                                        // WorkspaceAuthPolicy, see below
})
// workspace.origin, workspace.address, workspace.preview
await workspace.close()                         // idempotent
```

Declarations (`headless.d.ts`) are generated from `src/headless.ts` at build
time. Exports: `startHeadlessWorkspace`, `bridgeTokenAuth`, `version`
(BridgeHost version reported to the server), `SESSION_COOKIE`,
`readSessionCookie`, `createSessionCookie`, `sessionCookieHeader`,
`resetCredentials`, `WorkspacePreview` and the option/result types.

### Sign-in policy

`auth` is a `WorkspaceAuthPolicy`, or a factory that receives the workspace's
own Bridge credential (`bridgeTokenAuth` is the built-in factory used by the
container: sign-in with the Bridge token).

| Member | Called by | Contract |
| --- | --- | --- |
| `login(candidate)` | `POST /auth/login` | `candidate` is the `token` field of the JSON body. Return a new cookie value or `null` (401). |
| `validate(cookie)` | every `/preview/*` request, every `/bridge` upgrade, and every 15 s for open sockets | `false` closes existing sockets with code 4001. |
| `logout(cookie)` | `POST /auth/logout` | Forget the session; the server closes its sockets. |
| `cookieHeader(cookie, secure)` | after login/logout | Full `Set-Cookie` value. Use `sessionCookieHeader`. |
| `loginLocal()` | only with `autoLogin: true` | Optional loopback auto-login. Off by default. |

Cookie name is `kamin_workspace` (`SESSION_COOKIE`); values must be 43
base64url characters (`createSessionCookie()`). The Bridge token never reaches
a browser.

### What the embedding application must provide

- **Authentication.** The host trusts `auth` completely. Sign-in, user
  identity and authorization are the embedding application's responsibility.
- **Origin.** `origin` must be the exact scheme, host and port the browser
  uses. Requests whose `Host` or `Origin` differ are refused (403); use an
  `https:` origin when the browser connects through TLS (cookies get `Secure`).
  A reverse proxy must forward `/auth/*`, `/bridge` (WebSocket),
  `/preview/*` (HTTP and WebSocket) and `/health` unchanged, preserving `Host`.
- **Volumes.** `root` (the project) and the HOME of the process (CLI client
  settings, `~/.claude`) and `storage` must persist. `storage` holds the
  Bridge credential and browser sessions with mode 0600; never share it between
  workspaces.
- **Bridge credential.** On first start the host registers on the Bridge
  server, or consumes `BRIDGE_TOKEN` / `BRIDGE_TOKEN_FILE` from `env` once and
  stores it; both variables are then removed from `env`. Later starts read the
  stored file. `resetCredentials(storage)` forgets it.
- **One instance per process.** The shared handler registries are process-wide.
- **Node** `^22.19.0 || ^24 || ^26`, Linux in production. Agent tools run with
  the process's permissions; isolate the workspace in its own container.

### HTTP and WebSocket surface

| Route | Purpose |
| --- | --- |
| `GET /health` | Reachability, no authentication. |
| `POST /auth/login`, `POST /auth/logout`, `GET /auth/session` | Sign-in through the policy. |
| `WS /bridge` | BridgeHost frames (`invoke`, `send`, `event`, `invoke-diagnostic`). |
| `GET /preview/state` | Current preview state (kept for clients without the event). |
| `POST /preview/unmount` | Close the preview. |
| `/preview/remotes/<name>/*` | Authenticated proxy to the container-local dev server, including HMR. |
| other `GET` | `serveApp`, or `503 {"error":"Browser UI is not installed"}`. |

Preview state is pushed over `/bridge` as the event
`workspace:preview-state` with one argument `{ descriptor, build, available }`:
once after every connection and on every mount, props update, unmount, dev
server availability change or entry rebuild. The host samples the entry
fingerprint once per second for all views while a remote is mounted; browsers
no longer poll.

## Version and compatibility

- The package version equals the Bridge server version of the same release;
  `package.json` repeats it as `bridge.server`. That pair is built and tested
  together. Other server versions follow the compatibility of the KaminIDE
  Bridge extension of that release.
- `@kaminide/bridge-browser-client` of the same version speaks this host's
  frames; the frame format and channel names are the ones the KaminIDE webview
  uses.

## Build and pack from source

```sh
npm ci && npm --prefix extensions/claude-bridge/extension ci && npm --prefix extensions/claude-bridge/workspace ci
npm --prefix extensions/claude-bridge/workspace run build          # dist/headless.cjs, dist/headless.d.ts, dist/cli.cjs
npm --prefix extensions/claude-bridge/workspace run pack:headless  # dist-packages/kaminide-bridge-workspace-host-<server version>.tgz
```

`pack-headless.mjs` copies `dist/headless.cjs` byte for byte, declares exactly
the runtime packages the bundle imports (failing on an undeclared one) and
names the tarball `kaminide-bridge-workspace-host-<version>.tgz`.
`--version 6.3.170-studio.1` packs a pre-release.

## Requests from ATON Studio and their state

Studio's first request (kamin-bridge-webclient PR #1) asked for:

1. One composition for `cli.ts` and the headless entry — **done**: the CLI calls
   `startHeadlessWorkspace` and loads the same `dist/headless.cjs`.
2. Generated declarations — **done**: `dist/headless.d.ts` is generated from the
   source by the build.
3. Release tarballs with a stable name — **done**: release CI attaches
   `kaminide-bridge-workspace-host-<version>.tgz` and
   `kaminide-bridge-browser-client-<version>.tgz`.
4. Preview state over `/bridge` instead of polling — **done**: event
   `workspace:preview-state`.
5. Several users on one workspace — **open**, registered as
   [INC-2026-0094](../runtime-issues/INC-2026-0094.md). The engine currently
   assumes one owner: one Bridge identity, one tab set and one draft state.
6. Browser client package — **done**: `@kaminide/bridge-browser-client`.

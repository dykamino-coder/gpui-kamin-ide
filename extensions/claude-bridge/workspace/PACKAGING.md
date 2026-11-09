# Podman workspace and live acceptance

Task: [INC-2026-0087](../runtime-issues/INC-2026-0087.md).

This increment composes the shared client, browser environment and production
browser assets in `dist/cli.cjs`. It includes the published #643 and #646 heads;
those dependencies must land before this PR. An image build alone does not
establish full-parity acceptance.
Known missing features remain visible in the live report. Bridge's own FilePanel
is in scope. IDE editor open/reveal/openInVscode is NOT-APPLICABLE:
"KaminIDE host feature, not Bridge".

## Build from the repository root

```sh
podman build --format docker \
  --build-arg SOURCE_REVISION="$(git rev-parse HEAD)" \
  --ignorefile extensions/claude-bridge/workspace/containerignore \
  -f extensions/claude-bridge/workspace/Dockerfile \
  -t localhost/bridgeweb-acc-workspace:local .
```

Use `--format docker`: Podman's OCI format discards Dockerfile HEALTHCHECK.
The allowlist excludes local dependency/build output, dotenv files, logs and
private keys. No token is a build argument. The runtime is UID/GID 10001, with
Node 22, Bash, PowerShell, Git, Python and ripgrep. npm uses repository lockfiles;
OS packages follow Debian/Microsoft repositories at build time. No image is
published. `/workspace` is the initial project directory, and the full container
filesystem is available to the agent. Its container and mounts are the sandbox.

## Client only: existing external server

Provision a dedicated Bridge client token through the server dashboard or
documented `POST /api/dashboard/tokens`. Keep it in a private UTF-8 text file
outside the checkout, readable by UID 10001 inside the container. Do not use a
server CLI auth directory. The image accepts either `BRIDGE_TOKEN` or
`BRIDGE_TOKEN_FILE` and rejects both together. Compose uses the file option;
the file is mounted read-only and never copied into the image. The browser
sign-in uses the same token and then an opaque HttpOnly cookie.

Run from `extensions/claude-bridge/server` (POSIX shell example; set equivalent
environment variables in PowerShell):

```sh
export BRIDGE_TOKEN_FILE=/private/bridge-client-token
export BRIDGE_SERVER_URL=ws://host.containers.internal:3462
podman compose -p bridgeweb-acc-client -f docker-compose.workspace.yml --profile client up -d workspace
```

Open `http://127.0.0.1:3460`. Host publication defaults to loopback. To change
the port, set both `WORKSPACE_PORT` and the matching `WORKSPACE_ORIGIN`.
`WORKSPACE_PUBLISH_BIND` explicitly opts into a different host bind. For remote
access use an HTTPS reverse proxy preserving the exact Host and Origin, and set
`WORKSPACE_ORIGIN` to that external origin. The internal listener is HTTP on
3460; the health probe retains the configured Host and checks loopback `/health`.
No proxy headers grant authentication. `/health` checks the host, not model or
upstream readiness.

Named project and HOME volumes preserve files, client settings and plugin
configuration across recreation. Browser closure detaches a view; the host
process continues. This does not yet claim durable turn recovery after a host
restart. `down` stops this explicitly named project without deleting its volumes:

```sh
podman compose -p bridgeweb-acc-client -f docker-compose.workspace.yml --profile client down
```

## Combined: separate server and workspace containers

Use an approved, locally built server image in `BRIDGE_SERVER_IMAGE` (default
`localhost/opencode-claude-max-proxy_bridge:latest`). This profile creates a new
server on host loopback port 3463; it does not modify an existing server. Keep
the same explicit project name for every command:

```sh
export BRIDGE_TOKEN_FILE=/private/combined-client-token
podman compose -p bridgeweb-acc-combined -f docker-compose.workspace.yml --profile combined up -d bridge
```

Complete the new server's ordinary Claude CLI login and create its dedicated
client token using its dashboard at `http://127.0.0.1:3463`. Provision that token
in `BRIDGE_TOKEN_FILE`, then start its workspace:

```sh
podman compose -p bridgeweb-acc-combined -f docker-compose.workspace.yml --profile combined up -d workspace-combined
```

The server owns separate data/log/HOME/auth volumes, including `.claude.json`
and `.claude/`. The client gets none of those mounts. The workspace reaches the
server over the private compose network (`ws://bridge:3456`). Both workspace
profiles publish the same default port, so run one at a time or select another
workspace port and origin. Shutdown each process through its own container;
Tini forwards workspace signals and reaps children.

## Automated live acceptance

All browser processes run headlessly **inside Linux Podman**, never on the
Windows host (including headless_shell). The host needs Node 22+ and Podman only.
The runner mounts the checkout read-only, copies allowlisted source to its
container filesystem and copies sanitized reports back before removing its exact
container name. It never uses host node_modules or browser caches. The private
browser cache belongs to the runner image; garbage collection is disabled.

Build the runner from the repository root:

```sh
podman build --ignorefile extensions/claude-bridge/workspace/containerignore \
  -f extensions/claude-bridge/workspace/acceptance/Dockerfile \
  -t localhost/bridgeweb-acc-playwright:local .
```

Then from `extensions/claude-bridge/workspace`:

```powershell
npm run test:browser:container -- --report-dir C:/private/bridgeweb2/reports
npm run test:live -- --report-dir C:/private/bridgeweb2/reports
```

Each invocation performs one run, without browser retry loops. Linux GitHub CI
may also run the fixture command directly. The live script itself refuses to run
outside the Linux acceptance container. Default live prerequisite is the existing
`kamin-acc-302` at `host.containers.internal:3462`; never stop/recreate it or read
its volumes. Live mode mounts the Podman VM API socket into this trusted runner
so it can create only its unique `bridgeweb-acc-*` client/project/HOME and revoke
only its issued token. Default rootful socket is `/run/podman/podman.sock`; pass
`--podman-socket /run/user/1000/podman/podman.sock` for the explicitly selected
rootless connection. The runner uses VM host networking to reach the workspace's
loopback port. Fixture mode does not mount the Podman socket.

Token issuance and browser login stay in memory. The client receives the token
once over stdin, avoiding credentials in Podman's configured environment,
arguments or temporary files. Before issuance a synthetic non-credential probe
checks whether runtime ConfigStore persists its injected value. Until that core
seam is corrected, credential-storage is XFAIL and real-token scenarios BLOCKED.
No traces, screenshots, HARs or real transcripts are retained.

Use `--image <local-tag>` for another built workspace image, `--runner-image
<local-tag>` for the Linux browser runner, and `--strict` to make known missing
assertions fail. A passing assertion is PASS even if an older build lacked it.
A known missing assertion is XFAIL with a clear reason; a prerequisite outage is
BLOCKED. Exit codes: 0 no failures/blocks, 1 unexpected/strict failure, 2 blocked.
XFAIL remains incomplete owner acceptance. Source identity includes dirty status;
image identity is reported separately. Reports must be outside the checkout.

The suite drives the real shared App command API and the actual model/server;
it does not replace the host with a fake backend. It checks a chat reply,
Read/Write/Edit and independent byte verification, Bash, a disposable stdio MCP
tool with a file side effect, a real PostToolUse hook, Bridge FilePanel content,
last-page closure during foreground Bash, restored full transcript IDs/tool
results without duplicates, a permission prompt answered after reopen, and two
browser tabs viewing the same runtime. Persistence checks close the only page
before waiting for completion through the owned container, then open a fresh
document in the same browser context. Permission checks must observe an actual
blocked command before closing. A synthetic model reply cannot satisfy those
checks. The runner measures all shipped JavaScript chunks as raw bytes and
individual gzip/Brotli sums, and page JS heap through Chromium CDP after GC.
Heap is unavailable when browser startup is blocked; no estimate is substituted.

Reports contain only fixed failure codes/reasons, source/image identity,
durations and numeric measurements. Attach `summary.md` to the PR; keep raw
development evidence private. The suite is a live acceptance gate, not the
exhaustive capability matrix or a replacement for deterministic runtime tests.

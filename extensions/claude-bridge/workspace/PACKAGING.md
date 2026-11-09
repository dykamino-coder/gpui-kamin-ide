# Podman workspace and live acceptance

Task: [INC-2026-0087](../runtime-issues/INC-2026-0087.md).

This increment packages the shared client and its production browser assets.
Executable composition and browser environment services are being integrated
separately. Until `dist/cli.cjs` is present, startup exits 78 with an explicit
message; an image build alone is not deployable/full-parity acceptance.
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

Use Node 22+ on the host with Podman available and an existing authenticated
`kamin-acc-302` at `127.0.0.1:3462`. Never stop/recreate it or read its volumes.
The runner creates one unique `bridgeweb-acc-*` client, project and HOME, and
revokes only the token it created. It never lists tokens or reads CLI auth.
Token issuance and browser login stay in memory. The container receives its
token once over stdin, avoiding credentials in Podman's configured environment,
command arguments or temporary files. Raw stdout/stderr from child commands is
not reported. No traces, screenshots, HARs or transcripts are retained.

```powershell
# From workspace; choose private paths outside every checkout.
$env:PLAYWRIGHT_BROWSERS_PATH = 'C:/private/bridgeweb2/playwright'
$env:PLAYWRIGHT_SKIP_BROWSER_GC = '1'
npm ci --no-audit --no-fund
npx playwright install chromium --only-shell
node acceptance/run.mjs --report-dir C:/private/bridgeweb2/reports
```

Use `--image <local-tag>` for another built image, `--server-origin
http://127.0.0.1:<port>` only when the lead supplies a different server, and
`--strict` to make known missing-feature assertions fail the run. The default
report labels a known missing assertion XFAIL with its issue/reason; a passing
assertion always becomes PASS, even if an older build lacked it. Remove stale
entries in `acceptance/report.mjs` as the corresponding PRs merge. A prerequisite
outage produces BLOCKED, never PASS or XFAIL. Exit codes: 0 no failures/blocks,
1 unexpected failure (or strict expected failure), 2 blocked. XFAIL is still
incomplete owner acceptance.

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

# Podman browser workspace runbook

This is an opt-in client deployment. Use Podman only. Choose an isolated project
name and mount only its project/HOME and its own read-only client-token file.
The existing Bridge server and its volumes are never adopted or modified.
Shell examples below use Bash, including when invoking Podman from Windows.

From the repository root, build the local image with the explicit allowlist:

```sh
podman build --ignorefile extensions/claude-bridge/workspace/containerignore \
  -f extensions/claude-bridge/workspace/Dockerfile \
  --build-arg SOURCE_REVISION="$(git rev-parse HEAD)" \
  -t localhost/bridgeweb-acc-workspace:local .
```

The image contains Node 22, Bash, PowerShell, ripgrep, Git, Python 3 and tini,
running as UID/GID 10001. `/workspace` and `/home/workspace` are persistent mount
locations. No Claude CLI/login is installed in the client. The image's listener
uses `0.0.0.0` inside the container; published host ports default to loopback.

## Existing server

Use an already approved Bridge server URL and a separately issued client token
from its existing dashboard/API. Store that token outside the checkout in a
file readable by container UID 10001 and mount it read-only. Do not print the
value, place it in a command argument or persist it in workspace config.

Set `BRIDGE_TOKEN_FILE` to that host file path and `BRIDGE_SERVER_URL` to the
server's reachable `ws://` or `wss://` URL. Then, from the repository root:

```sh
podman compose -p bridgeweb-myworkspace \
  -f extensions/claude-bridge/server/docker-compose.workspace.yml \
  --profile client up -d
```

The default upstream is `ws://host.containers.internal:3462`; confirm the
configured route for the deployment. Open `http://127.0.0.1:3460` and sign in
using the configured client token. Startup logs intentionally contain no token
or upstream error payload. Health checks test only the client's health endpoint.

Changing `WORKSPACE_PORT` also requires matching `WORKSPACE_ORIGIN` (for example
`http://127.0.0.1:3470`). The Compose port target stays 3460 inside the container.
Project and client HOME volumes belong to the selected Compose project.
Replacing the owned client container retains those volumes and project/settings;
running shells and process-owned view state require the original live host.

## Combined profile

Combined mode starts separate client/server containers in the chosen project.
It preserves the server protocol and separate server HOME/CLI-auth volumes.
Set `BRIDGE_SERVER_IMAGE` to a previously built, approved local server image.
Use `--profile combined` in place of `--profile client`; do not enable both.
The client connects to `ws://bridge:3456`; the combined server publishes on
loopback 3463 by default (`COMBINED_BRIDGE_PORT` overrides it).

Provision that new owned server's normal CLI login/token through the existing
server [deployment procedure](../server/DEPLOYMENT.md) before bringing up its
client. Never mount/copy an unrelated server's auth volumes
into the client. Combined mode does not reuse or recreate the supplied server.
No server/image publication is implied by this local profile.

## Remote browser access

Keep the default host loopback publication unless remote access is intentional.
For an opted-in TLS reverse proxy, set an exact `WORKSPACE_ORIGIN`, such as
`https://workspace.example.test`. Preserve that Host and browser Origin at the
workspace; forwarded headers are not an authentication substitute. HTTPS enables
Secure session cookies. `WORKSPACE_PUBLISH_BIND` explicitly changes the host
publication address. Direct Node runs default to 127.0.0.1; a different
`WORKSPACE_BIND` requires the exact origin. The workspace grants container shell
access to its authenticated client identity.

## Linux headless acceptance

Do not install or launch a native Windows browser for this task. The launcher
below starts a Linux Playwright container with a read-only checkout mount,
then copies allowed source roots into its disposable execution directory,
and retains sanitized evidence outside the checkout. Build the runner locally:

```sh
podman build --ignorefile extensions/claude-bridge/workspace/containerignore \
  -f extensions/claude-bridge/workspace/acceptance/Dockerfile \
  -t localhost/bridgeweb-acc-playwright:local .
```

From `extensions/claude-bridge/workspace`, supply a private report directory:

```sh
npm run test:browser:container -- --report-dir /private/bridgeweb-fixture
npm run test:live -- --report-dir /private/bridgeweb-live --strict
```

On Windows use absolute private Windows paths outside the checkout. The runner's
live mode is tailored to the approved server published at Podman-VM loopback 3462.
It needs the VM's Podman API socket (default `/run/podman/podman.sock`, override
with `--podman-socket` for the rootless path). Only this trusted acceptance runner
mounts that socket; ordinary client containers must not mount it.

The runner creates uniquely named `bridgeweb-acc-*` containers/volumes and a
client token through the existing server API, revokes its own token, and removes
only exact owned test resources. An admin-authenticated server may block token
issuance; do not replace it with an unrelated credential or disable auth.

Reports identify runner source, dirty state, image ID/source, scenario outcomes,
Chromium version, all shipped JS byte sizes and post-GC page heap. PASS requires
an executed assertion; FAIL remains a failure, BLOCKED means unrun, and XFAIL is
incomplete acceptance. `--strict` makes known missing features fail. Live model
output, raw logs, credentials, screenshots, traces and transcripts are not retained.
Fixture smoke is deterministic UI/transport evidence, not live-model parity.
The executed #653 live suite passed its 15 listed scenarios on production
sources identical to main `91b757d0`; DESIGN.md records exact provenance and
remaining acceptance. This does not establish full desktop parity.

Stop/recreate only explicitly owned resources. Do not run prune or broad cleanup,
and do not remove persistent project/HOME/server-auth volumes during normal
client replacement. Follow the task card for remaining acceptance and ownership.

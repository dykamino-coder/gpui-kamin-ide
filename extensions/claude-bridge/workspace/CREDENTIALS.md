# Service credentials and loopback access

Task: [INC-2026-0087](../runtime-issues/INC-2026-0087.md).

The executable workspace service owns its Bridge identity and browser sessions.
On an empty HOME volume it registers through `POST /api/dashboard/tokens` on the
configured `BRIDGE_SERVER_URL`. Servers that require administrator authorization
for registration need a one-time bootstrap secret instead: supply `BRIDGE_TOKEN`
or mount a secret file and set `BRIDGE_TOKEN_FILE` to its container path. Do not
put a token in an image, command argument, URL or Compose file. The image also
accepts `--token-stdin` for callers that already hold a bootstrap token in memory.

The service stores the token and a random browser secret in
`/home/workspace/.local/state/bridge/workspace-credentials.json`, and HMAC-keyed
browser-session records in `workspace-sessions.json`. Both files have mode 0600;
the directory has mode 0700 and belongs to the image's non-root user. Raw browser
cookies are not persisted. Keep `/home/workspace` on its named volume across
container replacement. A later start uses these credentials without reading the
bootstrap source again. Invalid stored state or a changed server fails startup;
it does not silently create a different identity. Credentials are never logged.

With the default public URL `http://127.0.0.1:3460`, opening the page issues an
opaque HttpOnly, SameSite=Strict cookie automatically. The owner goes straight to
the workspace. HTTP Host must exactly match `WORKSPACE_ORIGIN`, including its
port; foreign Origin headers and cross-site fetches are rejected. WebSocket
upgrades require the exact Origin and an authenticated cookie. Use the configured
URL consistently: `localhost` and `127.0.0.1` are different Host values.

The Compose overlay publishes only on `127.0.0.1` by default. Its client profile
now needs no token file on servers that allow registration:

```sh
# Repository root; build only the allowlisted context.
podman build --format docker \
  --ignorefile extensions/claude-bridge/workspace/containerignore \
  -f extensions/claude-bridge/workspace/Dockerfile \
  -t localhost/bridgeweb-acc-workspace:local .
podman compose -p bridgeweb-acc-client \
  -f extensions/claude-bridge/server/docker-compose.workspace.yml \
  --profile client up -d
# Open http://127.0.0.1:3460
```

Set `BRIDGE_SERVER_URL` for an external server, or use `--profile combined` to
start the separate server service defined in the same overlay. The combined
server image must already be built locally. Neither mode modifies an existing
Bridge deployment. Do not run both client profiles on the same published port.
When registration is restricted, add a private local Compose override mounting
the bootstrap secret read-only and setting `BRIDGE_TOKEN_FILE`; remove that
override after the first successful start.

Remote publication requires both an exact non-loopback `WORKSPACE_ORIGIN` and
`WORKSPACE_ALLOW_REMOTE=1`. This mode retains the token sign-in page. Set
`WORKSPACE_PUBLISH_BIND` explicitly in Compose and terminate TLS at a trusted
proxy; HTTPS cookies carry Secure. Automatic login is limited to a loopback
public origin. Do not forward a loopback auto-login service through a public
proxy that rewrites Host to that origin.

Cookies remain valid for 30 days and are revalidated against the Bridge server.
Restarting or recreating the container with the same HOME volume preserves the
token, secret and existing cookies. The existing Resume action can reattach a
completed server conversation after a host restart. This does not preserve
container-local Bash processes across container shutdown. Closing just the
browser keeps the host and agent work running.

To explicitly reset credentials, stop only this workspace service and run the
same image against its exact HOME volume (substitute your own volume name):

```sh
podman run --rm --name bridgeweb-acc-credential-reset \
  -v bridgeweb-acc-client_workspace-home:/home/workspace \
  localhost/bridgeweb-acc-workspace:local --reset-credentials
```

Start the service again without a bootstrap secret to register a fresh Bridge
identity and browser secret. Old browser sessions are invalidated. With a
bootstrap secret, reset imports that supplied identity instead. Reset removes
only the two service credential/session files; it does not revoke a possibly
shared upstream token, erase projects, or delete conversation history. Revoke
an old upstream token separately through the server dashboard when appropriate.

Run the real headless Linux Podman first-visit/restart/recreate/foreign-Origin
gate with an acceptance runner image built from `acceptance/Dockerfile`:

```sh
node extensions/claude-bridge/workspace/acceptance/container-runner.mjs \
  --mode credentials --image localhost/bridgeweb-acc-workspace:local \
  --report-dir /private/bridgeweb-acc-credentials
```

The gate uses only its own `bridgeweb-acc-*` containers, fresh HOME/project
volumes and registered token, and cleans up those exact resources. It never
reads server volumes. Reports contain fixed assertions and measurements, with
no credentials or raw transcripts. Chromium runs headless in the Linux runner
with its private browser cache and `PLAYWRIGHT_SKIP_BROWSER_GC=1`.

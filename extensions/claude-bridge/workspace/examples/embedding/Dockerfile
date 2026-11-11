# Embedding example image: the application's own server, sign-in and page
# around @kaminide/bridge-workspace-host, without the KaminIDE browser UI.
# Build from the repository root (see README.md):
#   podman build --format docker \
#     --ignorefile extensions/claude-bridge/workspace/examples/embedding/containerignore \
#     -f extensions/claude-bridge/workspace/examples/embedding/Dockerfile \
#     -t localhost/kamin-embedding-example:local .
# A real application installs the two release tarballs instead of packing them.
FROM node:22-bookworm-slim AS packages
WORKDIR /opt/bridge
COPY package.json package-lock.json ./
COPY builtin-extensions/claude-bridge/package.json builtin-extensions/claude-bridge/package.json
COPY extensions/claude-bridge/extension/package*.json extensions/claude-bridge/extension/
COPY extensions/claude-bridge/workspace/package*.json extensions/claude-bridge/workspace/
COPY extensions/claude-bridge/browser-client/package*.json extensions/claude-bridge/browser-client/
RUN npm ci --omit=dev --no-audit --no-fund \
    && npm --prefix extensions/claude-bridge/extension ci --no-audit --no-fund \
    && npm --prefix extensions/claude-bridge/workspace ci --no-audit --no-fund \
    && npm --prefix extensions/claude-bridge/browser-client ci --no-audit --no-fund
COPY src/ src/
COPY scripts/release/pack-embedding.mjs scripts/release/pack-embedding.mjs
COPY extensions/claude-bridge/extension/ extensions/claude-bridge/extension/
COPY extensions/claude-bridge/workspace/ extensions/claude-bridge/workspace/
COPY extensions/claude-bridge/browser-client/ extensions/claude-bridge/browser-client/
COPY extensions/claude-bridge/webview/package.json extensions/claude-bridge/webview/package.json
COPY extensions/claude-bridge/webview/shared/ extensions/claude-bridge/webview/shared/
COPY extensions/claude-bridge/server/package.json extensions/claude-bridge/server/package.json
RUN node scripts/release/pack-embedding.mjs --out /opt/packages

FROM node:22-bookworm-slim AS runtime
# Tools the agent uses inside the workspace; add the project's own toolchain.
RUN apt-get update && apt-get install -y --no-install-recommends \
      bash ca-certificates curl git jq python3 ripgrep tini \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd --gid 10001 workspace \
    && useradd --uid 10001 --gid workspace --create-home --shell /bin/bash workspace \
    && mkdir -p /workspace /home/workspace/.local/state/bridge \
    && chown workspace:workspace /workspace /home/workspace/.local/state/bridge
WORKDIR /opt/app
COPY extensions/claude-bridge/workspace/examples/embedding/package.json extensions/claude-bridge/workspace/examples/embedding/*.mjs ./
COPY extensions/claude-bridge/workspace/examples/embedding/public/ ./public/
COPY --from=packages /opt/packages/*.tgz /opt/packages/
RUN npm install --omit=dev --no-audit --no-fund /opt/packages/kaminide-bridge-workspace-host-*.tgz /opt/packages/kaminide-bridge-browser-client-*.tgz
USER workspace
ENV NODE_ENV=production HOME=/home/workspace WORKSPACE_ROOT=/workspace \
    WORKSPACE_STORAGE=/home/workspace/.local/state/bridge PORT=8080
VOLUME ["/workspace", "/home/workspace"]
EXPOSE 8080
ENTRYPOINT ["/usr/bin/tini", "--", "node", "server.mjs"]

# Build from the repository root, using workspace/containerignore (see README).
FROM node:22-bookworm-slim AS build
WORKDIR /opt/bridge
COPY package.json package-lock.json ./
COPY builtin-extensions/claude-bridge/package.json builtin-extensions/claude-bridge/package.json
COPY extensions/claude-bridge/extension/package*.json extensions/claude-bridge/extension/
COPY extensions/claude-bridge/workspace/package*.json extensions/claude-bridge/workspace/
COPY extensions/claude-bridge/webview/package*.json extensions/claude-bridge/webview/
RUN npm ci --omit=dev --no-audit --no-fund \
    && npm --prefix extensions/claude-bridge/extension ci --no-audit --no-fund \
    && npm --prefix extensions/claude-bridge/workspace ci --no-audit --no-fund \
    && npm --prefix extensions/claude-bridge/webview ci --no-audit --no-fund
COPY src/ src/
COPY extensions/claude-bridge/extension/ extensions/claude-bridge/extension/
COPY extensions/claude-bridge/workspace/ extensions/claude-bridge/workspace/
COPY extensions/claude-bridge/webview/ extensions/claude-bridge/webview/
COPY extensions/claude-bridge/browser-client/src/ extensions/claude-bridge/browser-client/src/
COPY extensions/claude-bridge/server/src/ui/src/lib/terminal-console/ extensions/claude-bridge/server/src/ui/src/lib/terminal-console/
RUN npm --prefix extensions/claude-bridge/workspace run build \
    && npm --prefix extensions/claude-bridge/webview run build:browser \
    && npm --prefix extensions/claude-bridge/workspace prune --omit=dev --no-audit --no-fund

FROM node:22-bookworm-slim AS runtime
RUN apt-get update && apt-get install -y --no-install-recommends \
      bash ca-certificates curl git jq python3 ripgrep tini \
    && curl --fail --silent --show-error --location \
      https://packages.microsoft.com/config/debian/12/packages-microsoft-prod.deb -o /tmp/microsoft.deb \
    && dpkg -i /tmp/microsoft.deb && rm /tmp/microsoft.deb \
    && apt-get update && apt-get install -y --no-install-recommends powershell \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd --gid 10001 workspace \
    && useradd --uid 10001 --gid workspace --create-home --shell /bin/bash workspace \
    && mkdir -p /workspace /home/workspace/.local/state/bridge \
    && chown workspace:workspace /workspace /home/workspace/.local/state/bridge
WORKDIR /opt/bridge/extensions/claude-bridge/workspace
COPY --from=build /opt/bridge/node_modules /opt/bridge/node_modules
COPY --from=build /opt/bridge/extensions/claude-bridge/workspace/node_modules ./node_modules
COPY --from=build /opt/bridge/extensions/claude-bridge/workspace/dist ./dist
COPY --from=build /opt/bridge/extensions/claude-bridge/workspace/package.json ./package.json
COPY --from=build /opt/bridge/extensions/claude-bridge/webview/dist/browser ../webview/dist/browser
COPY --chmod=755 extensions/claude-bridge/workspace/container-entrypoint.sh ./container-entrypoint.sh
COPY extensions/claude-bridge/workspace/container-healthcheck.mjs ./container-healthcheck.mjs
ARG SOURCE_REVISION=unspecified
LABEL org.opencontainers.image.revision=$SOURCE_REVISION
USER workspace
ENV NODE_ENV=production HOME=/home/workspace WORKSPACE_ROOT=/workspace \
    WORKSPACE_STORAGE=/home/workspace/.local/state/bridge WORKSPACE_BIND=0.0.0.0 WORKSPACE_PORT=3460
VOLUME ["/workspace", "/home/workspace"]
EXPOSE 3460
HEALTHCHECK --interval=15s --timeout=5s --start-period=20s --retries=3 \
    CMD ["node", "container-healthcheck.mjs"]
ENTRYPOINT ["/usr/bin/tini", "--", "./container-entrypoint.sh"]

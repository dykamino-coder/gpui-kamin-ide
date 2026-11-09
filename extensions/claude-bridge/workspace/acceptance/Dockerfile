# Build from the repository root with workspace/containerignore.
FROM mcr.microsoft.com/playwright:v1.65.0-noble
ENV PLAYWRIGHT_BROWSERS_PATH=/opt/bridgeweb-acc-playwright PLAYWRIGHT_SKIP_BROWSER_GC=1
RUN mv /ms-playwright /opt/bridgeweb-acc-playwright \
    && apt-get update && apt-get install -y --no-install-recommends podman \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /opt/bridge
COPY package.json package-lock.json ./
COPY extensions/claude-bridge/extension/package*.json extensions/claude-bridge/extension/
COPY extensions/claude-bridge/workspace/package*.json extensions/claude-bridge/workspace/
COPY extensions/claude-bridge/webview/package*.json extensions/claude-bridge/webview/
RUN npm ci --omit=dev --no-audit --no-fund \
    && npm --prefix extensions/claude-bridge/extension ci --no-audit --no-fund \
    && npm --prefix extensions/claude-bridge/workspace ci --no-audit --no-fund \
    && npm --prefix extensions/claude-bridge/webview ci --no-audit --no-fund
COPY extensions/claude-bridge/workspace/acceptance/container-entry.mjs /opt/container-entry.mjs
ENTRYPOINT ["node", "/opt/container-entry.mjs"]

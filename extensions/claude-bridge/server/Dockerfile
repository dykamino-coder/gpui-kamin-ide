# =============================================================================
# Open Claude Bridge — Docker/Podman container (multi-stage)
# =============================================================================
# Stage 1 (builder): apt build tools + dev deps + Vite dashboard build.
# Stage 2 (runtime): slim Node + Claude CLI + only prod node_modules + dist.
# =============================================================================

# ---------- Stage 1: builder ------------------------------------------------
FROM node:22-bookworm-slim AS builder

RUN apt-get update && apt-get install -y --no-install-recommends \
    python3 make g++ git ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY package.json package-lock.json ./
COPY src/ui/package.json src/ui/package-lock.json src/ui/

RUN npm ci --no-audit --no-fund
RUN npm --prefix src/ui ci --no-audit --no-fund

COPY . .

RUN npm --prefix src/ui run build

# Drop dashboard node_modules — only the built dist/dashboard ships.
RUN rm -rf src/ui/node_modules

# Prune dev deps from server's node_modules.
RUN npm prune --omit=dev --no-audit --no-fund

# ---------- Stage 2: runtime -----------------------------------------------
FROM node:22-bookworm-slim AS runtime

# Claude Code CLI — PINNED. The bridge leans on undocumented CLI internals
# (IS_DEMO gate bypass, resume-threshold env vars, the project-slug algorithm,
# MITM TLS interception), so `@latest` silently drifting the CLI on every image
# rebuild is a real breakage risk — e.g. 2.1.199 made SSL cert errors fail
# immediately (a MITM CA misconfig is now instantly fatal instead of retried).
# Bump this deliberately + re-run the live smoke (spawn → JSONL slug → MITM
# handshake → hook round-trip) before shipping a new pin.
# + tsx (a devDependency, but the runtime CMD invokes it via npx; install it
# globally so the container doesn't pull it from the network on first start).
ARG CLAUDE_CODE_VERSION=2.1.236
ARG CLAUDE_CODE_CACHE_BUST=1
RUN npm install -g "@anthropic-ai/claude-code@${CLAUDE_CODE_VERSION}" tsx@4 \
    && npm cache clean --force \
    && rm -rf /tmp/* /root/.npm

# Non-root user (Claude Code refuses --dangerously-skip-permissions as root).
RUN groupadd -r bridge && useradd -r -g bridge -m -d /home/bridge -s /bin/bash bridge \
    && mkdir -p /app/data /home/bridge/.claude \
    && chown -R bridge:bridge /app /home/bridge

WORKDIR /app

# Copy artifacts from builder with --chown (one layer, no extra recursive chown).
COPY --from=builder --chown=bridge:bridge /app/node_modules ./node_modules
COPY --from=builder --chown=bridge:bridge /app/dist ./dist
COPY --from=builder --chown=bridge:bridge /app/src ./src
COPY --from=builder --chown=bridge:bridge /app/bin ./bin
COPY --from=builder --chown=bridge:bridge /app/package.json /app/package-lock.json /app/tsconfig.json ./
COPY --chown=bridge:bridge --chmod=755 docker-entrypoint.sh /app/docker-entrypoint.sh

# KaminIDE installer (the GPUI desktop client) — the
# ONLY installer shipped. `/download` serves the setup.exe for fresh installs;
# `/updates/kaminide` serves it to the in-app GPUI updater. The
# legacy Electron Squirrel artifacts (RELEASES / *.nupkg / *Setup.exe) are
# deliberately no longer baked — KaminIDE fully replaces the Electron client.
# The trailing `*` also tolerates legacy .exe.sig files in existing releases.
COPY --chown=bridge:bridge installer/KaminIDE_*_x64-setup.exe* /app/installer/

USER bridge

ENV NODE_ENV=production
ENV CLAUDE_PROXY_HOST=0.0.0.0
ENV CLAUDE_PROXY_PORT=3456
ENV HOME=/home/bridge
# libuv threadpool default is 4 — too small for our stack (DuckDB pool=8 +
# node-pty + native-mitm TLS + webcrypto leaf-cert + fs.promises). At 4
# threads, fs.createReadStream readers can starve and never emit. Must be
# set BEFORE any native module loads, hence at process env level.
ENV UV_THREADPOOL_SIZE=16

EXPOSE 3456

VOLUME ["/app/data", "/home/bridge/.claude"]

ENTRYPOINT ["/app/docker-entrypoint.sh"]
CMD ["npx", "tsx", "bin/claude-bridge.ts"]

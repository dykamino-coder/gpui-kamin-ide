// ============================================================================
// HTTP Server (Hono)
// ============================================================================

import { Hono, type Context } from "hono"
import { cors } from "hono/cors"
import { serveStatic } from "@hono/node-server/serve-static"
import fs from "fs"
import path from "path"
import { handleHealth } from "./handlers"
import { handleMcpRequest, handleMcpGet, handleMcpDelete } from "../pty/mcp-http-handler"
import { createDashboardRoutes } from "../dashboard/routes"
import { createOtelRoutes } from "../telemetry/otlp-receiver"
import { createTestRoutes } from "../test-api/routes"
import { createSyncRoutes } from "../sync/routes"
import { createHooksRoutes } from "../hooks/routes"
import { requestContextMiddleware, errorHandlerMiddleware } from "../middleware"
import { VERSION } from "../config"

/** Find latest desktop-app installer in ./installer/ (semver-aware sort).
 *  Matches both the KaminIDE Tauri NSIS setup (`KaminIDE_X.Y.Z_x64-setup.exe`,
 *  lowercase "setup") and the legacy Bridge Squirrel installer (`…Setup.exe`);
 *  prefers a KaminIDE build when both are present. */
function findInstaller(): { path: string; name: string } | null {
  const dirs = ["./installer", "./dist/electron"]
  for (const dir of dirs) {
    const resolved = path.resolve(dir)
    if (!fs.existsSync(resolved)) continue
    let files = fs.readdirSync(resolved).filter(f => f.endsWith(".exe") && /setup/i.test(f))
    // Prefer KaminIDE installers over any legacy Bridge Setup.exe still present.
    const kamin = files.filter(f => /kamin/i.test(f))
    if (kamin.length > 0) files = kamin
    if (files.length > 0) {
      // Sort by version number (extract digits after "Bridge-" and compare as semver)
      files.sort((a, b) => {
        const va = a.match(/(\d+)\.(\d+)\.(\d+)/)
        const vb = b.match(/(\d+)\.(\d+)\.(\d+)/)
        if (!va || !vb) return a.localeCompare(b)
        for (let i = 1; i <= 3; i++) {
          const diff = parseInt(va[i]!) - parseInt(vb[i]!)
          if (diff !== 0) return diff
        }
        return 0
      })
      const latest = files[files.length - 1]
      if (!latest) return null
      return { path: path.join(resolved, latest), name: latest }
    }
  }
  return null
}

export function createApp() {
  const app = new Hono()

  // CORS — restricted to localhost origins
  app.use("*", cors({
    origin: ['http://localhost:3456', 'http://localhost:5173', 'http://127.0.0.1:3456'],
    credentials: true,
  }))

  // Security headers
  app.use('*', async (c, next) => {
    await next()
    c.header('X-Content-Type-Options', 'nosniff')
    c.header('X-Frame-Options', 'DENY')
  })

  // Request context — attaches reqCtx to every request (includes token auth)
  app.use("*", requestContextMiddleware)

  // Error handler — catches thrown errors and returns formatted JSON
  app.use("*", errorHandlerMiddleware)

  // Global health
  app.get("/", handleHealth)
  app.get("/health", handleHealth)

  // MCP HTTP endpoint — Claude CLI sends JSON-RPC here for tool calls (Streamable HTTP)
  app.post("/mcp/:sessionId", handleMcpRequest)
  app.get("/mcp/:sessionId", handleMcpGet)
  app.delete("/mcp/:sessionId", handleMcpDelete)

  // OTel OTLP receiver — Claude Code CLI sends metrics and logs here
  app.route("/otel", createOtelRoutes())

  // Test API — for testing MCP/elicitation without real Claude CLI
  if (process.env.NODE_ENV !== 'production') {
    app.route("/", createTestRoutes())
  }

  // Per-token sync API (user-level and project-level file sync)
  app.route("/", createSyncRoutes())

  // Hook proxy — CLI POSTs here when a rewritten hook fires; bridge
  // dispatches to local Electron / server / external HTTP per resolved host.
  app.route("/", createHooksRoutes())

  // Dashboard REST API
  app.route("/", createDashboardRoutes())

  // UI static files (Vite build output) — SPA fallback for client-side routing
  app.get("/ui", serveStatic({ path: "./dist/dashboard/index.html" }))
  app.get("/ui/*", async (c, next) => {
    // Try serving static file first
    const staticPath = c.req.path.replace(/^\/ui/, '')
    if (staticPath.startsWith('/assets/') || staticPath.includes('.')) {
      return serveStatic({ root: "./dist/dashboard", rewriteRequestPath: (path) => path.replace(/^\/ui/, '') })(c, next)
    }
    // SPA fallback — serve index.html for all routes
    return serveStatic({ path: "./dist/dashboard/index.html" })(c, next)
  })

  // Public download landing page — unauthenticated, separate from /ui
  // (which is the dashboard and may be locked behind DASHBOARD_USER /
  // DASHBOARD_PASSWORD). Anyone with a server URL can grab the
  // installer for the Electron client without minting a token first.
  app.get("/download", (c) => {
    const installerFile = findInstaller()
    const v = VERSION
    const exists = !!installerFile
    const fileName = installerFile?.name ?? null
    const sizeMb = installerFile ? (fs.statSync(installerFile.path).size / 1024 / 1024).toFixed(1) : null
    const html = `<!DOCTYPE html>
<html lang="en"><head>
<meta charset="utf-8" />
<meta name="viewport" content="width=device-width,initial-scale=1" />
<title>KaminIDE — Download</title>
<style>
  :root { color-scheme: dark; }
  * { box-sizing: border-box }
  body {
    margin: 0; min-height: 100vh;
    font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", system-ui, sans-serif;
    background: radial-gradient(circle at top, #1e1e2e 0%, #11111b 60%);
    color: #cdd6f4;
    display: flex; align-items: center; justify-content: center; padding: 24px;
  }
  .card {
    background: #181825;
    border: 1px solid #313244;
    border-radius: 16px;
    padding: 40px 48px;
    max-width: 540px; width: 100%;
    box-shadow: 0 18px 60px rgba(0,0,0,0.45);
  }
  h1 { margin: 0 0 6px; font-size: 26px; font-weight: 700; letter-spacing: -0.01em }
  .sub { color: #a6adc8; font-size: 14px; margin: 0 0 28px; line-height: 1.55 }
  .meta { display: flex; gap: 16px; flex-wrap: wrap; font-size: 12px; color: #6c7086; margin-bottom: 24px }
  .meta span { display: inline-flex; align-items: center; gap: 6px }
  .meta b { color: #cdd6f4; font-weight: 600 }
  .btn {
    display: inline-flex; align-items: center; gap: 10px;
    padding: 14px 22px; border-radius: 10px;
    background: #89b4fa; color: #11111b;
    font-size: 15px; font-weight: 700; text-decoration: none;
    border: none; cursor: pointer;
    transition: background 0.15s, transform 0.1s;
  }
  .btn:hover { background: #74a4f5 }
  .btn:active { transform: scale(0.98) }
  .btn-disabled {
    background: #313244; color: #6c7086; cursor: not-allowed; pointer-events: none;
  }
  .hint {
    margin-top: 18px; padding: 12px 14px; border-radius: 8px;
    background: #1e1e2e; border-left: 3px solid #f9e2af;
    font-size: 12px; color: #a6adc8; line-height: 1.55;
  }
  .footer { margin-top: 28px; font-size: 11px; color: #45475a; text-align: center }
  .footer a { color: #585b70; text-decoration: none }
  .footer a:hover { color: #89b4fa }
</style>
</head><body>
<div class="card">
  <h1>KaminIDE</h1>
  <p class="sub">Native code editor with Claude built in — connects to this server and drives a Claude CLI session over PTY, with a full extension host, terminal, and folder workspaces. Available for Windows.</p>
  <div class="meta">
    <span>Server <b>v${v}</b></span>
    ${exists ? `<span>Installer <b>${fileName}</b></span><span><b>${sizeMb} MB</b></span>` : ''}
  </div>
  ${exists
    ? `<a class="btn" href="/download/electron" download><svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round" stroke-linejoin="round"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><polyline points="7 10 12 15 17 10"/><line x1="12" y1="15" x2="12" y2="3"/></svg>Download for Windows</a>`
    : `<button class="btn btn-disabled" disabled>Installer not available on this server</button>`
  }
  <div class="hint">
    After install: open KaminIDE, set the server URL <b>${c.req.url.replace(/\/download.*$/, '')}</b> and an API token (managed in the dashboard). Right-click any folder in Explorer → <b>Open with KaminIDE</b> to start a workspace.
  </div>
  <div class="footer">
    <a href="/ui">Dashboard</a> · <a href="/health">Health</a>
  </div>
</div>
</body></html>`
    c.header("Content-Type", "text/html; charset=utf-8")
    return c.body(html)
  })

  // Electron installer download
  app.get("/download/electron", (c) => {
    const installerFile = findInstaller()
    if (!installerFile) {
      return c.text("Installer not available", 404)
    }
    const stat = fs.statSync(installerFile.path)
    const stream = fs.createReadStream(installerFile.path)
    // Without this, a file deleted/unmounted mid-download emits an unhandled
    // 'error' on the stream → uncaughtException → whole-server shutdown.
    stream.on("error", () => { try { stream.destroy() } catch { /* gone */ } })
    c.header("Content-Type", "application/octet-stream")
    c.header("Content-Disposition", `attachment; filename="${installerFile.name}"`)
    c.header("Content-Length", String(stat.size))
    return c.body(stream as unknown as ReadableStream)
  })

  // Check if installer is available
  app.get("/api/download/check", (c) => {
    const installerFile = findInstaller()
    return c.json({
      available: !!installerFile,
      size: installerFile ? fs.statSync(installerFile.path).size : 0,
      name: installerFile?.name ?? null,
    })
  })

  // ── Auto-update feed ────────────────────────────────────────────────────
  // `VERSION` is the server's build. Electron polls this and, if it's older,
  // swaps its sidebar version label for an "Update to X.Y.Z" button. The
  // button hits the Squirrel feed below (RELEASES + .nupkg) via the built-in
  // `electron.autoUpdater`.
  app.get("/api/bridge-version", (c) => {
    const installerFile = findInstaller()
    return c.json({
      version: VERSION,
      installerAvailable: !!installerFile,
      feedUrl: `/updates/win32`,
      installerName: installerFile?.name ?? null,
    })
  })

  // Squirrel update feed for Windows. The installer dir already contains
  // `RELEASES` + `*.nupkg` produced by `npm run make` — the bridge container
  // bakes those alongside the Setup.exe.
  function serveFromInstaller(c: any, fileName: string, contentType: string) {
    const dirs = ["./installer", "./dist/electron"]
    for (const dir of dirs) {
      const full = path.resolve(dir, fileName)
      if (fs.existsSync(full)) {
        const stat = fs.statSync(full)
        c.header("Content-Type", contentType)
        c.header("Content-Length", String(stat.size))
        const s = fs.createReadStream(full)
        s.on("error", () => { try { s.destroy() } catch { /* gone */ } })
        return c.body(s as unknown as ReadableStream)
      }
    }
    return c.text("Not found", 404)
  }
  app.get("/updates/win32/RELEASES", (c) => {
    return serveFromInstaller(c, "RELEASES", "text/plain")
  })
  app.get("/updates/win32/:file{.+\\.nupkg}", (c) => {
    const file = c.req.param("file")
    if (file.includes("..") || file.includes("/") || file.includes("\\")) {
      return c.text("Invalid path", 400)
    }
    return serveFromInstaller(c, file, "application/octet-stream")
  })

  // ── KaminIDE self-update feed ───────────────────────────────────────────
  // The app polls this with its target/arch/current version. We compare against
  // the KaminIDE installer sitting in ./installer/ and either return 204 (up to
  // date) or an update manifest. Поле `signature`/.sig убрано в 6.3.92:
  // это был рудимент tauri-plugin-updater — GPUI-клиент подпись не проверяет,
  // а «требование .sig» лишь блокировало выдачу манифеста.
  app.get("/updates/kaminide/:target/:arch/:current", (c) => {
    const info = findKaminInstaller()
    if (!info) return c.body(null, 204) // nothing to offer
    const current = c.req.param("current")
    if (!isNewer(info.version, current)) return c.body(null, 204) // already current
    const base = c.req.url.replace(/\/updates\/kaminide\/.*$/, "")
    return c.json({
      version: info.version,
      pub_date: new Date(fs.statSync(info.path).mtimeMs).toISOString(),
      // Имя файла в URL обязательно: клиент сохраняет скачанное под последним
      // сегментом пути — без него получался `%TEMP%\artifact` без .exe,
      // который Windows отказывался запускать (установка молча не происходила).
      url: `${base}/updates/kaminide/artifact/${encodeURIComponent(info.name)}`,
      notes: `KaminIDE ${info.version}`,
    })
  })

  // The update artifact itself (the NSIS setup.exe the updater downloads+runs).
  // Также по /artifact/:name — имя игнорируется (отдаём текущий инсталлер),
  // оно нужно клиенту как имя сохраняемого файла.
  app.get("/updates/kaminide/artifact/:name", (c) => serveKaminArtifact(c))
  app.get("/updates/kaminide/artifact", (c) => serveKaminArtifact(c))
  const serveKaminArtifact = (c: Context) => {
    const info = findKaminInstaller()
    if (!info) return c.text("No update artifact", 404)
    const stat = fs.statSync(info.path)
    c.header("Content-Type", "application/octet-stream")
    c.header("Content-Disposition", `attachment; filename="${info.name}"`)
    c.header("Content-Length", String(stat.size))
    const s = fs.createReadStream(info.path)
    s.on("error", () => { try { s.destroy() } catch { /* gone */ } })
    return c.body(s as unknown as ReadableStream)
  }

  // Legacy redirect /dashboard → /ui
  app.get("/dashboard", (c) => c.redirect("/ui"))
  app.get("/dashboard/*", (c) => c.redirect(c.req.path.replace(/^\/dashboard/, '/ui')))

  return app
}

/** KaminIDE installer only (Tauri NSIS `KaminIDE_X.Y.Z_x64-setup.exe`) + its
 *  parsed version — the updater manifest needs the exact version. */
function findKaminInstaller(): { path: string; name: string; version: string } | null {
  for (const dir of ["./installer", "./dist/electron"]) {
    const resolved = path.resolve(dir)
    if (!fs.existsSync(resolved)) continue
    const files = fs.readdirSync(resolved).filter(f => /kamin/i.test(f) && /setup/i.test(f) && f.endsWith(".exe"))
    let best: { path: string; name: string; version: string } | null = null
    for (const f of files) {
      const m = f.match(/(\d+)\.(\d+)\.(\d+)/)
      if (!m) continue
      const version = `${m[1]}.${m[2]}.${m[3]}`
      if (!best || isNewer(version, best.version)) best = { path: path.join(resolved, f), name: f, version }
    }
    if (best) return best
  }
  return null
}

/** True iff semver `a` > `b` (plain numeric X.Y.Z; missing parts → 0). */
function isNewer(a: string, b: string): boolean {
  const pa = a.split(".").map(n => parseInt(n, 10) || 0)
  const pb = b.split(".").map(n => parseInt(n, 10) || 0)
  for (let i = 0; i < 3; i++) {
    const d = (pa[i] ?? 0) - (pb[i] ?? 0)
    if (d !== 0) return d > 0
  }
  return false
}

export type App = ReturnType<typeof createApp>

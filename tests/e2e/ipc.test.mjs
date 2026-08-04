#!/usr/bin/env node
// Bridge round-trip e2e (Tauri era). Spawns the packaged Tauri binary with
// WebView2 remote-debugging on, attaches Playwright over CDP, and asserts the
// two halves of the renderer's data path:
//   • window.kamin   — the Tauri shell bridge (layout persistence via invoke,
//     plus the preload entries that now reject because the data plane moved).
//   • hostRpc WS     — registry/commands/extensions arrive over the loopback
//     WebSocket from kamin-host; we assert the renderer received them (the
//     footer's live command count) rather than reaching into the module.
//
// Why .mjs not vitest: vitest can't drive a real WebView2 browser context.
// Single-file harness, exits 0/1.
//
// Usage: npm run tauri:build (or tauri:build:dev)  →  node tests/e2e/ipc.test.mjs
import { dirname, resolve } from "node:path"
import { fileURLToPath } from "node:url"
import { spawn } from "node:child_process"
import { chromium } from "playwright"
import { findTauriBinary } from "../../scripts/find-tauri-binary.mjs"

const REPO = resolve(dirname(fileURLToPath(import.meta.url)), "..", "..")
const CDP_PORT = 9334 // distinct from perf probes (9333) so concurrent runs don't clash
const LAUNCH_TIMEOUT_MS = 25_000
const WS_READY_TIMEOUT_MS = 20_000
const VIA_HOST_WS = "hostrpc" // substring (lower-cased) of the bridge's reject message
const sleep = (ms) => new Promise((r) => setTimeout(r, ms))

async function appPage(browser, deadline) {
  for (;;) {
    for (const ctx of browser.contexts()) {
      for (const p of ctx.pages()) {
        if ((await p.title().catch(() => "")) === "KaminIDE") return p
      }
    }
    if (performance.now() > deadline) throw new Error("KaminIDE page never appeared")
    await sleep(40)
  }
}

const found = findTauriBinary(REPO)
if (!found) {
  console.error("ipc.test: kaminide.exe not found under src-tauri/target — run `npm run tauri:build` first")
  process.exit(2)
}
console.info(`ipc.test: launching ${found.path}${found.release ? "" : " (debug build)"}`)

const proc = spawn(found.path, [], {
  env: { ...process.env, WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-port=${CDP_PORT}` },
  stdio: "ignore",
})

let failed = 0
async function check(name, fn) {
  try { await fn(); console.info(`  ✓ ${name}`) }
  catch (err) { console.error(`  ✗ ${name} — ${err.message}`); failed++ }
}

let browser
try {
  const deadline = performance.now() + LAUNCH_TIMEOUT_MS
  for (;;) {
    try { browser = await chromium.connectOverCDP(`http://localhost:${CDP_PORT}`); break }
    catch { if (performance.now() > deadline) throw new Error("CDP endpoint never came up"); await sleep(40) }
  }
  const page = await appPage(browser, deadline)
  await page.waitForLoadState("domcontentloaded")

  await check("window.kamin bridge is installed", async () => {
    const shape = await page.evaluate(() => ({
      kamin: typeof window.kamin,
      layoutStore: typeof window.kamin?.layoutStore?.get,
      registry: typeof window.kamin?.registry?.snapshot,
    }))
    if (shape.kamin !== "object") throw new Error(`window.kamin is ${shape.kamin}`)
    if (shape.layoutStore !== "function") throw new Error("layoutStore.get missing")
    if (shape.registry !== "function") throw new Error("registry.snapshot missing")
  })

  await check("layoutStore round-trips through the Rust invoke bridge", async () => {
    const orig = await page.evaluate(() => window.kamin.layoutStore.get())
    if (!orig || typeof orig !== "object") throw new Error(`layout_get returned ${JSON.stringify(orig)}`)
    // Пробуем НЕЙТРАЛЬНЫЙ ключ вне схемы: sidebarWidthPx конкурентно пишет
    // живой renderer при буте/ресайзе, и тест гонялся с ним (записали 321,
    // прочитали ширину рендерера — флак). layout_set — shallow-merge JSON,
    // так что чужой ключ прокатывается тем же мостом и ничем не перебивается.
    const probe = Date.now() % 100000
    await page.evaluate((v) => window.kamin.layoutStore.set({ e2eProbe: v }), probe)
    const after = await page.evaluate(() => window.kamin.layoutStore.get())
    if (after.e2eProbe !== probe) throw new Error(`expected ${probe}, got ${after.e2eProbe}`)
    await page.evaluate(() => window.kamin.layoutStore.set({ e2eProbe: null })) // cleanup
  })

  await check("registry/commands moved to the host WS (preload entries reject)", async () => {
    const msg = await page.evaluate(() =>
      window.kamin.registry.snapshot().then(() => null, (e) => String(e?.message ?? e)))
    if (!msg || !msg.toLowerCase().includes(VIA_HOST_WS)) {
      throw new Error(`expected a hostRpc-redirect rejection, got ${JSON.stringify(msg)}`)
    }
  })

  await check("host WS data plane delivered the command registry", async () => {
    // The footer shows a live "N cmds" count, fed by hostRpc.registry over the
    // loopback WS once kamin-host's ext-host child activates. Poll until > 0.
    const deadline2 = performance.now() + WS_READY_TIMEOUT_MS
    for (;;) {
      const n = await page.evaluate(() => {
        const btn = [...document.querySelectorAll("button")].find((b) => /Registered commands/i.test(b.getAttribute("aria-label") || ""))
        const m = /(\d+)\s*cmds/.exec(btn?.textContent || "")
        return m ? Number(m[1]) : 0
      })
      if (n > 0) return
      if (performance.now() > deadline2) throw new Error("command count stayed 0 — host WS never delivered the registry")
      await sleep(120)
    }
  })

  await check("command palette is wired (entry button present)", async () => {
    const has = await page.evaluate(() =>
      [...document.querySelectorAll("button")].some((b) => /command palette/i.test(b.getAttribute("aria-label") || "")))
    if (!has) throw new Error("command palette button not found")
  })
} catch (err) {
  console.error(`ipc.test: harness error — ${err.message}`)
  failed++
} finally {
  await browser?.close().catch(() => { /* ignore */ })
  proc.kill() // the Job Object tears down WebView2 + the host/ext-host node tree
  await sleep(500)
}

if (failed > 0) { console.error(`\nipc.test: FAIL — ${failed} assertion(s) failed`); process.exit(1) }
console.info("\nipc.test: PASS")

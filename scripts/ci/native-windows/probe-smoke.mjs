// Exercise the existing native probe, CEF webview and host through their actual protocols.
import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import { readFile, writeFile } from "node:fs/promises";
import path from "node:path";
import { setTimeout as delay } from "node:timers/promises";
import { promisify } from "node:util";

const [scenario, capture, output] = process.argv.slice(2);
const run = promisify(execFile);
const manifest = JSON.parse(await readFile(path.join(capture, "manifest.json"), "utf8"));
const report = { candidate: manifest.candidate_sha, exeSha256: manifest.source_exe_sha256,
  scenario, class: "hosted-windows-debug", windows: process.env.ImageVersion, steps: [] };
const origin = `http://127.0.0.1:${Number(process.env.CLAUDE_PROXY_PORT)}`;

async function probe(req) {
  const { stdout } = await run(process.execPath, ["scripts/probe.mjs", JSON.stringify(req)],
    { timeout: 12500, windowsHide: true });
  return JSON.parse(stdout);
}

async function step(name, check, retry = false) {
  const started = Date.now();
  const record = { name, status: "running" };
  report.steps.push(record);
  while (true) {
    try {
      await check();
      record.status = "passed";
      break;
    } catch (err) {
      if (!retry || Date.now() - started > 55000) {
        record.status = "failed";
        // Never export command lines, probe replies, profile contents or model messages.
        record.error = err.code === "ERR_ASSERTION" ? "unexpected response" : "request failed";
        throw new Error(name);
      }
      await delay(750);
    }
  }
  record.ms = Date.now() - started;
}

const rpc = (method, params = []) => probe({ cmd: "rpc", method, params });
const web = (body) => probe({ cmd: "wveval", id: "claudeBridgeChat", body, timeoutMs: 6000 });
try {
  await step("exact-debug-candidate", async () => {
    assert.equal(manifest.candidate_sha, process.env.SMOKE_SHA);
    assert.equal(manifest.with_host, true);
    assert.equal((await probe({ cmd: "nativeAcceptance", action: "status" })).debug, true);
  }, true);
  await step("gpui-window-layout", async () => {
    const { bounds } = await probe({ cmd: "metric", id: "status-bar" });
    assert.ok(bounds.w > 0 && bounds.h > 0);
  }, true);
  await step("extension-host-bridge-activation", async () => {
    const { value } = await rpc("kamin:extensions:list");
    assert.ok(value.some(e => e.id === "dykamino-studio.claude-bridge" && e.active && !e.activationError));
  }, true);
  await step("show-bridge-view", () => rpc("kamin:command:execute", ["claude-bridge.openChat"]));
  await step("cef-chat-renderer", async () => {
    const { value } = await web("return document.readyState === 'complete' && typeof window.bridgeCmd?.raw === 'function'");
    assert.equal(value, true);
  }, true);
  if (scenario === "bridge-roundtrip") {
    await step("loopback-bridge-health", async () => {
      const reply = await fetch(`${origin}/health`, {signal: AbortSignal.timeout(3000)});
      assert.ok(reply.ok);
      assert.equal((await reply.json()).status, "ok");
    }, true);
    await step("cef-extension-host-bridge-roundtrip", async () => {
      const { value } = await web(`return await window.bridgeCmd.raw('serverFetch', ${JSON.stringify(origin)}, '/health')`);
      assert.equal(value.status, 200);
      assert.equal(value.data.status, "ok");
      const pkg = JSON.parse(await readFile("extensions/claude-bridge/server/package.json", "utf8"));
      assert.equal(value.data.version, pkg.version);
    }, true);
  }
  report.status = "passed";
} catch (error) {
  report.status = "failed";
  process.exitCode = 1;
  console.error(`Failed native acceptance boundary: ${error.message}`);
} finally {
  // Only the application window is captured, never the full desktop.
  try {
    const shot = await probe({ cmd: "screenshot", path: path.join(output, "window.png") });
    report.window = { width: shot.w, height: shot.h };
  } catch { report.window = null; }
  await writeFile(path.join(output, "report.json"), JSON.stringify(report, null, 2) + "\n");
  console.log(JSON.stringify(report));
}

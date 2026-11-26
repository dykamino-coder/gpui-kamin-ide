// Exercise the existing native probe, CEF webview and host through their actual protocols.
import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import { open, readFile, writeFile } from "node:fs/promises";
import path from "node:path";
import { setTimeout as delay } from "node:timers/promises";
import { promisify } from "node:util";
import { checkWorkspaceLink, createWorkspaceFixture } from "./workspace-link.mjs";

const [scenario, capture, output] = process.argv.slice(2);
const run = promisify(execFile);
const manifest = JSON.parse(await readFile(path.join(capture, "manifest.json"), "utf8"));
const report = { candidate: manifest.candidate_sha, exeSha256: manifest.source_exe_sha256,
  scenario, class: "hosted-windows-debug", windows: process.env.ImageVersion, steps: [] };
const origin = `http://127.0.0.1:${Number(process.env.CLAUDE_PROXY_PORT)}`;

async function lastLogBytes(file) {
  const handle = await open(file, "r").catch(() => null);
  if (!handle) return "";
  try {
    const { size } = await handle.stat();
    const buf = Buffer.alloc(Math.min(size, 128 * 1024));
    const { bytesRead } = await handle.read(buf, 0, buf.length, size - buf.length);
    return buf.toString("utf8", 0, bytesRead);
  } finally { await handle.close(); }
}

async function probe(req) {
  try {
    const { stdout } = await run(process.execPath, ["scripts/probe.mjs", JSON.stringify(req)],
      { timeout: 12500, windowsHide: true });
    return JSON.parse(stdout);
  } catch (error) {
    const output = (error.stdout ?? "") + (error.stderr ?? "");
    // Map only known public diagnostics. The raw response can contain profile data.
    const reason = ["ECONNREFUSED", "view is not live", "probe: timeout", "ws not connected",
      "event channel not ready", "unknown region", "timeout"].find(value => output.includes(value));
    throw Object.assign(new Error("Probe call failed"), {reason: reason ?? "probe error"});
  }
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
        record.error = err.reason ?? (err.code === "ERR_ASSERTION" ? "unexpected response" : "request failed");
        record.ms = Date.now() - started;
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
  if (scenario !== "native-client") {
    const fixture = scenario === "workspace-link" ? await createWorkspaceFixture(capture) : null;
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
    let tab;
    await step("create-synthetic-codex-chat", async () => {
      // Keep the disposable Bridge credential inside the isolated renderer.
      const { value } = await web(`const origin = ${JSON.stringify(origin)};
        const { token } = await window.bridgeCmd.raw('createToken', origin, 'native-smoke');
        return await window.bridgeCmd.raw('createTab', { serverUrl: origin, token, engine: 'codex',
          cwd: ${JSON.stringify(fixture?.directory ?? "")} })`);
      assert.equal(typeof value, "string");
      tab = value;
    });
    await step("codex-chat-prompt-ready", async () => {
      const { value } = await web("return window.bridgeCmd.state()");
      assert.equal(value.activeTabId, tab);
      assert.equal(value.promptReady, true);
    }, true);
    const marker = "native smoke roundtrip";
    await step("submit-synthetic-turn", async () => {
      const { value } = await web(`return window.bridgeCmd.send(${JSON.stringify(marker)})`);
      assert.equal(value, tab);
    });
    await step("render-synthetic-answer", async () => {
      const { value } = await web("return window.bridgeCmd.entries(12)");
      assert.equal(value.settling, false);
      assert.ok(value.items.some(row => row.type === "assistant" && row.text.startsWith(`echo: ${marker} [model=`)));
    }, true);
    if (fixture) await checkWorkspaceLink({ fixture, step, probe, web, report });
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
  const diag = path.join(capture, "LOCALAPPDATA", "kaminide-gpui-dev", "cache", "diag.log");
  const logs = (await Promise.all([diag, path.join(capture, "stderr.log"), path.join(capture, "stdout.log")].map(lastLogBytes))).join("\n");
  report.runtime = {
    cefInitialized: /\[cef\] initialize=1\b/.test(logs),
    microsoftSoftwareAdapterObserved: /Microsoft Basic Render Driver/i.test(logs),
    hostChildReadyLogged: /\bCHILD_READY\b/.test(logs),
    rustPanicLogged: /\bpanicked at\b/.test(logs),
  };
  await writeFile(path.join(output, "report.json"), JSON.stringify(report, null, 2) + "\n");
  console.log(JSON.stringify(report));
}

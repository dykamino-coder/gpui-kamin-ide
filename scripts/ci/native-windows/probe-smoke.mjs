// Exercise the existing native probe, CEF webview and host through their actual protocols.
import assert from "node:assert/strict";
import { readFile, writeFile } from "node:fs/promises";
import path from "node:path";
import { setTimeout as delay } from "node:timers/promises";
import { probe } from "./native-probe.mjs";
import { readRuntimeDiagnostics } from "./runtime-diagnostics.mjs";
import { registerClient } from "./cef-input.mjs";
import { checkConsoleScroll } from "./console-scroll.mjs";
import { checkEngineMenus } from "./engine-menus.mjs";
import { checkWorkspaceLink, createWorkspaceFixture } from "./workspace-link.mjs";

const [scenario, capture, output] = process.argv.slice(2);
const manifest = JSON.parse(await readFile(path.join(capture, "manifest.json"), "utf8"));
const installed = scenario === "installed-client";
const behavior = installed ? process.env.INSTALLED_SCENARIO ?? "native-client" : scenario;
const report = { candidate: manifest.candidate_sha, exeSha256: manifest.source_exe_sha256,
  scenario, behavior, class: installed ? "hosted-windows-installed-release" : "hosted-windows-debug", windows: process.env.ImageVersion, steps: [] };
const origin = `http://127.0.0.1:${Number(process.env.CLAUDE_PROXY_PORT)}`;

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
  await step(installed ? "exact-installed-candidate" : "exact-debug-candidate", async () => {
    assert.equal(manifest.candidate_sha, process.env.SMOKE_SHA);
    if (installed) {
      const pong = await probe({ cmd: "ping" });
      assert.equal(pong.app, "kaminide-gpui");
      assert.equal(pong.version, manifest.version);
    } else {
      assert.equal(manifest.with_host, true);
      assert.equal((await probe({ cmd: "nativeAcceptance", action: "status" })).debug, true);
    }
  }, true);
  await step("gpui-window-layout", async () => {
    const { bounds } = await probe({ cmd: "metric", id: "status-bar" });
    assert.ok(bounds.w > 0 && bounds.h > 0);
  }, true);
  if (installed) {
    await step("capture-version-shown-in-status-bar", async () => {
      await probe({ cmd: "shotRegion", id: "status-version", path: path.join(output, "version.png") });
    }, true);
  }
  await step("extension-host-bridge-activation", async () => {
    const { value } = await rpc("kamin:extensions:list");
    assert.ok(value.some(e => e.id === "dykamino-studio.claude-bridge" && e.active && !e.activationError));
  }, true);
  if (behavior !== "native-client") {
    // Hosted monitors can start at 1024x768. Use the real shell resize so panels
    // and modals have enough space for visible, hit-tested CEF mouse input.
    await step("size-workspace-for-interaction", () => probe({ cmd: "resize", w: 1800, h: 1050 }));
  }
  // The welcome screen has no active IDE session and does not mount the chat view.
  // Open the link fixture as the native workspace, just as "New session in folder" does.
  const fixture = behavior === "workspace-link" ? await createWorkspaceFixture(capture) : null;
  if (fixture) await step("open-fixture-ide-session", () => rpc("kamin:sessions:newSessionInFolder", [fixture.directory]));
  else await step("start-empty-ide-session", () => rpc("kamin:sessions:newNoFolderSession"));
  await step("show-bridge-view", () => rpc("kamin:command:execute", ["claude-bridge.openChat"]));
  await step("cef-chat-renderer", async () => {
    const { value } = await web("return document.readyState === 'complete' && typeof window.bridgeCmd?.raw === 'function'");
    assert.equal(value, true);
  }, true);
  if (behavior !== "native-client") {
    // Some hosted displays clamp the outer window. Hide the right column through
    // the normal layout action so the chat and file editor remain usable.
    await step("prepare-chat-controls", async () => {
      const { value } = await web("return innerWidth");
      if (value < 400) {
        const { bounds } = await probe({ cmd: "metric", id: "right-panel-column" });
        if (bounds.w > 0) await probe({ cmd: "emit", kind: "layoutFlag", name: "right" });
      }
    });
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
    const driver = { step, probe, web, report };
    await registerClient(driver, origin);
    let tab;
    let previousTab;
    await step("create-synthetic-codex-chat", async () => {
      // This is the native IDE session command. A bare extension tab can be
      // rebound to the previous IDE session when its activation finishes.
      previousTab = (await web("return window.bridgeCmd.state().activeTabId")).value;
      await web("window.bridgeCmd.raw('newAgentSession','codex','gpt-fake'); return true");
    });
    await step("codex-chat-prompt-ready", async () => {
      const { value } = await web(`const s=window.bridgeCmd.state();
        const row=(await window.bridgeCmd.raw('listTabs')).find(t=>t.id===s.activeTabId);
        return {id:s.activeTabId, ready:s.promptReady, engine:row?.engine, connected:row?.status==='connected'}`);
      report.activeEngine = ["claude","codex"].includes(value?.engine) ? value.engine : null;
      assert.ok(value?.id && value.id !== previousTab && value.ready && value.connected && value.engine === "codex");
      tab = value.id;
    }, true);
    const marker = "native smoke roundtrip";
    await step("submit-synthetic-turn", async () => {
      const { value } = await web(`return window.bridgeCmd.send(${JSON.stringify(marker)})`);
      assert.equal(value, tab);
    });
    await step("render-synthetic-answer", async () => {
      const expected = `echo: ${marker} [model=`;
      const { value } = await web(`const entries=window.bridgeCmd.entries(20), expected=${JSON.stringify(expected)};
        return { settled:!entries.settling,
          user:entries.items.some(row=>row.type==='user' && row.text===${JSON.stringify(marker)}),
          assistant:entries.items.some(row=>row.type==='assistant' && row.text.startsWith(expected)),
          rendered:Array.from(document.querySelectorAll('.jsonl-entry-assistant')).some(el=>el.textContent.includes(expected)) }`);
      report.turn = value;
      assert.ok(value.settled && value.user && value.assistant && value.rendered);
    }, true);
    if (fixture) await checkWorkspaceLink({ fixture, installed, ...driver });
    if (behavior === "console-scroll") await checkConsoleScroll({ ...driver, rpc });
    if (behavior === "engine-menus") await checkEngineMenus(driver, tab);
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
  report.runtime = await readRuntimeDiagnostics(capture);
  await writeFile(path.join(output, "report.json"), JSON.stringify(report, null, 2) + "\n");
  console.log(JSON.stringify(report));
}

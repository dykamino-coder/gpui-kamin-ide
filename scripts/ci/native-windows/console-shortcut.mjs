import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { promisify } from "node:util";
import { setTimeout as delay } from "node:timers/promises";
import { byText, clickElement, menuOption } from "./cef-input.mjs";
import { geometry, readLayout, view } from "./console-layout.mjs";

const invoke = promisify(execFile);
const options = { timeout: 10000, windowsHide: true };
const shiftTab = fileURLToPath(new URL("./shift-tab.ps1", import.meta.url));

async function windowState(action) {
  const powershell = path.join(process.env.SystemRoot, "System32", "WindowsPowerShell", "v1.0", "powershell.exe");
  const { stdout } = await invoke(powershell, ["-NoProfile", "-NonInteractive", "-File", shiftTab, "-Action", action], options);
  return JSON.parse(stdout.trim());
}

const terminalState = `const terminal=Array.from(document.querySelectorAll('.bridge-console')).find(el=>el.getClientRects().length);
  const input=terminal?.querySelector('textarea.xterm-helper-textarea');
  return { mounted:!!input, focused:!!input && input===document.activeElement,
    documentFocused:document.hasFocus(), received:window.smokeConsoleKey===true };`;

export async function checkPermissionConsoleShortcut(driver, label, name) {
  const { step, probe, report, web } = driver;
  const terminal = body => probe({ cmd: "wveval", id: view, body, timeoutMs: 1500 });
  const started = Date.now();
  const result = { observations: {} };
  (report.consoleShortcuts ??= {})[name] = result;
  let actionStarted = started;
  async function observe(phase) {
    const [layout, views, chat, focusRing] = await Promise.all([
      readLayout(driver), probe({cmd:"webviews"}),
      web("return document.hasFocus()").then(r => r.value, () => null),
      probe({cmd:"focus"}).then(r => !!r.focused, () => null),
    ]);
    const live = views.views.includes(view);
    const [bounds, dom] = await Promise.all([geometry(driver, layout, actionStarted),
      live ? terminal(terminalState).then(r => r.value, () => null) : null]);
    const sample = { elapsedMs: Date.now()-started, layout, bounds, live, dom, chatDocumentFocused: chat,
      nativeFocusRingControl: focusRing };
    result.observations[phase] = sample;
    if (dom?.focused) result.observations.firstDomFocus ??= sample;
    return sample;
  }
  await step(name + "-mount-state", async () => {
    const s = await observe("beforeCommand");
    result.alreadyMounted = !!s.dom?.mounted;
    if (name === "first-console-command") {
      assert.deepEqual(s.layout.pins, []);
      assert.equal(s.layout.right, false);
      assert.equal(result.alreadyMounted, false);
    } else if (name === "repeat-console-command") {
      assert.ok(result.alreadyMounted && s.layout.slotShown);
    } else {
      assert.deepEqual(s.layout.pins, ["rightTop"]);
      assert.equal(s.layout.right, false);
    }
  }, true);
  await step(name + "-active-window", async () => {
    // AppActivate can report false for a detached updater launch. The HWND
    // is independently verifiable, even when the activation reply is false.
    result.activation ??= await windowState("Activate");
    if (result.activation.after) return;
    await probe({ cmd: "focusWindow" });
    result.afterClientActivation = await windowState("Observe");
    assert.ok(result.afterClientActivation.after);
  }, true);
  await clickElement(driver, name + "-trigger", byText("button.perm-trigger", label), { native: true });
  actionStarted = Date.now();
  await clickElement(driver, name + "-action", menuOption("Change mode in Console"), { native: true });
  let first = true;
  await step(name + "-focused-terminal", async () => {
    const s = await observe(first ? "firstAfterCommand" : "beforeKey");
    first = false;
    if (name === "pinned-console-command") assert.equal(s.layout.active, "rightTop");
    assert.ok(s.bounds.inActivePanel && s.bounds.paintedSinceAction);
    assert.ok(s.dom?.focused && s.dom.documentFocused);
    const { value } = await terminal(`const input=document.activeElement;
      if (!document.hasFocus() || !input?.matches('.bridge-console .xterm-helper-textarea')) return false;
      window.smokeConsoleKey=null;
      function onKey(ev) {
        if (ev.key!=='Tab' || !ev.shiftKey) return;
        window.removeEventListener('keydown',onKey,true);
        window.smokeConsoleKey=ev.target===input;
      }
      window.addEventListener('keydown',onKey,true); return true;`);
    result.focused = value === true;
    assert.ok(result.focused);
    result.observations.beforeKey = s;
  }, true);
  await step(name + "-windows-shift-tab", async () => {
    const receipt = await windowState("Send");
    result.foreground = { before: receipt.before === true, after: receipt.after === true };
    assert.ok(result.foreground.before && result.foreground.after);
  });
  // Keep the first report even if a hidden CEF view disappears during the retry window.
  let attempt = 0;
  await step(name + "-terminal-received-key", async () => {
    const phase = ["afterKey", "settled"][attempt++] ?? "last";
    const s = await observe(phase);
    result.received ||= s.dom?.received === true;
    if (phase === "afterKey") {
      await delay(700);
      const settled = await observe("settled");
      result.received ||= settled.dom?.received === true;
      assert.ok(result.received && settled.dom?.mounted && settled.bounds.inActivePanel);
    } else {
      assert.ok(result.received && s.dom?.mounted && s.bounds.inActivePanel);
    }
  }, true);
}

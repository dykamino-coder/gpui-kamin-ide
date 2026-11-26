import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import { promisify } from "node:util";
import { byText, clickElement } from "./cef-input.mjs";

const invoke = promisify(execFile);
function shell(expression) {
  return invoke("powershell.exe", ["-NoProfile", "-NonInteractive", "-Command",
    "$ErrorActionPreference='Stop'; $w=New-Object -ComObject WScript.Shell; " + expression],
    { timeout: 8000, windowsHide: true });
}

export async function checkPermissionConsoleShortcut(driver, label, name) {
  const { step, probe, report } = driver;
  const terminal = body => probe({ cmd: "wveval", id: "claudeBridgeConsoleView", body, timeoutMs: 6000 });
  const result = {};
  (report.consoleShortcuts ??= {})[name] = result;
  await step(name + "-mount-state", async () => {
    const views = (await probe({cmd:"webviews"})).views;
    const mounted = views.includes("claudeBridgeConsoleView") &&
      (await terminal("return !!document.querySelector('.xterm .xterm-helper-textarea')")).value === true;
    result.alreadyMounted = mounted;
    assert.equal(mounted, name === "repeat-console-command");
  }, true);
  await step(name + "-active-window", () =>
    shell("if (!(Get-Process -Id $env:SMOKE_APP_PID).MainWindowHandle -or !$w.AppActivate([int]$env:SMOKE_APP_PID)) { throw 'window is not active' }"));
  // Route the initiating click through the GPUI window. A key targeted at one
  // CEF view would bypass the shell's selection of the keyboard recipient.
  await clickElement(driver, name + "-trigger", byText("button.perm-trigger", label), { native: true });
  await clickElement(driver, name + "-action", byText(".perm-option-name", "Change mode in Console"), { native: true });
  await step(name + "-focused-terminal", async () => {
    const { value } = await terminal(`const visible=Array.from(document.querySelectorAll('.bridge-console')).find(el=>el.getClientRects().length);
      if (!visible || document.activeElement!==visible.querySelector('textarea.xterm-helper-textarea')) return false;
      window.smokeConsoleKey=null;
      function onKey(ev) {
        if (ev.key!=='Tab' || !ev.shiftKey) return;
        window.removeEventListener('keydown',onKey,true);
        window.smokeConsoleKey=ev.target===visible.querySelector('textarea.xterm-helper-textarea');
      }
      window.addEventListener('keydown',onKey,true);
      return true`);
    result.focused = value === true;
    assert.ok(result.focused);
  }, true);
  await step(name + "-windows-shift-tab", () => shell("$w.SendKeys('+{TAB}')"));
  await step(name + "-terminal-received-key", async () => {
    result.received = (await terminal("return window.smokeConsoleKey")).value === true;
    assert.ok(result.received);
  }, true);
}

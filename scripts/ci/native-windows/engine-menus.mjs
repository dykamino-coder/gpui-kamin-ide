import assert from "node:assert/strict";
import { checkClaudeModeReceipts } from "./claude-mode-receipts.mjs";
import { collapseRight, pinInCollapsedRight } from "./console-layout.mjs";
import { checkPermissionConsoleShortcut } from "./console-shortcut.mjs";
import { byText, clickElement, menuOption } from "./cef-input.mjs";

const trigger = label => byText("button.perm-trigger", label);
const modeNames = { default: "Manual", manual: "Manual", plan: "Plan", acceptEdits: "Accept edits",
  auto: "Auto", dontAsk: "Don’t ask", bypassPermissions: "Bypass permissions" };

export async function checkEngineMenus(driver, tab, receipts = false) {
  const { step, web, report } = driver;
  // Only export synthetic mode information; never export tab objects, IPC replies or credentials.
  async function state() {
    const { value } = await web(`const t=(await window.bridgeCmd.raw('listTabs')).find(t => t.id ===
        window.bridgeCmd.state().activeTabId);
      return {sameTab:t?.id === ${JSON.stringify(tab)}, engine:t?.engine, connected:t?.status === 'connected',
        ready:window.bridgeCmd.state().promptReady, session:t?.sessionId, requested:t?.claudePermissions?.requested,
        model:t?.model, native:t?.claudePermissions?.selected ?? null,
        nativeAvailable:t?.claudePermissions?.available ?? null, codex:t?.codexCollaboration?.selected,
        controls:Array.from(document.querySelectorAll('button.perm-trigger')).filter(el=>el.getClientRects().length).map(el=>el.textContent.trim()),
        options:Array.from(document.querySelectorAll('.perm-menu.visible .perm-option-name'), el=>el.textContent.trim()),
        currentOptions:Array.from(document.querySelectorAll('.perm-menu.visible .perm-option[aria-disabled="true"]'), el=>
          el.querySelector('.perm-option-check i') && el.querySelector('.perm-option-name')?.textContent.trim()).filter(Boolean)}`);
    return value;
  }
  async function inspect(engine, name) {
    let label;
    await step(name + "-engine", async () => {
      const row = await state();
      assert.ok(row.sameTab && row.connected && row.engine === engine);
      const i = row.controls.indexOf("Bridge tools");
      assert.ok(i > 0);
      label = row.controls[i - 1];
      if (engine === "codex") {
        assert.ok(["default", "plan"].includes(row.codex));
        assert.equal(label, row.codex === "plan" ? "Codex · Plan" : "Codex · Default");
      } else if (row.native) {
        assert.equal(label, modeNames[row.native]);
      } else {
        assert.ok(["Claude permissions", ...Object.values(modeNames)].includes(label));
      }
      report.engineMenus.push({ engine, label, source: engine === "codex" ? "codex-collaboration" :
        row.native ? "claude-receipt" : label === "Claude permissions" ? "no-observed-mode" : "claude-history" });
    }, true);
    await clickElement(driver, name + "-engine-open", trigger(label));
    await step(name + "-engine-options", async () => {
      const { options, currentOptions, native, nativeAvailable } = await state();
      if (engine === "claude" && native && !nativeAvailable?.includes(native)) assert.ok(currentOptions.includes(modeNames[native]));
      assert.ok(options.includes(engine === "codex" ? "Default" : "Change mode in Console"));
      assert.ok(!options.includes("Use approval rules"));
      if (engine === "claude") assert.ok(!options.includes("Default"));
    }, true);
    await clickElement(driver, name + "-engine-close", trigger(label));
    await clickElement(driver, name + "-tools-open", trigger("Bridge tools"));
    await step(name + "-tools-options", async () => {
      const { options } = await state();
      assert.ok(options.includes("Use approval rules") && options.includes("Read tools only"));
      assert.ok(!options.includes("Default") && !options.includes("Change mode in Console"));
    }, true);
    await clickElement(driver, name + "-tools-close", trigger("Bridge tools"));
  }
  async function switchModel(name, model) {
    // The model dropdown is right-aligned; the three mode dropdowns are left-aligned.
    const modelButton = "Array.from(document.querySelectorAll('button.perm-trigger')).filter(el=>el.getClientRects().length).at(-1)";
    await clickElement(driver, name + "-model-open", modelButton);
    await clickElement(driver, name + "-choose-model", menuOption(model));
  }
  report.engineMenus = [];
  await inspect("codex", "initial-codex");
  await switchModel("first-opus", "Opus 5.5");
  await inspect("claude", "first-claude");
  if (receipts) await checkClaudeModeReceipts(driver, state, tab);
  await switchModel("return-codex", "GPT Fake");
  await inspect("codex", "returned-codex");
  await switchModel("return-opus", "Opus 5.5");
  await inspect("claude", "returned-claude");
  if (receipts) await checkClaudeModeReceipts(driver, state, tab, true);
  await collapseRight(driver, "first-console");
  for (const name of ["first-console-command", "repeat-console-command", "pinned-console-command"]) {
    if (name === "pinned-console-command") await pinInCollapsedRight(driver);
    const row = await state();
    await checkPermissionConsoleShortcut(driver, row.controls[row.controls.indexOf("Bridge tools") - 1], name);
  }
}

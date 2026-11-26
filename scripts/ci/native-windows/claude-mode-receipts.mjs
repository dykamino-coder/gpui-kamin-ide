// The loopback Bridge starts the checkout-only CLI with the production PTY flags.
// Its real child process reports permission_mode in UserPromptSubmit/Stop hooks.
import assert from "node:assert/strict";
import { byText, clickElement, menuOption } from "./cef-input.mjs";

const names = { bypassPermissions: "Bypass permissions", default: "Manual", plan: "Plan" };
const trigger = label => byText("button.perm-trigger", label);

export async function checkClaudeModeReceipts(driver, state, tab, returned = false) {
  const { web, step, report } = driver;
  report.claudeReceipts ??= [];
  const prefix = returned ? "returned" : "first";
  async function turn(mode) {
    const label = names[mode], marker = "native-mode-" + prefix + "-" + mode;
    await step(marker + "-prompt-ready", async () => {
      const row = await state();
      assert.ok(row.sameTab && row.engine === "claude" && row.connected && row.ready);
    }, true);
    await step(marker + "-submit", async () => {
      const { value } = await web(`return window.bridgeCmd.send(${JSON.stringify("/fake-note " + marker)})`);
      assert.equal(value, tab);
    });
    await step(marker + "-receipt", async () => {
      const row = await state();
      assert.ok(row.sameTab && row.connected && row.engine === "claude" && row.ready);
      assert.equal(row.native, mode);
      assert.ok(row.controls.includes(label));
      const { value } = await web(`return Array.from(document.querySelectorAll('.jsonl-entry-assistant'))
        .some(el=>el.textContent.includes(${JSON.stringify("noted: " + marker)}))`);
      assert.equal(value, true);
    }, true);
    await clickElement(driver, marker + "-open", trigger(label));
    await step(marker + "-checked-item", async () => {
      const row = await state();
      assert.ok(row.currentOptions.includes(label) && row.options.includes("Change mode in Console"));
      assert.ok(!row.options.includes("Use approval rules"));
      report.claudeReceipts.push({ phase: prefix, label, mode, source: "synthetic-cli-turn-hook" });
    }, true);
    await clickElement(driver, marker + "-close", trigger(label));
  }
  async function change(oldMode, mode) {
    const before = await state();
    assert.equal(before.native, oldMode);
    await clickElement(driver, "change-to-" + mode + "-open", trigger(names[oldMode]));
    await clickElement(driver, "change-to-" + mode + "-choose", menuOption(names[mode]));
    await step("change-to-" + mode + "-restarted-before-receipt", async () => {
      const row = await state();
      assert.ok(row.sameTab && row.connected && row.session && row.session !== before.session);
      assert.equal(row.requested, mode);
      assert.equal(row.native, null);
    }, true);
  }
  if (returned) {
    await turn("plan");
    return;
  }
  await turn("bypassPermissions");
  await change("bypassPermissions", "default");
  await turn("default");
  await change("default", "plan");
  await turn("plan");
}

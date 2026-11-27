import assert from "node:assert/strict";
import test from "node:test";
import { consoleLayout, geometry, hasVisiblePaint } from "./console-layout.mjs";

const tool = "claudeBridgeConsole";
const model = slot => ({ [slot]: { pinned: [tool], active: tool } });
const panel = { x: 300, y: 20, w: 420, h: 400, paint_age_ms: 5 };

test("a live webview in a hidden or inactive slot cannot be accepted", () => {
  for (const [slot, flag] of [["main","mainVisible"], ["rightTop","rightPanelVisible"],
    ["rightBottom","rightPanelBottomVisible"], ["centralBottom","filePanelVisible"],
    ["centralBottom","filePanelBottomVisible"], ["sidebar","sidebarVisible"]]) {
    const state = consoleLayout({ [flag]: false, activityModel: model(slot) });
    assert.equal(state.active, slot);
    assert.equal(state.slotShown, false);
  }
  const state = consoleLayout({ activityModel: { rightTop: { pinned: [tool], active: "tree" } } });
  assert.deepEqual(state.pins, ["rightTop"]);
  assert.equal(state.slotShown, false);
  assert.equal(consoleLayout({ activityModel: model("centralBottom") }).slotShown, true);
});

test("geometry requires painted native content overlapping its active panel", () => {
  assert.ok(hasVisiblePaint(panel, panel));
  for (const input of [null, { ...panel, paint_age_ms: null }, { ...panel, x: -500 },
    { ...panel, x: 900 }, { ...panel, w: 0 }, { ...panel, h: 17 }]) assert.equal(hasVisiblePaint(panel, input), false);
});

test("a rectangle painted before the reveal or while the pane is hidden cannot pass", async () => {
  const driver = { probe: async () => ({ bounds: panel }) };
  const state = consoleLayout({ activityModel: model("rightTop") });
  assert.ok((await geometry(driver, state, Date.now()-1000)).paintedSinceAction);
  assert.equal((await geometry(driver, state, Date.now()+100)).paintedSinceAction, false);
  assert.equal((await geometry(driver, { ...state, slotShown: false }, 0)).inActivePanel, false);
});

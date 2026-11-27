// Read only known panel flags from the disposable native client profile.
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import path from "node:path";

const container = "claudeBridgeConsole";
export const view = "claudeBridgeConsoleView";
const slots = ["sidebar", "main", "mainBottom", "centralBottom", "rightTop", "rightBottom"];
const region = { sidebar: "sidebar", main: "main", mainBottom: "main-bottom", centralBottom: "central-bottom", rightTop: "right-top", rightBottom: "right-bottom" };

export function consoleLayout(layout) {
  const model = layout.activityModel ?? {};
  const pins = slots.filter(slot => model[slot]?.pinned?.includes(container));
  const active = pins.find(slot => model[slot]?.active === container) ?? null;
  const right = layout.rightPanelVisible ?? true;
  const shown = { sidebar: layout.sidebarVisible ?? true, main: layout.mainVisible ?? true,
    mainBottom: (layout.mainVisible ?? true) && (layout.mainBottomVisible ?? false),
    centralBottom: (layout.filePanelVisible ?? true) && (layout.filePanelBottomVisible ?? true),
    rightTop: right, rightBottom: right && (layout.rightPanelBottomVisible ?? true) };
  return { pins, active, right, slotShown: !!active && shown[active] };
}

export async function readLayout({ probe, capture }) {
  await probe({ cmd: "flushLayout" });
  return consoleLayout(JSON.parse(await readFile(path.join(capture, "LOCALAPPDATA", "kaminide-gpui-dev", "data", "layout.json"), "utf8")));
}

export async function collapseRight(driver, name) {
  await driver.step(name + "-collapse-right", async () => {
    if ((await readLayout(driver)).right) await driver.probe({ cmd: "emit", kind: "layoutFlag", name: "right" });
  });
  await driver.step(name + "-right-is-collapsed", async () => {
    assert.equal((await readLayout(driver)).right, false);
  }, true);
}

export async function pinInCollapsedRight(driver) {
  await collapseRight(driver, "pinned-console");
  const { pins } = await readLayout(driver);
  for (const slot of pins) await driver.probe({ cmd: "hideView", id: container, slot });
  await driver.probe({ cmd: "showView", id: container, slot: "rightTop" });
  await driver.step("hidden-right-console-pin", async () => {
    const state = await readLayout(driver);
    assert.deepEqual(state.pins, ["rightTop"]);
    assert.equal(state.right, false);
  }, true);
}

export function hasVisiblePaint(panel, consoleBounds) {
  if (!panel || !consoleBounds || !Number.isFinite(consoleBounds.paint_age_ms)) return false;
  const left = Math.max(0, panel.x, consoleBounds.x);
  const top = Math.max(0, panel.y, consoleBounds.y);
  return Math.min(panel.x + panel.w, consoleBounds.x + consoleBounds.w) - left > 20 &&
    Math.min(panel.y + panel.h, consoleBounds.y + consoleBounds.h) - top > 20;
}

export async function geometry(driver, layout, started) {
  const bounds = id => driver.probe({ cmd: "metric", id }).then(r => r.bounds, () => null);
  const [panel, terminal] = await Promise.all([layout.active ? bounds(region[layout.active]) : null, bounds(view)]);
  return { inActivePanel: layout.slotShown && hasVisiblePaint(panel, terminal),
    paintedSinceAction: terminal?.paint_age_ms != null && terminal.paint_age_ms <= Date.now() - started,
    paintAgeMs: terminal?.paint_age_ms ?? null };
}

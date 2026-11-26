// Inputs are dispatched by the CEF host at coordinates hit-tested in the application webview.
import assert from "node:assert/strict";

export async function clickElement({ step, web, probe }, name, expression) {
  let point;
  await step(name + "-hit-test", async () => {
    const { value } = await web(`const el = (${expression});
      if (!el || el.disabled || el.closest('[aria-disabled="true"]')) return null;
      el.scrollIntoView({block:'nearest',inline:'nearest'});
      const b = el.getBoundingClientRect(), x = b.left + b.width / 2, y = b.top + b.height / 2;
      return b.width > 0 && b.height > 0 && el.contains(document.elementFromPoint(x,y)) ? {x,y} : null`);
    assert.ok(value && Number.isFinite(value.x) && Number.isFinite(value.y));
    point = value;
  }, true);
  await step(name + "-click", async () => {
    for (const action of ["move", "down", "up"]) {
      const reply = await probe({ cmd: "webmouse", id: "claudeBridgeChat", action, ...point });
      assert.equal(reply.delivery, "cef-host");
    }
  });
}

export const byText = (selector, label) =>
  `Array.from(document.querySelectorAll(${JSON.stringify(selector)})).find(el => el.textContent.trim() === ${JSON.stringify(label)})`;

/** Production first-run modal. The disposable credential stays inside the renderer and Extension Host. */
export async function registerClient(driver, origin) {
  const { step, web } = driver;
  await step("set-loopback-server", async () => {
    const { value } = await web(`const field = document.querySelector('input[placeholder^="localhost:3456"]');
      if (!field) return false;
      field.value = ${JSON.stringify(origin)};
      field.dispatchEvent(new Event('input',{bubbles:true}));
      return true`);
    assert.equal(value, true);
  }, true);
  await clickElement(driver, "create-client-token", byText("button", "Create token & continue"));
  await step("first-run-setup-complete", async () => {
    const { value } = await web(`const cfg = await window.bridgeCmd.raw('getConfig');
      return !document.querySelector('input[placeholder^="localhost:3456"]')
        && !!cfg.token && cfg.serverUrl === ${JSON.stringify(origin.replace(/^http/, "ws"))}`);
    assert.equal(value, true);
  }, true);
}

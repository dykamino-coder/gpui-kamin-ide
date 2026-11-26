import assert from "node:assert/strict";
import { mkdir, writeFile } from "node:fs/promises";
import path from "node:path";
import { pathToFileURL } from "node:url";

export async function createWorkspaceFixture(capture) {
  const directory = path.join(capture, "fixture-project");
  await mkdir(directory);
  const file = path.join(directory, "sample file.ts");
  await writeFile(file, "// first\n// second\n// third\nconst fourth = 4;\n");
  return { directory, file };
}

export async function checkWorkspaceLink({ fixture, step, probe, web, report }) {
  const markdown = `[Open fixture](${pathToFileURL(fixture.file).href}#L4C2)`;
  const observed = () => probe({ cmd: "nativeAcceptance", action: "editorLocation" });
  const sameFile = actual => typeof actual === "string" &&
    path.toNamespacedPath(actual).toLowerCase() === path.toNamespacedPath(fixture.file).toLowerCase();

  await step("workspace-link-before-click", async () => {
    assert.equal(sameFile((await observed()).location?.path), false);
    assert.equal(typeof (await web(`return window.bridgeCmd.send(${JSON.stringify(markdown)})`)).value, "string");
  });
  let point;
  await step("assistant-link-hit-test", async () => {
    // Use the hyperlink in the echoed assistant answer, never the user's message.
    const { value } = await web(`const link = Array.from(document.querySelectorAll(
        '.jsonl-entry-assistant a[data-workspace-location]')).find(a => a.textContent === 'Open fixture');
      if (!link) return null;
      link.scrollIntoView({block:'center',inline:'nearest'});
      const b = link.getBoundingClientRect(), x = b.left + b.width/2, y = b.top + b.height/2;
      return link.contains(document.elementFromPoint(x,y)) ? {x,y} : null`);
    assert.ok(value && Number.isFinite(value.x) && Number.isFinite(value.y));
    point = value;
  }, true);
  await step("cef-mouse-click", async () => {
    for (const action of ["move", "down", "up"]) {
      const reply = await probe({ cmd: "webmouse", id: "claudeBridgeChat", action, ...point });
      assert.equal(reply.delivery, "cef-host");
    }
  });
  await step("native-editor-file-and-caret", async () => {
    const { location } = await observed();
    assert.ok(sameFile(location?.path));
    assert.equal(location.line, 4);
    assert.equal(location.column, 2);
    report.editor = { fixture: "sample file.ts", line: location.line, column: location.column };
  }, true);
}

import assert from "node:assert/strict";
import { mkdir, writeFile } from "node:fs/promises";
import path from "node:path";
import { pathToFileURL } from "node:url";
import { clickElement } from "./cef-input.mjs";

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
  await clickElement({ step, web, probe }, "assistant-workspace-link",
    `Array.from(document.querySelectorAll('.jsonl-entry-assistant a[data-workspace-location]'))
      .find(a => a.textContent === 'Open fixture')`);
  await step("native-editor-file-and-caret", async () => {
    const { location } = await observed();
    assert.ok(sameFile(location?.path));
    assert.equal(location.line, 4);
    assert.equal(location.column, 2);
    report.editor = { fixture: "sample file.ts", line: location.line, column: location.column };
  }, true);
}

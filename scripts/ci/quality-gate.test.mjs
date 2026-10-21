import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { spawnSync } from "node:child_process";
import test from "node:test";

// Execute the real aggregate step, so a declared-but-unused result cannot
// silently bypass the gate again when another job is added or split.
const workflow = readFileSync(".github/workflows/pr-checks.yml", "utf8");
const gate = workflow.slice(workflow.indexOf("\n  quality-gate:"));
const needs = [...gate.matchAll(/^      - ([\w-]+)$/gm)].map((m) => m[1]);
const bindings = [...gate.matchAll(/^          (\w+): \$\{\{ needs\.([\w-]+)\.result \}\}$/gm)];
const script = gate.split("        run: |\n")[1].replace(/^          /gm, "");
const successful = Object.fromEntries(bindings.map((m) => [m[1], "success"]));
const run = (overrides = {}) => spawnSync("bash", ["-c", script], {
  encoding: "utf8", env: { ...process.env, ...successful, ...overrides },
});

test("aggregate observes every dependency and runs even after failure/cancellation", () => {
  assert.ok(needs.includes("windows-installer"));
  assert.ok(needs.includes("vendor-taffy"));
  assert.deepEqual(bindings.map((m) => m[2]).sort(), needs.toSorted());
  assert.match(gate, /if: \$\{\{ always\(\) \}\}/);
  assert.equal(run().status, 0);
  assert.equal(run(Object.fromEntries(bindings.map((m) => [m[1], "skipped"]))).status, 0);
});

for (const [, variable, job] of bindings) {
  test(`${job} cannot bypass the actual quality gate`, () => {
    for (const result of ["failure", "cancelled", "", "unknown"]) {
      const outcome = run({ [variable]: result });
      assert.equal(outcome.status, 1, `${job}=${result}: ${outcome.stderr}`);
      assert.match(outcome.stderr, /Applicable check did not pass/);
    }
  });
}

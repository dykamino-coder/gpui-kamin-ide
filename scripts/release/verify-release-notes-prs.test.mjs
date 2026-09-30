import assert from "node:assert/strict";
import test from "node:test";
import { validateReleaseNotePull, validateRuntimeCloseout } from "./verify-release-notes-prs.mjs";

const pull = {
  number: 117,
  merged_at: "2026-09-19T08:00:00Z",
  base: { ref: "main" },
  merge_commit_sha: "a".repeat(40),
};

test("accepts a merged implementation PR", () => {
  assert.equal(validateReleaseNotePull(pull, ["crates/installer/src/steps.rs"]),
    "a".repeat(40));
});

test("rejects a diagnostic-only PR", () => {
  assert.throws(() => validateReleaseNotePull(pull, [
    "extensions/claude-bridge/runtime-issues/INC-2026-0052.md",
  ]), /only registered or documented/);
});

test("rejects an unmerged PR", () => {
  assert.throws(() => validateReleaseNotePull({ ...pull, merged_at: null },
    ["crates/installer/src/steps.rs"]), /not a merged/);
});

test("rejects version-bump and documentation PR titles", () => {
  for (const title of ["chore(release): bump versions", "docs(runtime): record bug"]) {
    assert.throws(() => validateReleaseNotePull({ ...pull, title },
      ["src/runtime.ts"]), /documentation or a version bump/);
  }
});

const register = `| ID | State | Result | Track | Prerequisite | Next artifact |
| --- | --- | --- | --- | --- | --- |
| [BR-31](RUNTIME_RELIABILITY.md#br-31) | ready | change | delivery | none | Wake PR |
| [BR-25](RUNTIME_RELIABILITY.md#br-25) | waiting | verify | agents | BR-31 | Windows gate |
`;
const runtimePull = {
  number: 147,
  body: "## Задача\nBR-31 implements pump wake; BR-25 depends on it.\n\n## Limitations\nWindows CEF gate remains.\n",
};

test("rejects a release when the BR row ignores its merged implementation", () => {
  assert.throws(() => validateRuntimeCloseout(runtimePull, register),
    /BR-31.*does not acknowledge this merge/);
});

test("allows either verification or a bounded next change after recording the merged PR", () => {
  const reconciled = register.replace("| Wake PR |", "| PR #147 merged; Windows R6 gate remains |");
  assert.doesNotThrow(() => validateRuntimeCloseout(runtimePull, reconciled));
  assert.doesNotThrow(() => validateRuntimeCloseout(runtimePull,
    reconciled.replace("| ready | change | delivery", "| ready | verify | delivery")));
});

test("does not treat an incidental BR mention outside the task section as an implementation", () => {
  assert.doesNotThrow(() => validateRuntimeCloseout({
    ...runtimePull,
    body: "## Task\nFix INC-2026-0055.\n\n## Limitations\nBR-31 still needs a Windows gate.\n",
  }, register));
});

test("rejects a task missing from the shared BR register", () => {
  assert.throws(() => validateRuntimeCloseout(runtimePull, ""), /no runtime register row/);
});

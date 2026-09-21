import assert from "node:assert/strict";
import test from "node:test";
import { validateReleaseNotePull } from "./verify-release-notes-prs.mjs";

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

import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";
import { deliveryRecords, verifyDelivery } from "./runtime-delivery.mjs";

const sha = "a".repeat(40);
const line = (commit = sha) => `- **Published fix:** \`kaminide-v1.0.65\`; commit \`${commit}\`; [#184](https://github.com/example/project/pull/184).`;

test("delivery records are explicit, strict and separate from acceptance", () => {
  assert.deepEqual(deliveryRecords("- **Status:** reported\nNo implementation yet."), []);
  assert.deepEqual(deliveryRecords(`- **Status:** confirmed; field observation pending\n${line()}`), [
    { tag: "kaminide-v1.0.65", commit: sha, pr: 184, repository: "example/project" },
  ]);
  for (const bad of [line("a".repeat(7)), line().replace("/184)", "/185)"), "- **Published fix:** TODO"]) {
    assert.throws(() => deliveryRecords(bad), /Invalid Published fix/);
  }
  assert.throws(() => deliveryRecords(`- **Status:** resolved; not yet released\n${line()}`), /contradicts/);
});

test("a real release must contain the implementation, not an earlier or divergent tree", () => {
  const repo = mkdtempSync(join(tmpdir(), "runtime-delivery-"));
  const git = (...args) => execFileSync("git", ["-C", repo, ...args], { encoding: "utf8" }).trim();
  try {
    git("init", "--quiet");
    git("config", "user.name", "Test");
    git("config", "user.email", "test@example.invalid");
    git("commit", "--allow-empty", "-qm", "baseline");
    git("tag", "kaminide-v1.0.64");
    git("commit", "--allow-empty", "-qm", "fix (#184)");
    const commit = git("rev-parse", "HEAD");
    git("tag", "kaminide-v1.0.65");
    const record = { tag: "kaminide-v1.0.65", commit, pr: 184 };
    const release = { tag_name: record.tag, draft: false, published_at: "2026-10-12T00:00:00Z" };
    assert.doesNotThrow(() => verifyDelivery(record, { repo, release }));
    assert.throws(() => verifyDelivery({ ...record, tag: "kaminide-v1.0.64" }, {
      repo, release: { ...release, tag_name: "kaminide-v1.0.64" },
    }), /does not contain/);
    git("checkout", "--quiet", "--orphan", "divergent");
    git("commit", "--allow-empty", "-qm", "different root");
    assert.throws(() => verifyDelivery({ ...record, commit: git("rev-parse", "HEAD") }, { repo, release }), /does not contain/);
    for (const invalid of [{ ...release, draft: true }, { ...release, published_at: null }, { ...release, tag_name: "other" }]) {
      assert.throws(() => verifyDelivery(record, { repo, release: invalid }), /not a published/);
    }
    assert.throws(() => verifyDelivery({ ...record, commit: sha }, { repo, release }), /does not contain/);
  } finally {
    rmSync(repo, { recursive: true, force: true });
  }
});

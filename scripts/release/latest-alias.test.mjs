import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import test from "node:test";

const script = fileURLToPath(new URL("./latest-alias.mjs", import.meta.url));

test("a later docs merge does not prevent the newest release from advancing latest", () => {
  const repo = mkdtempSync(join(tmpdir(), "kaminide-latest-alias-"));
  const git = (...args) => execFileSync("git", args, { cwd: repo, encoding: "utf8" }).trim();
  const decide = (release, alias) => execFileSync(
    process.execPath, [script, release, alias],
    { cwd: repo, encoding: "utf8", stdio: ["ignore", "pipe", "pipe"] },
  ).trim();

  try {
    git("init", "-q");
    git("config", "user.name", "Release Test");
    git("config", "user.email", "release-test@example.invalid");
    git("commit", "-q", "--allow-empty", "-m", "previous release");
    const previousRelease = git("rev-parse", "HEAD");
    git("commit", "-q", "--allow-empty", "-m", "new release");
    const newRelease = git("rev-parse", "HEAD");
    git("commit", "-q", "--allow-empty", "-m", "docs after release");

    assert.equal(decide(newRelease, previousRelease), "true");
    assert.equal(decide(newRelease, ""), "true");
    assert.equal(decide(previousRelease, newRelease), "false");

    git("checkout", "-q", "-b", "divergent", previousRelease);
    git("commit", "-q", "--allow-empty", "-m", "unrelated release");
    const divergentRelease = git("rev-parse", "HEAD");
    assert.throws(() => decide(newRelease, divergentRelease), (error) =>
      /divergent histories/.test(error.stderr));
  } finally {
    rmSync(repo, { recursive: true, force: true });
  }
});

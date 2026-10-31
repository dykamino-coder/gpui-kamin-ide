import assert from "node:assert/strict";
import { mkdirSync, mkdtempSync, readFileSync, rmSync, rmdirSync, symlinkSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { spawnSync } from "node:child_process";
import test from "node:test";
import { validatePrMetadata } from "./pr-metadata.mjs";
import { taskCard, validateRuntimeAcceptance } from "./runtime-acceptance.mjs";

const read = (name) => readFileSync(`.github/workflows/${name}.yml`, "utf8");
const quality = read("pr-checks");
const metadata = read("pr-metadata");
const cache = read("cargo-cache");
const triggers = (text) => text.split("\non:\n")[1].split(/\n\S/)[0];

test("only metadata reruns on edits, including the body-dependent gates", () => {
  assert.doesNotMatch(triggers(quality), /\bedited\b/);
  assert.match(triggers(metadata), /opened, synchronize, reopened, edited, ready_for_review/);
  for (const script of ["runtime-acceptance.mjs", "release-notes.mjs", "verify-release-notes-prs.mjs"]) {
    assert.match(metadata, new RegExp(script.replaceAll(".", "\\.")));
    assert.ok(!quality.includes(script));
  }
  assert.match(metadata, /export RELEASE_CANDIDATE=.*jq -r '\.release'/);
  assert.match(metadata, /pull-requests: read/);
  assert.match(metadata, /--previous-tag/);
  assert.match(quality, /installer_candidate="\$release_candidate"/);
  assert.doesNotMatch(quality, /pull_request\.body/);
});

test("green metadata cannot survive deleting required runtime evidence", async () => {
  const headSha = "a".repeat(40);
  const card = await taskCard("BR-31");
  const body = `## Task
BR-31: verify delivery.
## Summary
Wake the delivery pump.
## Validation
Automated tests and the recorded Windows scenario passed.
## Risks and limitations
Acceptance covers the recorded delivery scenario only.
## Versioning
Release versions: unchanged
## Runtime acceptance
- Task: BR-31
- Windows runtime merge gate: passed
- Candidate: ${headSha}
- Scenario: Reveal Agents view without pointer input after renderer reap.
- Observed: Rows appeared within one second without a forced repaint.
- Evidence: https://github.com/example/evidence/tree/${headSha}/br-31
`;
  const validate = (body) => {
    validatePrMetadata({ title: "fix(shell): wake delivery", body });
    validateRuntimeAcceptance({ body, files: ["crates/shell/src/web/mod.rs"], headSha, card });
  };
  validate(body);
  assert.throws(() => validate(body.replace(/^- Evidence:.*$/m, "")), /task-specific runtime evidence/);
  assert.throws(() => validate(body.split("## Runtime acceptance")[0]), /requires ## Runtime acceptance/);
});

test("release intent stays source-classified and removing release notes fails metadata", () => {
  const input = { title: "chore(release): prepare release", release: true,
    repository: "example/repo" };
  const body = `## Task
Publish the coordinated version change.
## Release notes
### Shipped changes
- Ship the accepted change ([#12](https://github.com/example/repo/pull/12)).
### Verification and limitations
- Automated checks passed; acceptance is limited to the stated scenario.
`;
  validatePrMetadata({ ...input, body });
  assert.throws(() => validatePrMetadata({ ...input, body: "## Task\nPublish the coordinated version change." }), /Release notes/);
  assert.throws(() => validatePrMetadata({ ...input,
    body: body.replace("- Automated checks passed; acceptance is limited to the stated scenario.", "") }), /verification bullets/);
});

test("only the independent main push workflow saves Cargo caches", () => {
  assert.doesNotMatch(quality, /actions\/cache\/save@/);
  assert.doesNotMatch(triggers(cache), /pull_request|workflow_dispatch|workflow_run/);
  assert.match(triggers(cache), /push:\n    branches: \[main\]/);
  assert.equal((cache.match(/if: github.event_name == 'push' && github.ref == 'refs\/heads\/main'/g) ?? []).length, 2);
  for (const namespace of ["windows-cargo-v2", "windows-vendor-gpui-v1"]) {
    const key = `${namespace}-\${{ hashFiles('Cargo.lock') }}-\${{ github.sha }}`;
    assert.ok(quality.includes(key));
    assert.ok(cache.includes(key));
  }
  assert.match(read("docker"), /workflows: \["pull request checks"\]/);
  const baseline = read("main-baseline");
  assert.match(baseline, /key: windows-cargo-inputs-v1-/);
  assert.doesNotMatch(baseline, /actions\/cache\/save@|^            target$/m);
});

test("cache scope avoids docs and warms only affected products", () => {
  const run = cache.split("        run: |\n")[1].split("\n  workspace:")[0]
    .replace(/^          /gm, "");
  // Run the real scope script; stub only git's external diff and output location.
  const directory = mkdtempSync(join(tmpdir(), "kamin-cache-scope-"));
  const output = join(directory, "output.txt");
  try {
    for (const [path, workspace, standalone] of [
      ["docs/README.md", false, false],
      ["crates/html/src/lib.rs", true, false],
      ["vendor/gpui/src/lib.rs", true, true],
      ["Cargo.toml", true, true],
      [".github/workflows/pr-checks.yml", true, true],
    ]) {
      writeFileSync(output, "");
      const script = `git() { printf '%s\\n' "$CHANGED_PATH"; }
${run}
`;
      const result = spawnSync("bash", ["-c", script], { encoding: "utf8",
        env: { ...process.env, PUSH_BEFORE: "a".repeat(40), RUNNER_TEMP: directory.replaceAll("\\", "/"),
          GITHUB_OUTPUT: output.replaceAll("\\", "/"), CHANGED_PATH: path } });
      assert.equal(result.status, 0, result.stderr);
      const flags = readFileSync(output, "utf8");
      assert.match(flags, new RegExp(`workspace=${workspace}`), path);
      assert.match(flags, new RegExp(`standalone=${standalone}`), path);
    }
  } finally {
    rmSync(join(directory, "cache-files.txt"), { force: true });
    rmSync(output, { force: true });
    rmdirSync(directory);
  }
});

test("byte summary measures the archive inputs without traversing junctions", () => {
  const directory = mkdtempSync(join(tmpdir(), "kamin-cache-bytes-"));
  const cargo = join(directory, ".cargo");
  const registry = join(cargo, "registry");
  const target = join(directory, "target");
  const summary = join(directory, "summary.md");
  try {
    mkdirSync(registry, { recursive: true });
    mkdirSync(target);
    writeFileSync(join(registry, "source"), "abc");
    writeFileSync(join(target, "compiled"), "1234567");
    symlinkSync(registry, join(target, "link"), "junction");
    const result = spawnSync(process.execPath, ["scripts/ci/cargo-cache-size.mjs", target], {
      encoding: "utf8", env: { ...process.env, USERPROFILE: directory, HOME: directory,
        GITHUB_STEP_SUMMARY: summary },
    });
    assert.equal(result.status, 0, result.stderr);
    const report = readFileSync(summary, "utf8");
    assert.match(report, /registry: 3 bytes/);
    assert.match(report, /git: 0 bytes/);
    assert.match(report, /target: 7 bytes/);
    assert.equal(readFileSync(join(registry, "source"), "utf8"), "abc");
  } finally {
    for (const path of [join(target, "link"), join(target, "compiled"), join(registry, "source"), summary]) {
      rmSync(path, { force: true });
    }
    for (const path of [target, registry, cargo, directory]) rmdirSync(path);
  }
});

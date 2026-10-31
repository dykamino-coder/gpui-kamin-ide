import assert from "node:assert/strict";
import { readFile, mkdtemp, writeFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { execFileSync, spawnSync } from "node:child_process";
import test from "node:test";
import { validatePrMetadata } from "./pr-metadata.mjs";
import { taskCard, validateRuntimeAcceptance } from "./runtime-acceptance.mjs";

const title = "ci(pr): validate pull request descriptions";
const body = `## Summary
Catch missing risk and verification information before merge.
## Validation
- Node contract tests passed; no Windows GUI gate applies to this tooling.
## Risks and limitations
CI checks structure, not whether the reported tests actually ran.
`;
const validate = (overrides = {}) => validatePrMetadata({ title, body, ...overrides });

test("accepts a filled compact change or diagnostic description", () => {
  assert.doesNotThrow(() => validate());
  assert.doesNotThrow(() => validate({ body: `${body}## Versioning\n- Release versions: unchanged.\n` }));
  assert.doesNotThrow(() => validate({ title: "docs(runtime): clarify research inputs" }));
  assert.doesNotThrow(() => validate({ title: "tools(release)!: change the notes CLI" }));
});

test("rejects Russian titles, missing prefixes and multiline subjects", () => {
  for (const bad of ["fix(chat): исправить ссылку", "fix(чат): repair links", "Repair links", "fix: repair\nlinks", "fix:"]) {
    assert.throws(() => validate({ title: bad }), /Conventional Commits/);
  }
  assert.doesNotThrow(() => validate({ title: "fix(models): map Opus 5 → Opus 5.5" }));
});

test("accepts the team's working language in the body", () => {
  const translated = body.replace("Summary", "Что изменено").replace("Validation", "Проверка")
    .replace("Risks and limitations", "Риски и ограничения")
    .replace("Node contract tests passed; no Windows GUI gate applies to this tooling.",
      "N/A: Windows GUI не затронут; автоматические проверки прошли.");
  assert.doesNotThrow(() => validate({ body: translated.replaceAll("\n", "\r\n") }));
  assert.doesNotThrow(() => validate({ body: translated.replace("Риски и ограничения", "Ограничения") }));
});

test("rejects absent sections, HTML comments, unchecked boxes and bare placeholders", () => {
  for (const heading of ["Summary", "Validation", "Risks and limitations"]) {
    assert.throws(() => validate({ body: body.replace(`## ${heading}`, "## Other") }), /Fill ##/);
  }
  for (const placeholder of ["<!-- measured risks go here -->", "- [ ] Record risks", "N/A", "TODO", "- TBD", "<explanation>"]) {
    assert.throws(() => validate({ body: body.replace("CI checks structure, not whether the reported tests actually ran.", placeholder) }), /Fill ## Risks/);
  }
});

test("headings inside example fences cannot stand in for a real description", () => {
  assert.throws(() => validate({ body: `\`\`\`markdown\n${body}\`\`\`` }), /Fill ## Summary/);
  assert.throws(() => validate({ body: `~~~~markdown\n${body}~~~~` }), /Fill ## Summary/);
  assert.throws(() => validate({ body: `\`\`\`markdown\n\`\`\`not-a-closing-fence\n${body}\`\`\`` }), /Fill ## Summary/);
});

test("a short honest body passes without a manual version declaration", () => {
  const short = `## Summary
Fix the hover gap.
## Validation
- cargo test -p kamin-shell hover: passed.
## Risks and limitations
No live Windows run; the card needs no runtime gate.
`;
  assert.doesNotThrow(() => validate({ body: short }));
  assert.throws(() => validate({ body: short.replace("## Validation\n- cargo test -p kamin-shell hover: passed.\n", "") }), /Fill ## Validation/);
  assert.throws(() => validate({ body: short.replace("cargo test -p kamin-shell hover: passed.", "TODO") }), /Fill ## Validation/);
});

test("a diff-derived version change needs release metadata, not a change body", () => {
  // RELEASE_CANDIDATE=true comes from the coordinated version diff in CI.
  const changeBody = `${body}## Versioning\n- Release versions: unchanged.\n`;
  assert.throws(() => validate({ release: true, repository: "example/repo" }), /Release notes/i);
  assert.throws(() => validate({ body: changeBody, release: true, repository: "example/repo" }), /Release notes/i);
});

test("Dependabot formatting exemption needs the actual bot identity and a body", () => {
  const generated = "Bumps an official GitHub Action and its full SHA pin.";
  assert.doesNotThrow(() => validate({ body: generated, author: { login: "dependabot[bot]", type: "Bot" } }));
  for (const author of [{ login: "dependabot[bot]", type: "User" }, { login: "someone", type: "Bot" }]) {
    assert.throws(() => validate({ body: generated, author }), /Fill ## Summary/);
  }
  assert.throws(() => validate({ body: "", author: { login: "dependabot[bot]", type: "Bot" } }), /filled in/);
  assert.throws(() => validate({ body: "<!-- generated body -->", author: { login: "dependabot[bot]", type: "Bot" } }), /filled in/);
});

test("release PRs keep the existing notes parser instead of the change template", () => {
  const releaseBody = `## Task
Owner-authorized batch release.
## Release notes
### Shipped changes
- Repair links ([#1](https://github.com/example/repo/pull/1)).
### Verification and limitations
- Unit tests passed; manual acceptance is recorded in the implementation PR.
`;
  assert.doesNotThrow(() => validate({ body: releaseBody, release: true, repository: "example/repo" }));
  assert.throws(() => validate({ body: releaseBody.replace("### Verification and limitations", "### Other"), release: true, repository: "example/repo" }), /sections must be ordered/);
  assert.throws(() => validate({ body: releaseBody }), /Fill ## Summary/);
});

test("the blank repository template cannot pass without an author's results", async () => {
  const template = await readFile(".github/pull_request_template.md", "utf8");
  assert.throws(() => validate({ body: template }), /Fill ## Summary/);
});

test("CLI accepts both GitHub event and REST PR JSON and returns failures", async () => {
  const dir = await mkdtemp(join(tmpdir(), "pr-metadata-"));
  try {
    const file = join(dir, "event.json");
    for (const payload of [{ pull_request: { title, body } }, { title, body }]) {
      await writeFile(file, JSON.stringify(payload));
      const output = execFileSync(process.execPath, ["scripts/ci/pr-metadata.mjs", "--event", file], { encoding: "utf8", env: { ...process.env, RELEASE_CANDIDATE: "false" } });
      assert.match(output, /format passed/);
    }
    await writeFile(file, JSON.stringify({ title, body: "" }));
    const result = spawnSync(process.execPath, ["scripts/ci/pr-metadata.mjs", "--event", file], { encoding: "utf8", env: { ...process.env, RELEASE_CANDIDATE: "false" } });
    assert.equal(result.status, 1);
    assert.match(result.stderr, /must be filled in/);
    await writeFile(file, JSON.stringify({ title, body }));
    const { RELEASE_CANDIDATE: _ignored, ...withoutClass } = process.env;
    const unclassified = spawnSync(process.execPath, ["scripts/ci/pr-metadata.mjs", "--event", file], { encoding: "utf8", env: { ...withoutClass, GITHUB_ACTIONS: "true" } });
    assert.equal(unclassified.status, 1);
    assert.match(unclassified.stderr, /derived from the diff/);
  } finally {
    await rm(dir, { recursive: true, force: true });
  }
});

test("removing required runtime acceptance evidence still fails with a short body", async () => {
  const headSha = "b".repeat(40);
  const card = await taskCard("BR-31");
  const runtimeBody = `## Task
BR-31
${body}## Runtime acceptance
- Task: BR-31
- Windows runtime merge gate: passed
- Candidate: ${headSha}
- Scenario: Reveal Agents view without pointer input after renderer reap.
- Observed: Rows appeared within one second without a forced repaint.
- Evidence: https://github.com/example/evidence/tree/${headSha}/br-31
`;
  const check = (text) => {
    validate({ title: "fix(shell): wake delivery", body: text });
    validateRuntimeAcceptance({ body: text, title: "fix(shell): wake delivery", files: ["crates/shell/src/web/mod.rs"], headSha, card });
  };
  assert.doesNotThrow(() => check(runtimeBody));
  assert.throws(() => check(runtimeBody.split("## Runtime acceptance")[0]), /requires ## Runtime acceptance/);
  assert.throws(() => check(runtimeBody.replace(/^- Evidence:.*$/m, "")), /task-specific runtime evidence/);
});

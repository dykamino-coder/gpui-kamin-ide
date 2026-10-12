import assert from "node:assert/strict";
import { readFile, mkdtemp, writeFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { execFileSync, spawnSync } from "node:child_process";
import test from "node:test";
import { validatePrMetadata } from "./pr-metadata.mjs";

const title = "ci(pr): validate pull request descriptions";
const body = `## Summary
Catch missing risk and verification information before merge.
## Validation
- Node contract tests passed; no Windows GUI gate applies to this tooling.
## Risks and limitations
CI checks structure, not whether the reported tests actually ran.
## Versioning
- Release versions: unchanged.
`;
const validate = (overrides = {}) => validatePrMetadata({ title, body, ...overrides });

test("accepts a filled compact change or diagnostic description", () => {
  assert.doesNotThrow(() => validate());
  assert.doesNotThrow(() => validate({ title: "docs(runtime): clarify research inputs" }));
  assert.doesNotThrow(() => validate({ title: "tools(release)!: change the notes CLI" }));
});

test("rejects Russian titles, missing prefixes and multiline subjects", () => {
  for (const bad of ["fix(chat): исправить ссылку", "fix(чат): repair links", "Repair links", "fix: repair\nlinks", "fix:"]) {
    assert.throws(() => validate({ title: bad }), /Conventional Commits/);
  }
  assert.doesNotThrow(() => validate({ title: "fix(models): map Opus 5 → Opus 5.5" }));
});

test("retains existing Russian heading aliases and N/A with a reason", () => {
  const translated = body.replace("Summary", "Что изменено").replace("Validation", "Проверка")
    .replace("Risks and limitations", "Ограничения").replace("Versioning", "Версионирование и публикация")
    .replace("Node contract tests passed; no Windows GUI gate applies to this tooling.",
      "N/A: Windows GUI не затронут; автоматические проверки прошли.");
  assert.doesNotThrow(() => validate({ body: translated.replaceAll("\n", "\r\n") }));
});

test("rejects absent sections, HTML comments, unchecked boxes and bare placeholders", () => {
  for (const heading of ["Summary", "Validation", "Risks and limitations", "Versioning"]) {
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

test("requires the explicit unchanged-version declaration on ordinary PRs", () => {
  assert.throws(() => validate({ body: body.replace("Release versions: unchanged.", "No release was published.") }), /must declare/);
  assert.throws(() => validate({ body: body.replace("- Release versions:", "- [ ] Release versions:") }), /Fill ## Versioning/);
  assert.doesNotThrow(() => validate({ body: body.replace("- Release versions:", "- [x] Release versions:") }));
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
  } finally {
    await rm(dir, { recursive: true, force: true });
  }
});

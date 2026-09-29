import assert from "node:assert/strict";
import test from "node:test";
import { parseReleaseNotes } from "./release-notes.mjs";

const repository = "dykamino-coder/gpui-kamin-ide";
const body = `## Result
Publish 1.0.57.

## Release notes
### Shipped changes
- Installer shortcuts now start from the installation directory ([#117](https://github.com/${repository}/pull/117)).

### Verification and limitations
- Windows CI passed; manual Windows smoke testing was unavailable.

## Scope
Version bump only.
`;

test("publishes only curated notes and returns implementation PRs", () => {
  const result = parseReleaseNotes(body, repository);
  assert.match(result.markdown, /Installer shortcuts/);
  assert.doesNotMatch(result.markdown, /Publish 1\.0\.57|Version bump only/);
  assert.deepEqual(result.pullNumbers, [117]);
  assert.match(result.sha256, /^[a-f0-9]{64}$/);
});

test("rejects missing notes and unchanged template", () => {
  assert.throws(() => parseReleaseNotes("## Result\nNone", repository), /Release notes/);
  assert.throws(
    () => parseReleaseNotes(body.replace("- Installer shortcuts", "<!-- TODO -->\n- Installer shortcuts"), repository),
    /template comments/,
  );
});

test("rejects claimed changes without an implementation PR", () => {
  assert.throws(
    () => parseReleaseNotes(body.replace(/ \(\[#117\].*?\)/, ""), repository),
    /implementation PR/,
  );
});

test("rejects an empty verification section", () => {
  assert.throws(
    () => parseReleaseNotes(body.replace("- Windows CI passed; manual Windows smoke testing was unavailable.", ""), repository),
    /verification bullets/,
  );
});

test("keeps open diagnostic tasks separate from shipped implementation PRs", () => {
  const withKnownIssue = body.replace("### Verification and limitations", `### Upgrade notes
- Existing installations need no migration.

### Known issues
- After RDP reconnect, the app may stop responding; no workaround is confirmed ([task](https://github.com/${repository}/pull/113)).

### Verification and limitations`);
  const result = parseReleaseNotes(withKnownIssue, repository);
  assert.match(result.markdown, /### Known issues/);
  assert.deepEqual(result.pullNumbers, [117]);
});

test("rejects unlinked or empty known issues", () => {
  const withKnownIssue = body.replace("### Verification and limitations", `### Known issues
- RDP reconnect may hang the app.

### Verification and limitations`);
  assert.throws(() => parseReleaseNotes(withKnownIssue, repository), /public task/);
  assert.throws(() => parseReleaseNotes(withKnownIssue.replace("- RDP reconnect may hang the app.", ""), repository), /public task/);
});

test("rejects unknown, duplicate, or misordered release note sections", () => {
  const extra = "### Known issues\n- Tracked ([task](https://github.com/" + repository + "/pull/113)).\n\n";
  const withKnownIssue = body.replace("### Verification and limitations", extra + "### Verification and limitations");
  assert.throws(() => parseReleaseNotes(withKnownIssue.replace("### Known issues", "### Fixed"), repository), /sections must be ordered/);
  assert.throws(() => parseReleaseNotes(body.replace("### Verification and limitations", extra + extra + "### Verification and limitations"), repository), /sections must be ordered/);
  assert.throws(() => parseReleaseNotes(body.replace("### Shipped changes", extra + "### Shipped changes"), repository), /sections must be ordered/);
});

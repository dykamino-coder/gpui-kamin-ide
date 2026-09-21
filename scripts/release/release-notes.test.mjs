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

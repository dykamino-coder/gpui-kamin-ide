import assert from "node:assert/strict";
import test from "node:test";
import { implementationBranches, verifyImplementationLinks } from "./runtime-implementation-links.mjs";

const tracked = { repository: "example/project", branch: "fix/history" };
const branch = "- **Implementation branch:** `example/project:fix/history`.";
const linked = "- **Fix PR:** [#536](https://github.com/example/project/pull/536), merged; remaining verification is open.";
const pull = (extra = {}) => ({ number: 536, merged_at: "2026-10-29T17:21:46Z",
  base: { ref: "main", repo: { full_name: tracked.repository } },
  head: { ref: tracked.branch, repo: { full_name: tracked.repository } }, ...extra });

test("only explicit records declare branch ownership", () => {
  assert.deepEqual(implementationBranches("Review branch `fix/history` and #536."), []);
  assert.deepEqual(implementationBranches(branch), [tracked]);
  assert.equal(implementationBranches(`${branch}\n${branch.replace("fix/history", "fix/tasks")}`).length, 2);
  for (const bad of [branch.replace("`.", "`"), branch.replace("fix/history", "../history"), "- **Implementation branch:** TODO"]) {
    assert.throws(() => implementationBranches(bad), /Invalid Implementation branch/);
  }
});

test("a merged concurrent fix cannot remain Fix PR none, even if prose mentions it", () => {
  const stale = `${branch}\n- **Fix PR:** none.\nHistorical discussion: [#536](https://github.com/example/project/pull/536).`;
  assert.throws(() => verifyImplementationLinks(stale, tracked, [pull()]), /merged #536; reconcile/);
});

test("a correct link permits an open incident without inferring acceptance or release", () => {
  assert.doesNotThrow(() => verifyImplementationLinks(`- **Status:** open; Windows gate pending.\n${linked}`, tracked, [pull()]));
  assert.throws(() => verifyImplementationLinks(linked.replace("/536)", "/535)"), tracked, [pull()]), /Fix PR field/);
});

test("unmerged, other-branch, other-base and fork PRs cannot create false ownership", () => {
  const irrelevant = [pull({ merged_at: null }), pull({ head: { ref: "other", repo: { full_name: tracked.repository } } }),
    pull({ base: { ref: "development", repo: { full_name: tracked.repository } } }),
    pull({ head: { ref: tracked.branch, repo: { full_name: "another/fork" } } })];
  assert.doesNotThrow(() => verifyImplementationLinks("- **Fix PR:** none.", tracked, irrelevant));
});

test("every merged result from the explicitly owned branch needs a current link", () => {
  assert.throws(() => verifyImplementationLinks(linked, tracked, [pull(), pull({ number: 540 })]), /merged #540/);
  assert.doesNotThrow(() => verifyImplementationLinks(`${linked}\n- **Fix PR:** [#540](https://github.com/example/project/pull/540).`, tracked, [pull(), pull({ number: 540 })]));
});

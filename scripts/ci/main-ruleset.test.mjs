import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

test("owner ruleset requires the actual stable workflow checks without a bypass", () => {
  const ruleset = JSON.parse(readFileSync(".github/main-ruleset.json", "utf8"));
  assert.equal(ruleset.enforcement, "active");
  assert.deepEqual(ruleset.conditions.ref_name, { include: ["~DEFAULT_BRANCH"], exclude: [] });
  assert.deepEqual(ruleset.bypass_actors, []);
  const rules = Object.fromEntries(ruleset.rules.map((rule) => [rule.type, rule.parameters]));
  assert.ok(Object.hasOwn(rules, "non_fast_forward"));
  assert.ok(Object.hasOwn(rules, "deletion"));
  assert.equal(rules.pull_request.required_approving_review_count, 0);
  assert.equal(rules.pull_request.required_review_thread_resolution, true);
  assert.equal(rules.required_status_checks.strict_required_status_checks_policy, true);
  const checks = rules.required_status_checks.required_status_checks.map((check) => check.context);
  assert.equal(checks.length, 2);
  for (const [file, name] of [["pr-checks.yml", "required quality gate"], ["pr-metadata.yml", "PR title and description"]]) {
    assert.ok(checks.includes(name));
    assert.ok(readFileSync(`.github/workflows/${file}`, "utf8").includes(`    name: ${name}\n`));
  }
  const workflow = readFileSync(".github/workflows/pr-checks.yml", "utf8");
  const rootScope = new RegExp(workflow.match(/set_scope root_node '([^']+)'/)[1]);
  for (const file of [".github/main-ruleset.json", ".github/workflows/pr-metadata.yml"]) {
    assert.match(file, rootScope, `Changing ${file} must run this contract test`);
  }
});

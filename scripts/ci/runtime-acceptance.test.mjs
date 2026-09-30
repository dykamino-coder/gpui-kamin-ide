import assert from "node:assert/strict";
import test from "node:test";
import {
  primaryTask,
  requiresWindowsRuntimeGate,
  taskCard,
  validateRuntimeAcceptance,
} from "./runtime-acceptance.mjs";

const sha = "a".repeat(40);
const files = ["crates/shell/src/web/mod.rs"];
const body = `## Задача
BR-31 — wake the CEF delivery pump.

## Проверки
Windows quality and release candidate passed.
`;

test("real BR-31 and INC-0002 cards require a live Windows gate", async () => {
  for (const taskId of ["BR-06", "BR-22", "BR-23", "BR-24", "BR-31",
    "INC-2026-0002", "INC-2026-0038", "INC-2026-0043", "INC-2026-0047"]) {
    assert.equal(requiresWindowsRuntimeGate(await taskCard(taskId)), true);
  }
  for (const taskId of ["BR-02", "BR-14", "BR-17", "INC-2026-0026"]) {
    assert.equal(requiresWindowsRuntimeGate(await taskCard(taskId)), false);
  }
});

test("a green Windows build cannot pass a task-specific CEF runtime gate", async () => {
  assert.equal(primaryTask(body), "BR-31");
  const card = await taskCard("BR-31");
  assert.throws(() => validateRuntimeAcceptance({
    body, files, headSha: sha, card,
  }), /requires ## Runtime acceptance/);
});

test("a task ID in the title works for existing PR descriptions", async () => {
  assert.equal(primaryTask("## Что было\nРендеринг панели задерживается.",
    "fix(shell): wake delivery (BR-31)"), "BR-31");
  const card = await taskCard("BR-31");
  assert.throws(() => validateRuntimeAcceptance({
    body: "## Что было\nРендеринг панели задерживается.",
    title: "fix(shell): wake delivery (BR-31)", files, headSha: sha, card,
  }), /requires ## Runtime acceptance/);
});

test("letter-suffixed BR cards remain identifiable", async () => {
  assert.equal(primaryTask("## Task\nBR-18A local hook investigation"), "BR-18A");
  assert.match(await taskCard("BR-18A"), /^### BR-18A/);
});

test("a required gate needs the exact candidate and task-specific evidence", async () => {
  const card = await taskCard("BR-31");
  const accepted = `${body}
## Runtime acceptance
- Task: BR-31
- Windows runtime merge gate: passed
- Candidate: ${sha}
- Scenario: Reveal Agents view without pointer input after renderer reap.
- Observed: Rows appeared within one second without a forced repaint.
- Evidence: https://github.com/example/private-evidence/tree/${sha}/br-31
`;
  assert.doesNotThrow(() => validateRuntimeAcceptance({
    body: accepted, files, headSha: sha, card,
  }));
  assert.throws(() => validateRuntimeAcceptance({
    body: accepted.replace(`Candidate: ${sha}`, `Candidate: ${"b".repeat(40)}`),
    files, headSha: sha, card,
  }), /exact PR head/);
  assert.throws(() => validateRuntimeAcceptance({
    body: accepted.replace(/https:\/\/github.com\/example\/private-evidence\/tree\/\S+/,
      "https://github.com/example/repo/actions/runs/123"),
    files, headSha: sha, card,
  }), /generic Actions run/);
});

test("other changes keep their ordinary proportional CI checks", async () => {
  const diagnosticsCard = await taskCard("BR-17");
  assert.doesNotThrow(() => validateRuntimeAcceptance({
    body, files: ["docs/README.md"], headSha: sha, card: "",
  }));
  assert.doesNotThrow(() => validateRuntimeAcceptance({
    body: "## Task\nNo BR/INC task; change a standalone feature.\n",
    files, headSha: sha, card: "",
  }));
  assert.doesNotThrow(() => validateRuntimeAcceptance({
    body: "## Task\nBR-17 persistent diagnostics.\n",
    files, headSha: sha, card: diagnosticsCard,
  }));
});

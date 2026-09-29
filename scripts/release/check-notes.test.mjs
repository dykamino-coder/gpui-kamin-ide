import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { test } from "node:test";

import { repositoryFromRemote } from "./check-notes.mjs";

const cli = fileURLToPath(new URL("./check-notes.mjs", import.meta.url));

/** Запуск CLI ровно так, как его запустит человек: тело PR подаётся на вход. */
function run(body) {
  try {
    const stdout = execFileSync(process.execPath, [cli, "--repo", "owner/repo"], {
      input: body,
      encoding: "utf8",
      stdio: ["pipe", "pipe", "pipe"],
    });
    return { code: 0, stdout, stderr: "" };
  } catch (error) {
    return { code: error.status, stdout: error.stdout ?? "", stderr: error.stderr ?? "" };
  }
}

const shipped = "- что-то сделано — https://github.com/owner/repo/pull/7\n";
const upgrade = "- действий не требуется\n";
const known = "- остаток — https://github.com/owner/repo/issues/9\n";

function notes({ up = upgrade, kn = known } = {}) {
  return [
    "## Release notes", "",
    "### Shipped changes", "", shipped,
    "### Upgrade notes", "", up,
    "### Known issues", "", kn,
    "### Verification and limitations", "", "- прогнаны тесты", "",
  ].join("\n");
}

test("правильные заметки принимаются", () => {
  const result = run(notes());
  assert.equal(result.code, 0);
  assert.match(result.stdout, /заметки релиза в порядке/);
  assert.match(result.stdout, /PR в «Shipped changes»: 1/);
});

test("абзац вместо списка в Upgrade notes отклоняется", () => {
  // Та самая ошибка, что дважды роняла release PR на сервере.
  const result = run(notes({ up: "Отдельных действий не требуется.\n" }));
  assert.equal(result.code, 1);
  assert.match(result.stderr, /Upgrade notes/);
});

test("пункт Known issues без ссылки на задачу отклоняется", () => {
  const result = run(notes({ kn: "- остаток работы без ссылки\n" }));
  assert.equal(result.code, 1);
  assert.match(result.stderr, /known issue/i);
});

test("пустое тело отличается от неверного: код 2, а не 1", () => {
  // Отдельный код возврата, чтобы «нечего проверять» не выглядело как
  // «проверка провалена».
  const result = run("   \n");
  assert.equal(result.code, 2);
  assert.match(result.stderr, /пустое/);
});

test("репозиторий распознаётся из обоих видов адреса origin", () => {
  assert.equal(repositoryFromRemote("https://github.com/owner/repo.git"), "owner/repo");
  assert.equal(repositoryFromRemote("git@github.com:owner/repo.git"), "owner/repo");
  assert.equal(repositoryFromRemote("https://example.com/owner/repo"), null);
});

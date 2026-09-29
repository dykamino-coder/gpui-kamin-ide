#!/usr/bin/env node
// Локальная проверка тела release PR тем же разборщиком, что и CI.
//
// Правила заметок (список в «Upgrade notes», полный URL задачи в каждом
// пункте «Known issues», ссылка на PR в каждом пункте «Shipped changes»)
// проверялись только на сервере, по одной ошибке за прогон. Разборщик лежит
// рядом, поэтому та же проверка доступна до пуша:
//
//   node scripts/release/check-notes.mjs body.md
//   gh pr view <N> --json body --jq .body | node scripts/release/check-notes.mjs
//
// Репозиторий берётся из `--repo`, иначе из origin, иначе из умолчания.
import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";

import { parseReleaseNotes } from "./release-notes.mjs";

const DEFAULT_REPOSITORY = "dykamino-coder/gpui-kamin-ide";

export function repositoryFromRemote(remoteUrl) {
  const match = /github\.com[:/]([A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+?)(?:\.git)?$/.exec(
    (remoteUrl ?? "").trim(),
  );
  return match ? match[1] : null;
}

function resolveRepository(argv) {
  const flag = argv.indexOf("--repo");
  if (flag !== -1 && argv[flag + 1]) return argv[flag + 1];
  try {
    const remote = execFileSync("git", ["remote", "get-url", "origin"], {
      encoding: "utf8",
      stdio: ["ignore", "pipe", "ignore"],
    });
    return repositoryFromRemote(remote) ?? DEFAULT_REPOSITORY;
  } catch {
    return DEFAULT_REPOSITORY;
  }
}

function readBody(argv) {
  const path = argv.find((arg, index) => index >= 2 && !arg.startsWith("--") && argv[index - 1] !== "--repo");
  if (path) return readFileSync(path, "utf8");
  return readFileSync(0, "utf8");
}

function main() {
  const repository = resolveRepository(process.argv);
  let body;
  try {
    body = readBody(process.argv);
  } catch (error) {
    console.error(`не прочитать тело PR: ${error.message}`);
    process.exit(2);
  }
  if (body.trim() === "") {
    console.error("тело PR пустое: передайте файл или подайте текст на вход");
    process.exit(2);
  }
  try {
    const notes = parseReleaseNotes(body, repository);
    const pulls = notes.pullNumbers ?? [];
    console.log(`заметки релиза в порядке (${repository}); PR в «Shipped changes»: ${pulls.length}`);
  } catch (error) {
    console.error(`заметки релиза не проходят проверку: ${error.message}`);
    process.exit(1);
  }
}

// Запуск как скрипта — не при импорте из теста.
if (process.argv[1] && process.argv[1].endsWith("check-notes.mjs")) main();

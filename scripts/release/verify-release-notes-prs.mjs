#!/usr/bin/env node

import { execFile } from "node:child_process";
import { readFile } from "node:fs/promises";
import { promisify } from "node:util";

const run = promisify(execFile);

export function validateReleaseNotePull(pull, paths) {
  if (pull.merged_at == null || pull.base?.ref !== "main" ||
      !/^[0-9a-f]{40}$/.test(pull.merge_commit_sha ?? "")) {
    throw new Error(`PR #${pull.number} is not a merged main-branch change`);
  }
  if (/^(docs\(|chore\(release\))/i.test(pull.title ?? "")) {
    throw new Error(`PR #${pull.number} is documentation or a version bump`);
  }
  if (paths.length === 0 || paths.every((path) =>
    path.endsWith(".md") || path.startsWith("docs/") ||
    path.includes("/runtime-issues/"))) {
    throw new Error(`PR #${pull.number} only registered or documented work`);
  }
  return pull.merge_commit_sha;
}

function valueAfter(flag) {
  const index = process.argv.indexOf(flag);
  if (index === -1 || !process.argv[index + 1]) throw new Error(`Missing ${flag}`);
  return process.argv[index + 1];
}

if (process.argv[1]?.endsWith("/verify-release-notes-prs.mjs")) {
  try {
    const repository = valueAfter("--repository");
    if (!/^[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+$/.test(repository)) {
      throw new Error("Expected a GitHub owner/repository name");
    }
    const head = valueAfter("--head");
    const previousTag = process.argv.includes("--previous-tag")
      ? valueAfter("--previous-tag") : null;
    const numbers = (await readFile(valueAfter("--pulls-file"), "utf8"))
      .trim().split(/\r?\n/).filter(Boolean);
    if (numbers.length === 0) throw new Error("Release notes cite no change PRs");
    for (const number of numbers) {
      if (!/^[1-9]\d*$/.test(number)) throw new Error(`Invalid PR number: ${number}`);
      const { stdout: pullJson } = await run("gh", [
        "api", `repos/${repository}/pulls/${number}`,
      ]);
      const { stdout: pathList } = await run("gh", [
        "api", `repos/${repository}/pulls/${number}/files`,
        "--paginate", "--jq", ".[].filename",
      ]);
      const pull = JSON.parse(pullJson);
      const mergeSha = validateReleaseNotePull(pull,
        pathList.trim().split(/\r?\n/).filter(Boolean));
      await run("git", ["merge-base", "--is-ancestor", mergeSha, head]);
      if (previousTag) {
        try {
          await run("git", ["merge-base", "--is-ancestor", mergeSha, previousTag]);
        } catch (error) {
          if (error.code === 1) continue;
          throw error;
        }
        throw new Error(`PR #${number} was already included by ${previousTag}`);
      }
    }
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error));
    process.exitCode = 1;
  }
}

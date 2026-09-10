#!/usr/bin/env node

import { readFile } from "node:fs/promises";
import { resolve } from "node:path";
import { inspectReleaseChange } from "./provenance.mjs";

function valueAfter(flag) {
  const index = process.argv.indexOf(flag);
  if (index === -1 || !process.argv[index + 1]) {
    throw new Error(`Missing ${flag}`);
  }
  return process.argv[index + 1];
}

try {
  const changedFiles = process.argv.includes("--changed-files")
    ? (await readFile(resolve(valueAfter("--changed-files")), "utf8"))
        .split(/\r?\n/)
        .filter(Boolean)
    : undefined;
  const result = await inspectReleaseChange({
    repoRoot: resolve(valueAfter("--repo")),
    baseRevision: valueAfter("--base"),
    headRevision: valueAfter("--head"),
    changedPaths: changedFiles,
  });
  process.stdout.write(`${JSON.stringify(result)}\n`);
} catch (error) {
  console.error(error instanceof Error ? error.message : String(error));
  process.exitCode = 1;
}

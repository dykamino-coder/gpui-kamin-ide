#!/usr/bin/env node

import { execFileSync } from "node:child_process";
import { fileURLToPath } from "node:url";

function isAncestor(ancestor, descendant) {
  try {
    execFileSync("git", ["merge-base", "--is-ancestor", ancestor, descendant]);
    return true;
  } catch (error) {
    if (error.status === 1) return false;
    throw error;
  }
}

export function shouldPromoteLatest(releaseSha, currentAliasSha, ancestor = isAncestor) {
  if (!currentAliasSha) return true;
  if (ancestor(currentAliasSha, releaseSha)) return true;
  if (ancestor(releaseSha, currentAliasSha)) return false;
  throw new Error("Rolling release tag and requested release are on divergent histories");
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const releaseSha = process.argv[2];
  const currentAliasSha = process.argv[3] ?? "";
  if (!/^[0-9a-f]{40}$/.test(releaseSha ?? "") ||
      (currentAliasSha && !/^[0-9a-f]{40}$/.test(currentAliasSha))) {
    console.error("Expected full release and optional rolling-tag commit SHAs");
    process.exitCode = 1;
  } else {
    try {
      process.stdout.write(`${shouldPromoteLatest(releaseSha, currentAliasSha)}\n`);
    } catch (error) {
      console.error(error instanceof Error ? error.message : String(error));
      process.exitCode = 1;
    }
  }
}

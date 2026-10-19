import { execFileSync } from "node:child_process";
import { readdirSync, readFileSync } from "node:fs";
import { resolve } from "node:path";
import { pathToFileURL } from "node:url";

const format = /^- \*\*Published fix:\*\* `(kaminide-v\d+\.\d+\.\d+)`; commit `([a-f0-9]{40})`; \[#(\d+)\]\(https:\/\/github\.com\/([\w.-]+\/[\w.-]+)\/pull\/\3\)\.$/;

export function deliveryRecords(card) {
  const lines = card.split(/\r?\n/).filter((line) => line.startsWith("- **Published fix:**"));
  if (lines.length && /\*\*Status:\*\*[^\n]*(?:not yet in a published release|not yet released)/i.test(card)) {
    throw new Error("Published fix contradicts the current Status field");
  }
  return lines.map((line) => {
    const match = line.match(format);
    if (!match) throw new Error(`Invalid Published fix record: ${line}`);
    return { tag: match[1], commit: match[2], pr: Number(match[3]), repository: match[4] };
  });
}

export function verifyDelivery(record, { repo = ".", release }) {
  if (release.tag_name !== record.tag || release.draft || !release.published_at) {
    throw new Error(`${record.tag} is not a published GitHub Release`);
  }
  // Full SHAs and refs/tags avoid resolving a similarly named branch instead.
  const git = (...args) => execFileSync("git", ["-C", repo, ...args], { encoding: "utf8", stdio: ["ignore", "pipe", "pipe"] });
  try {
    git("rev-parse", "--verify", `refs/tags/${record.tag}^{commit}`);
    git("merge-base", "--is-ancestor", record.commit, `refs/tags/${record.tag}`);
  } catch {
    throw new Error(`${record.tag} does not contain recorded fix #${record.pr} (${record.commit}) or full Git history/tags are missing`);
  }
}

export function checkRuntimeDeliveries({ repo = "." } = {}) {
  const directory = resolve(repo, "extensions/claude-bridge/runtime-issues");
  const releases = new Map();
  let count = 0;
  for (const file of readdirSync(directory).filter((name) => /^INC-\d+-\d+\.md$/.test(name)).sort()) {
    try {
      for (const record of deliveryRecords(readFileSync(resolve(directory, file), "utf8"))) {
        // Historical delivery in an upstream repo stays upstream even in a fork.
        const key = `${record.repository}/${record.tag}`;
        if (!releases.has(key)) {
          releases.set(key, JSON.parse(execFileSync("gh", ["api", `repos/${record.repository}/releases/tags/${record.tag}`], { encoding: "utf8" })));
        }
        verifyDelivery(record, { repo, release: releases.get(key) });
        count++;
      }
    } catch (error) {
      throw new Error(`${file}: ${error.message}`);
    }
  }
  return count;
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  try {
    console.log(`Verified ${checkRuntimeDeliveries()} recorded runtime deliveries (publication + Git ancestry; not runtime acceptance).`);
  } catch (error) {
    console.error(error.message);
    process.exitCode = 1;
  }
}

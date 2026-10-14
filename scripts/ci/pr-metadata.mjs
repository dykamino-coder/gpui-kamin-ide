#!/usr/bin/env node

import { readFile } from "node:fs/promises";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { parseReleaseNotes } from "../release/release-notes.mjs";

const requiredSections = [
  ["Summary", /^(summary|change|what changed|что изменено|что и зачем|что и почему)$/i],
  ["Validation", /^(validation|verification|how it was verified|проверка|проверки|как проверено)$/i],
  ["Risks and limitations", /^(risks and limitations|risks|limitations|риски|ограничения|границы)$/i],
  ["Versioning", /^(versioning|versions|версионирование|версии|версионирование и публикация)$/i],
];

function sections(body) {
  const result = [];
  let current;
  let fence;
  for (const line of body.replace(/<!--[\s\S]*?-->/g, "").split(/\r?\n/)) {
    const marker = line.match(/^\s*(`{3,}|~{3,})/);
    if (marker && (!fence || (marker[1][0] === fence[0] &&
        marker[1].length >= fence.length && !line.slice(marker[0].length).trim()))) {
      fence = fence ? undefined : marker[1];
    } else if (!fence) {
      const heading = line.match(/^##\s+(.+?)\s*#*\s*$/);
      if (heading) {
        current = { name: heading[1], lines: [] };
        result.push(current);
        continue;
      }
    }
    current?.lines.push(line);
  }
  return result;
}

function content(lines) {
  return lines.join("\n").replace(/<!--[\s\S]*?-->/g, "").split(/\r?\n/)
    .filter((line) => !/^\s*[-*]\s+\[ \]/.test(line))
    .map((line) => line.replace(/^\s*[-*]\s+\[x\]\s*/i, ""))
    .filter((line) => !/^\s*(?:[-*]\s+)?(?:[`~]{3,}.*|<[^>]+>|TODO|TBD|N\/A|None|Not applicable)[.!:]?\s*$/i.test(line))
    .join("\n").trim();
}

export function validatePrMetadata({ title, body = "", author, release = false, repository }) {
  if (typeof title !== "string" ||
      !/^[a-z][a-z0-9-]*(?:\([^()\r\n]+\))?!?: \S[^\r\n]*$/.test(title) ||
      [...title.matchAll(/\p{Letter}/gu)].some(([letter]) => !/\p{Script=Latin}/u.test(letter))) {
    throw new Error("PR title must use Conventional Commits and English wording (Latin letters)");
  }
  if (typeof body !== "string" || !content(body.split(/\r?\n/))) {
    throw new Error("PR description must be filled in");
  }
  if (release) {
    parseReleaseNotes(body, repository);
    return;
  }
  // This exception concerns generated formatting, never review/merge authority.
  if (author?.login === "dependabot[bot]" && author.type === "Bot") return;

  const parts = sections(body);
  for (const [name, pattern] of requiredSections) {
    const part = parts.find(({ name: heading }) => pattern.test(heading));
    if (!part || !content(part.lines)) {
      throw new Error(`Fill ## ${name}; comments, unchecked boxes and bare N/A are not a result`);
    }
    if (name === "Versioning" &&
        !/^(?:[-*]\s+)?Release versions:\s*unchanged\.?\s*$/mi.test(content(part.lines))) {
      throw new Error("Non-release PRs must declare Release versions: unchanged in ## Versioning");
    }
  }
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    const eventPath = process.argv[process.argv.indexOf("--event") + 1];
    if (!process.argv.includes("--event") || !eventPath) throw new Error("Expected --event <JSON file>");
    const payload = JSON.parse(await readFile(eventPath, "utf8"));
    const pr = payload.pull_request ?? payload;
    const release = process.env.RELEASE_CANDIDATE ?? "false";
    if (!["true", "false"].includes(release)) throw new Error("Invalid RELEASE_CANDIDATE");
    validatePrMetadata({
      title: pr.title,
      body: pr.body ?? "",
      author: pr.user,
      release: release === "true",
      repository: process.env.GITHUB_REPOSITORY,
    });
    process.stdout.write("PR title and description format passed; factual acceptance still requires review.\n");
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error));
    process.exitCode = 1;
  }
}

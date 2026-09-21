#!/usr/bin/env node

import { createHash } from "node:crypto";
import { readFile, writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

const heading = /^## Release notes\s*$/gm;
const shippedHeading = /^### Shipped changes\s*$/gm;
const upgradeHeading = /^### Upgrade notes\s*$/gm;
const knownIssuesHeading = /^### Known issues\s*$/gm;
const verificationHeading = /^### Verification and limitations\s*$/gm;
const sectionOrder = [
  "Shipped changes", "Upgrade notes", "Known issues", "Verification and limitations",
];

function sectionBetween(body, startPattern, endPattern, label) {
  const starts = [...body.matchAll(startPattern)];
  if (starts.length !== 1) {
    throw new Error(`Expected exactly one ${label} heading`);
  }
  const start = starts[0].index + starts[0][0].length;
  const rest = body.slice(start);
  const end = endPattern.exec(rest);
  return (end ? rest.slice(0, end.index) : rest).trim();
}

export function parseReleaseNotes(body, repository) {
  if (!/^[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+$/.test(repository)) {
    throw new Error("Expected a GitHub owner/repository name");
  }
  const notes = sectionBetween(body, heading, /^## /m, "Release notes");
  if (!notes || notes.length > 12_000 || notes.includes("<!--")) {
    throw new Error("Release notes must be filled in, without template comments");
  }
  const sections = [...notes.matchAll(/^### (.+?)\s*$/gm)].map((match) => match[1]);
  if (sections[0] !== "Shipped changes" ||
      sections.at(-1) !== "Verification and limitations" ||
      sections.length !== new Set(sections).size ||
      sections.some((section) => !sectionOrder.includes(section)) ||
      sections.some((section, index) => index > 0 &&
        sectionOrder.indexOf(section) < sectionOrder.indexOf(sections[index - 1]))) {
    throw new Error("Release notes sections must be ordered: Shipped changes, optional Upgrade notes, optional Known issues, Verification and limitations");
  }
  const shipped = sectionBetween(
    notes,
    shippedHeading,
    /^### /m,
    "Shipped changes",
  );
  const verification = sectionBetween(
    notes,
    verificationHeading,
    /^### /m,
    "Verification and limitations",
  );
  const upgrade = sections.includes("Upgrade notes")
    ? sectionBetween(notes, upgradeHeading, /^### /m, "Upgrade notes") : null;
  const knownIssues = sections.includes("Known issues")
    ? sectionBetween(notes, knownIssuesHeading, /^### /m, "Known issues") : null;
  const shippedBullets = shipped.split(/\r?\n/).filter((line) => /^- \S/.test(line));
  const verificationBullets = verification.split(/\r?\n/).filter((line) => /^- \S/.test(line));
  if (shippedBullets.length === 0 || verificationBullets.length === 0) {
    throw new Error("Release notes need shipped changes and verification bullets");
  }
  if (upgrade !== null && !upgrade.split(/\r?\n/).some((line) => /^- \S/.test(line))) {
    throw new Error("Upgrade notes need at least one bullet");
  }
  const escapedRepository = repository.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  if (knownIssues !== null) {
    const issueBullets = knownIssues.split(/\r?\n/).filter((line) => /^- \S/.test(line));
    const publicTaskUrl = new RegExp(
      `https://github\\.com/${escapedRepository}/(?:issues/\\d+|pull/\\d+|blob/[^\\s)]+)`,
    );
    if (issueBullets.length === 0 || issueBullets.some((bullet) => !publicTaskUrl.test(bullet))) {
      throw new Error("Every known issue must link to a public task in this repository");
    }
  }
  const pullUrl = new RegExp(
    `https://github\\.com/${escapedRepository}/pull/(\\d+)\\b`,
    "g",
  );
  const pullNumbers = new Set();
  for (const bullet of shippedBullets) {
    const references = [...bullet.matchAll(pullUrl)];
    if (references.length === 0) {
      throw new Error("Every shipped change must link to an implementation PR");
    }
    for (const reference of references) pullNumbers.add(Number(reference[1]));
  }
  if (notes.match(/https:\/\/github\.com\/[^\s)]+\/pull\/\d+/g)?.some(
    (url) => !url.startsWith(`https://github.com/${repository}/pull/`),
  )) {
    throw new Error("Release notes must not cite pull requests from another repository");
  }
  const markdown = `## Release notes\n\n${notes}\n`;
  return {
    markdown,
    sha256: createHash("sha256").update(markdown).digest("hex"),
    pullNumbers: [...pullNumbers].sort((a, b) => a - b),
  };
}

function valueAfter(flag) {
  const index = process.argv.indexOf(flag);
  if (index === -1 || !process.argv[index + 1]) throw new Error(`Missing ${flag}`);
  return process.argv[index + 1];
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    const body = await readFile(resolve(valueAfter("--body-file")), "utf8");
    const result = parseReleaseNotes(body, valueAfter("--repository"));
    if (process.argv.includes("--expected-sha256") &&
        result.sha256 !== valueAfter("--expected-sha256")) {
      throw new Error("Release notes changed after the release context was validated");
    }
    await writeFile(resolve(valueAfter("--output")), result.markdown);
    if (process.argv.includes("--pulls-output")) {
      await writeFile(resolve(valueAfter("--pulls-output")),
        `${result.pullNumbers.join("\n")}\n`);
    }
    process.stdout.write(`${result.sha256}\n`);
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error));
    process.exitCode = 1;
  }
}

import { execFileSync } from "node:child_process";

// Explicit branch ownership is opt-in. Prose references never infer acceptance
// or silently make an unrelated PR responsible for an incident.
const branchLine = /^- \*\*Implementation branch:\*\* `([\w.-]+\/[\w.-]+):([\w./-]+)`\.$/;

export function implementationBranches(card) {
  return card.split(/\r?\n/).filter((line) => line.startsWith("- **Implementation branch:**"))
    .map((line) => {
      const match = line.match(branchLine);
      if (!match || match[2].includes("..") || match[2].endsWith("/")) {
        throw new Error(`Invalid Implementation branch record: ${line}`);
      }
      return { repository: match[1], branch: match[2] };
    });
}

export function verifyImplementationLinks(card, tracked, pulls) {
  const fix = card.split(/\r?\n/).filter((line) => line.startsWith("- **Fix PR:**")).join("\n");
  for (const pr of pulls) {
    if (!pr.merged_at || pr.base?.ref !== "main" ||
        pr.base?.repo?.full_name !== tracked.repository ||
        pr.head?.repo?.full_name !== tracked.repository || pr.head?.ref !== tracked.branch) continue;
    const link = `https://github.com/${tracked.repository}/pull/${pr.number}`;
    if (!fix.includes(`](${link})`)) {
      throw new Error(`Tracked branch ${tracked.branch} has merged #${pr.number}; reconcile the card's Fix PR field. This check does not close the incident or pass its runtime gates.`);
    }
  }
}

export function mergedBranchPulls({ repository, branch }) {
  const owner = repository.split("/")[0];
  const query = new URLSearchParams({ state: "closed", head: `${owner}:${branch}`, per_page: "100" });
  return JSON.parse(execFileSync("gh", ["api", "--paginate", "--slurp",
    `repos/${repository}/pulls?${query}`], { encoding: "utf8" })).flat();
}

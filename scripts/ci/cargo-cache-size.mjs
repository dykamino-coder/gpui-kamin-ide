// Report uncompressed bytes without deleting Cargo fingerprints or build products.
import { appendFile, lstat, readdir } from "node:fs/promises";
import { homedir } from "node:os";
import { join } from "node:path";

async function size(path) {
  const entry = await lstat(path).catch((error) => {
    if (error.code === "ENOENT") return null;
    throw error;
  });
  if (!entry || entry.isSymbolicLink()) return 0;
  if (!entry.isDirectory()) return entry.size;
  let bytes = 0;
  for (const child of await readdir(path)) bytes += await size(join(path, child));
  return bytes;
}

const paths = [
  ["registry", join(homedir(), ".cargo", "registry")],
  ["git", join(homedir(), ".cargo", "git")],
  ["target", process.argv[2] ?? "target"],
];
const lines = ["### Cargo cache bytes (before compression)", ""];
for (const [name, path] of paths) lines.push(`- ${name}: ${await size(path)} bytes`);
lines.push("- Incremental and debug symbols disabled, matching the quality jobs.", "");
console.log(lines.join("\n"));
if (process.env.GITHUB_STEP_SUMMARY) {
  await appendFile(process.env.GITHUB_STEP_SUMMARY, `${lines.join("\n")}\n`);
}

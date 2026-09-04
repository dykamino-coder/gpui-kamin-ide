#!/usr/bin/env node

import { readFile, writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import {
  assertCleanWorktree,
  createProvenance,
  verifyProvenance,
} from "./provenance.mjs";

function parseArgs(argv) {
  const [command, ...rest] = argv;
  const values = new Map();
  for (let index = 0; index < rest.length; index += 2) {
    const key = rest[index];
    const value = rest[index + 1];
    if (!key?.startsWith("--") || !value)
      throw new Error(`Invalid argument: ${key ?? ""}`);
    values.set(key.slice(2), value);
  }
  return { command, values };
}

function required(values, name) {
  const value = values.get(name);
  if (!value) throw new Error(`Missing --${name}`);
  return value;
}

async function main() {
  const { command, values } = parseArgs(process.argv.slice(2));
  const repoRoot = resolve(values.get("repo-root") ?? process.cwd());
  const installerPath = resolve(required(values, "installer"));

  if (command === "create") {
    const outputPath = resolve(required(values, "output"));
    await assertCleanWorktree(repoRoot);
    const provenance = await createProvenance({ repoRoot, installerPath });
    await writeFile(outputPath, `${JSON.stringify(provenance, null, 2)}\n`, {
      flag: "wx",
    });
    process.stdout.write(`${outputPath}\n`);
    return;
  }

  if (command === "verify") {
    const provenancePath = resolve(required(values, "provenance"));
    const releaseSha = required(values, "release-sha");
    const provenance = JSON.parse(await readFile(provenancePath, "utf8"));
    const checked = await verifyProvenance({
      repoRoot,
      installerPath,
      provenance,
      releaseSha,
    });
    process.stdout.write(`${JSON.stringify(checked)}\n`);
    return;
  }

  throw new Error(
    "Usage: provenance-cli.mjs <create|verify> --installer PATH ...",
  );
}

main().catch((error) => {
  process.stderr.write(
    `${error instanceof Error ? error.message : String(error)}\n`,
  );
  process.exitCode = 1;
});

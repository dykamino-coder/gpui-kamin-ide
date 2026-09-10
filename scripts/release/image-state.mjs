#!/usr/bin/env node

import {
  assertImageLabels,
  findDockerHubImage,
} from "./docker-registry.mjs";

function args(argv) {
  const values = new Map();
  for (let index = 0; index < argv.length; index += 2) {
    const key = argv[index];
    const value = argv[index + 1];
    if (!key?.startsWith("--") || !value) {
      throw new Error(`Invalid argument: ${key ?? ""}`);
    }
    values.set(key.slice(2), value);
  }
  return values;
}

function required(values, name) {
  const value = values.get(name);
  if (!value) throw new Error(`Missing --${name}`);
  return value;
}

async function main() {
  const values = args(process.argv.slice(2));
  const image = required(values, "image");
  const tag = required(values, "tag");
  const expected = {
    source: required(values, "source"),
    revision: required(values, "revision"),
    version: required(values, "version"),
  };
  let lastError;
  for (let attempt = 1; attempt <= 3; attempt += 1) {
    try {
      const result = await findDockerHubImage({ image, tag });
      if (result) {
        assertImageLabels(result.labels, expected);
        if (result.attestationCount < 1) {
          throw new Error("Image has no provenance attestation");
        }
        process.stdout.write(`${JSON.stringify({ exists: true, ...result })}\n`);
        return;
      }
    } catch (error) {
      lastError = error;
    }
    if (attempt < 3) {
      await new Promise((resolve) => setTimeout(resolve, 2_000));
    }
  }
  if (lastError) throw lastError;
  process.stdout.write(`${JSON.stringify({ exists: false })}\n`);
}

main().catch((error) => {
  process.stderr.write(
    `${error instanceof Error ? error.message : String(error)}\n`,
  );
  process.exitCode = 1;
});

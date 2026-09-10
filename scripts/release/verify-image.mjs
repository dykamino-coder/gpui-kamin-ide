#!/usr/bin/env node

import {
  assertImageLabels,
  inspectDockerHubImage,
} from "./docker-registry.mjs";

function args(argv) {
  const values = new Map();
  for (let index = 0; index < argv.length; index += 2) {
    const key = argv[index];
    const value = argv[index + 1];
    if (!key?.startsWith("--") || !value)
      throw new Error(`Invalid argument: ${key ?? ""}`);
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
  const expectedDigest = values.get("digest");
  const attempts = Number(values.get("attempts") ?? "1");
  if (!Number.isSafeInteger(attempts) || attempts < 1 || attempts > 20) {
    throw new Error("--attempts must be an integer between 1 and 20");
  }

  let lastError;
  for (let attempt = 1; attempt <= attempts; attempt += 1) {
    try {
      const result = await inspectDockerHubImage({ image, tag });
      assertImageLabels(result.labels, expected);
      if (result.attestationCount < 1)
        throw new Error("Image has no provenance attestation");
      if (expectedDigest && result.digest !== expectedDigest) {
        throw new Error(
          `Image digest ${result.digest} does not match ${expectedDigest}`,
        );
      }
      process.stdout.write(`${JSON.stringify(result)}\n`);
      return;
    } catch (error) {
      lastError = error;
      if (attempt < attempts)
        await new Promise((resolve) => setTimeout(resolve, 5_000));
    }
  }
  throw lastError;
}

main().catch((error) => {
  process.stderr.write(
    `${error instanceof Error ? error.message : String(error)}\n`,
  );
  process.exitCode = 1;
});

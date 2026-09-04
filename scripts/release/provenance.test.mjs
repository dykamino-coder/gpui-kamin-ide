import assert from "node:assert/strict";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import test from "node:test";
import {
  createProvenance,
  gitObjectId,
  readReleaseVersions,
  validateProvenance,
  verifyProvenance,
} from "./provenance.mjs";

const repoRoot = resolve(import.meta.dirname, "../..");

test("creates and verifies provenance against the release tree", async () => {
  const directory = await mkdtemp(join(tmpdir(), "kamin-provenance-"));
  const versions = await readReleaseVersions(repoRoot);
  const releaseSha = await gitObjectId(repoRoot, "HEAD");
  const installerName = `KaminIDE_${versions.app}_x64-setup.exe`;
  const installerPath = join(directory, installerName);
  try {
    await writeFile(installerPath, "fixture installer");
    const provenance = await createProvenance({ repoRoot, installerPath });
    const verified = await verifyProvenance({
      repoRoot,
      installerPath,
      provenance,
      releaseSha,
    });
    assert.equal(verified.installer.name, installerName);
    assert.equal(verified.versions.server, versions.server);
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});

test("rejects a changed installer", async () => {
  const directory = await mkdtemp(join(tmpdir(), "kamin-provenance-"));
  const versions = await readReleaseVersions(repoRoot);
  const releaseSha = await gitObjectId(repoRoot, "HEAD");
  const installerPath = join(
    directory,
    `KaminIDE_${versions.app}_x64-setup.exe`,
  );
  try {
    await writeFile(installerPath, "original");
    const provenance = await createProvenance({ repoRoot, installerPath });
    await writeFile(installerPath, "changed");
    await assert.rejects(
      verifyProvenance({
        repoRoot,
        installerPath,
        provenance,
        releaseSha,
      }),
      /Installer size does not match provenance|Installer SHA-256 does not match provenance/,
    );
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});

test("rejects malformed provenance", () => {
  assert.throws(
    () => validateProvenance({ schemaVersion: 2 }),
    /schemaVersion/,
  );
});

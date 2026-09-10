import assert from "node:assert/strict";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import test from "node:test";
import {
  classifyReleaseChange,
  compareReleaseVersions,
  createProvenance,
  gitObjectId,
  inspectReleaseChange,
  readReleaseVersions,
  validateProvenance,
  validateReleaseChangePaths,
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

test("classifies only a coordinated version increase as a release", () => {
  const previous = { app: "1.0.55", server: "6.3.132" };
  assert.deepEqual(classifyReleaseChange(previous, previous), {
    release: false,
    previous,
    current: previous,
  });
  assert.equal(
    classifyReleaseChange(previous, {
      app: "1.0.56",
      server: "6.3.133",
    }).release,
    true,
  );
  assert.throws(
    () =>
      classifyReleaseChange(previous, {
        app: "1.0.56",
        server: previous.server,
      }),
    /must change together/,
  );
  assert.throws(
    () =>
      classifyReleaseChange(previous, {
        app: "1.0.54",
        server: "6.3.133",
      }),
    /App release version must increase/,
  );
});

test("compares stable and prerelease versions", () => {
  assert.equal(compareReleaseVersions("1.2.3", "1.2.2"), 1);
  assert.equal(compareReleaseVersions("1.2.3", "1.2.3-rc.1"), 1);
  assert.equal(compareReleaseVersions("1.2.3-rc.2", "1.2.3-rc.1"), 1);
  assert.equal(compareReleaseVersions("1.2.3+build.2", "1.2.3+build.1"), 0);
});

test("requires a release PR to contain exactly version and lock files", () => {
  const releasePaths = [
    "Cargo.toml",
    "Cargo.lock",
    "extensions/claude-bridge/server/package.json",
    "extensions/claude-bridge/server/package-lock.json",
  ];
  assert.deepEqual(validateReleaseChangePaths(releasePaths), [...releasePaths].sort());
  assert.throws(
    () => validateReleaseChangePaths([...releasePaths, "src/main.rs"]),
    /unexpected: src\/main\.rs/,
  );
  assert.throws(
    () => validateReleaseChangePaths(releasePaths.slice(1)),
    /missing: Cargo\.toml/,
  );
});

test("inspects release versions from two git revisions", async () => {
  const head = await gitObjectId(repoRoot, "HEAD");
  const result = await inspectReleaseChange({
    repoRoot,
    baseRevision: head,
    headRevision: head,
  });
  assert.equal(result.release, false);
});

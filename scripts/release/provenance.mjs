import { createHash } from "node:crypto";
import { createReadStream } from "node:fs";
import { readFile, stat } from "node:fs/promises";
import { basename, join } from "node:path";
import { promisify } from "node:util";
import { execFile } from "node:child_process";

const execFileAsync = promisify(execFile);
const SHA256_RE = /^[a-f0-9]{64}$/;
const GIT_OID_RE = /^[a-f0-9]{40}$/;
const VERSION_RE = /^\d+\.\d+\.\d+(?:[-+][0-9A-Za-z.-]+)?$/;
const RELEASE_FILES = {
  app: "Cargo.toml",
  server: "extensions/claude-bridge/server/package.json",
};
export const RELEASE_CHANGE_PATHS = [
  RELEASE_FILES.app,
  "Cargo.lock",
  RELEASE_FILES.server,
  "extensions/claude-bridge/server/package-lock.json",
];

function requireString(value, field, pattern) {
  if (typeof value !== "string" || !pattern.test(value)) {
    throw new Error(`Invalid ${field}`);
  }
  return value;
}

export async function sha256File(filePath) {
  const hash = createHash("sha256");
  await new Promise((resolve, reject) => {
    const stream = createReadStream(filePath);
    stream.on("data", (chunk) => hash.update(chunk));
    stream.on("end", resolve);
    stream.on("error", reject);
  });
  return hash.digest("hex");
}

function parseReleaseVersions(cargoToml, serverManifestText) {
  const serverManifest = JSON.parse(serverManifestText);
  const workspace = cargoToml.match(
    /\[workspace\.package\][\s\S]*?^version\s*=\s*"([^"]+)"/m,
  );
  if (!workspace)
    throw new Error("Cannot read workspace package version from Cargo.toml");

  return {
    app: requireString(workspace[1], "versions.app", VERSION_RE),
    server: requireString(
      serverManifest.version,
      "versions.server",
      VERSION_RE,
    ),
  };
}

export async function readReleaseVersions(repoRoot) {
  const [cargoToml, serverManifest] = await Promise.all([
    readFile(join(repoRoot, RELEASE_FILES.app), "utf8"),
    readFile(join(repoRoot, RELEASE_FILES.server), "utf8"),
  ]);
  return parseReleaseVersions(cargoToml, serverManifest);
}

async function readGitFile(repoRoot, revision, filePath) {
  const { stdout } = await execFileAsync(
    "git",
    ["show", `${revision}:${filePath}`],
    { cwd: repoRoot, maxBuffer: 10 * 1024 * 1024 },
  );
  return stdout;
}

export async function readReleaseVersionsAt(repoRoot, revision) {
  const [cargoToml, serverManifest] = await Promise.all([
    readGitFile(repoRoot, revision, RELEASE_FILES.app),
    readGitFile(repoRoot, revision, RELEASE_FILES.server),
  ]);
  return parseReleaseVersions(cargoToml, serverManifest);
}

function semverParts(version) {
  const withoutBuild = version.split("+", 1)[0];
  const [core, prerelease = ""] = withoutBuild.split("-", 2);
  return {
    core: core.split(".").map(Number),
    prerelease: prerelease ? prerelease.split(".") : [],
  };
}

export function compareReleaseVersions(left, right) {
  requireString(left, "version", VERSION_RE);
  requireString(right, "version", VERSION_RE);
  const a = semverParts(left);
  const b = semverParts(right);
  for (let index = 0; index < 3; index += 1) {
    if (a.core[index] !== b.core[index]) {
      return a.core[index] > b.core[index] ? 1 : -1;
    }
  }
  if (a.prerelease.length === 0 || b.prerelease.length === 0) {
    if (a.prerelease.length === b.prerelease.length) return 0;
    return a.prerelease.length === 0 ? 1 : -1;
  }
  const length = Math.max(a.prerelease.length, b.prerelease.length);
  for (let index = 0; index < length; index += 1) {
    const leftPart = a.prerelease[index];
    const rightPart = b.prerelease[index];
    if (leftPart === rightPart) continue;
    if (leftPart === undefined) return -1;
    if (rightPart === undefined) return 1;
    const leftNumber = /^\d+$/.test(leftPart) ? Number(leftPart) : null;
    const rightNumber = /^\d+$/.test(rightPart) ? Number(rightPart) : null;
    if (leftNumber !== null && rightNumber !== null) {
      return leftNumber > rightNumber ? 1 : -1;
    }
    if (leftNumber !== null || rightNumber !== null) {
      return leftNumber !== null ? -1 : 1;
    }
    return leftPart > rightPart ? 1 : -1;
  }
  return 0;
}

export function classifyReleaseChange(previous, current) {
  const appChanged = previous.app !== current.app;
  const serverChanged = previous.server !== current.server;
  if (appChanged !== serverChanged) {
    throw new Error("App and server release versions must change together");
  }
  if (!appChanged) {
    return { release: false, previous, current };
  }
  if (compareReleaseVersions(current.app, previous.app) <= 0) {
    throw new Error("App release version must increase");
  }
  if (compareReleaseVersions(current.server, previous.server) <= 0) {
    throw new Error("Server release version must increase");
  }
  return { release: true, previous, current };
}

export function validateReleaseChangePaths(paths) {
  const actual = [...new Set(paths)].sort();
  const expected = [...RELEASE_CHANGE_PATHS].sort();
  const missing = expected.filter((path) => !actual.includes(path));
  const unexpected = actual.filter((path) => !expected.includes(path));
  if (missing.length > 0 || unexpected.length > 0) {
    const details = [
      missing.length > 0 ? `missing: ${missing.join(", ")}` : "",
      unexpected.length > 0 ? `unexpected: ${unexpected.join(", ")}` : "",
    ].filter(Boolean);
    throw new Error(`Release PR must change only the release files (${details.join("; ")})`);
  }
  return actual;
}

export async function inspectReleaseChange({
  repoRoot,
  baseRevision,
  headRevision = "HEAD",
  changedPaths,
}) {
  const [previous, current] = await Promise.all([
    readReleaseVersionsAt(repoRoot, baseRevision),
    readReleaseVersionsAt(repoRoot, headRevision),
  ]);
  const result = classifyReleaseChange(previous, current);
  if (result.release && changedPaths) {
    validateReleaseChangePaths(changedPaths);
  }
  return result;
}

export async function gitObjectId(repoRoot, revision, suffix = "^{commit}") {
  const { stdout } = await execFileAsync(
    "git",
    ["rev-parse", `${revision}${suffix}`],
    {
      cwd: repoRoot,
    },
  );
  return requireString(stdout.trim(), `git object for ${revision}`, GIT_OID_RE);
}

export async function assertCleanWorktree(repoRoot) {
  const { stdout } = await execFileAsync(
    "git",
    ["status", "--porcelain", "--untracked-files=all"],
    { cwd: repoRoot },
  );
  if (stdout.trim()) {
    throw new Error("Release provenance requires a clean git worktree");
  }
}

export function validateProvenance(value) {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    throw new Error("Provenance must be a JSON object");
  }
  if (value.schemaVersion !== 1)
    throw new Error("Unsupported provenance schemaVersion");

  const source = value.source;
  const versions = value.versions;
  const installer = value.installer;
  if (!source || typeof source !== "object")
    throw new Error("Missing source metadata");
  if (!versions || typeof versions !== "object")
    throw new Error("Missing version metadata");
  if (!installer || typeof installer !== "object")
    throw new Error("Missing installer metadata");

  const normalized = {
    schemaVersion: 1,
    source: {
      commit: requireString(source.commit, "source.commit", GIT_OID_RE),
      tree: requireString(source.tree, "source.tree", GIT_OID_RE),
    },
    versions: {
      app: requireString(versions.app, "versions.app", VERSION_RE),
      server: requireString(versions.server, "versions.server", VERSION_RE),
    },
    installer: {
      name: requireString(
        installer.name,
        "installer.name",
        /^KaminIDE_[^/]+_x64-setup\.exe$/,
      ),
      size: installer.size,
      sha256: requireString(installer.sha256, "installer.sha256", SHA256_RE),
    },
  };
  if (
    !Number.isSafeInteger(normalized.installer.size) ||
    normalized.installer.size <= 0
  ) {
    throw new Error("Invalid installer.size");
  }
  return normalized;
}

export async function createProvenance({
  repoRoot,
  installerPath,
  revision = "HEAD",
}) {
  const versions = await readReleaseVersions(repoRoot);
  const name = basename(installerPath);
  const expectedName = `KaminIDE_${versions.app}_x64-setup.exe`;
  if (name !== expectedName) {
    throw new Error(`Installer name must be ${expectedName}, got ${name}`);
  }
  const installerStat = await stat(installerPath);
  if (!installerStat.isFile() || installerStat.size <= 0) {
    throw new Error("Installer must be a non-empty file");
  }

  return validateProvenance({
    schemaVersion: 1,
    source: {
      commit: await gitObjectId(repoRoot, revision),
      tree: await gitObjectId(repoRoot, revision, "^{tree}"),
    },
    versions,
    installer: {
      name,
      size: installerStat.size,
      sha256: await sha256File(installerPath),
    },
  });
}

export async function verifyProvenance({
  repoRoot,
  installerPath,
  provenance,
  releaseSha,
}) {
  const checked = validateProvenance(provenance);
  requireString(releaseSha, "release SHA", GIT_OID_RE);
  const versions = await readReleaseVersions(repoRoot);
  const expectedTree = await gitObjectId(repoRoot, releaseSha, "^{tree}");
  const expectedInstallerName = `KaminIDE_${versions.app}_x64-setup.exe`;
  const installerStat = await stat(installerPath);
  const installerHash = await sha256File(installerPath);

  const assertions = [
    [
      checked.source.tree === expectedTree,
      "Installer source tree does not match release commit",
    ],
    [
      checked.versions.app === versions.app,
      "App version does not match release commit",
    ],
    [
      checked.versions.server === versions.server,
      "Server version does not match release commit",
    ],
    [
      checked.installer.name === expectedInstallerName,
      "Installer filename has the wrong app version",
    ],
    [
      checked.installer.name === basename(installerPath),
      "Installer filename does not match provenance",
    ],
    [
      checked.installer.size === installerStat.size,
      "Installer size does not match provenance",
    ],
    [
      checked.installer.sha256 === installerHash,
      "Installer SHA-256 does not match provenance",
    ],
  ];
  for (const [condition, message] of assertions) {
    if (!condition) throw new Error(message);
  }
  return checked;
}

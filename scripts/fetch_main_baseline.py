"""Download a verified exact-main Windows baseline using authenticated GitHub CLI."""

import argparse
import hashlib
import json
from pathlib import Path
import re
import subprocess
import tempfile
import time
import zipfile


REPOSITORY = "dykamino-coder/gpui-kamin-ide"
FILES = {"main.exe", "html-tests.exe", "html-tests-wpt.exe"}
CONFIGURATIONS = {
    "main.exe": ("dev", ["wpt-harness"]),
    "html-tests.exe": ("agent", []),
    "html-tests-wpt.exe": ("agent", ["wpt-harness"]),
}


def gh_json(path):
    return json.loads(subprocess.check_output(["gh", "api", path], text=True))


def find_artifact(sha):
    path = (f"repos/{REPOSITORY}/actions/workflows/pr-checks.yml/runs"
            f"?head_sha={sha}&event=push&branch=main&status=success&per_page=100")
    for run in gh_json(path)["workflow_runs"]:
        if not (run["head_sha"] == sha and run["event"] == "push"
                and run["head_branch"] == "main" and run["conclusion"] == "success"
                and run["head_repository"]["full_name"] == REPOSITORY
                and run["path"] == ".github/workflows/pr-checks.yml"):
            continue
        artifacts = gh_json(f"repos/{REPOSITORY}/actions/runs/{run['id']}/artifacts?per_page=100")
        for artifact in artifacts["artifacts"]:
            if artifact["name"] == f"kamin-main-baseline-{sha}" and not artifact["expired"]:
                if artifact["workflow_run"]["head_sha"] != sha:
                    raise ValueError("Artifact run SHA mismatch")
                return artifact
    raise ValueError(f"No successful main push artifact for {sha}; it may be pending or expired")


def verified_files(archive, artifact, sha):
    digest = artifact.get("digest", "")
    checksum = hashlib.sha256()
    for chunk in iter(lambda: archive.read(1024 * 1024), b""):
        checksum.update(chunk)
    actual = "sha256:" + checksum.hexdigest()
    if not isinstance(digest, str) or not re.fullmatch(r"sha256:[0-9a-f]{64}", digest) or digest != actual:
        raise ValueError("GitHub artifact archive SHA-256 mismatch or missing digest")
    archive.seek(0)
    with zipfile.ZipFile(archive) as bundle:
        names = bundle.namelist()
        expected = FILES | {"manifest.json"}
        if len(names) != len(expected) or set(names) != expected:
            raise ValueError("Unexpected archive members (flat manifest and three exe files required)")
        manifest = json.loads(bundle.read("manifest.json"))
        if (manifest.get("schema") != 1 or manifest.get("commit") != sha
                or manifest.get("repository") != REPOSITORY
                or manifest.get("target") != "x86_64-pc-windows-msvc"
                or manifest.get("gate_debug") != "line-tables-only"):
            raise ValueError("Manifest source/commit/target mismatch")
        records = manifest.get("files", [])
        if len(records) != 3 or {record["name"] for record in records} != FILES:
            raise ValueError("Manifest file set mismatch")
        data = {"manifest.json": bundle.read("manifest.json")}
        for record in records:
            if (record.get("profile"), record.get("features")) != CONFIGURATIONS[record["name"]]:
                raise ValueError(f"Binary configuration mismatch: {record['name']}")
            content = bundle.read(record["name"])
            if (len(content) != record["size"]
                    or hashlib.sha256(content).hexdigest() != record["sha256"]):
                raise ValueError(f"Binary SHA-256/size mismatch: {record['name']}")
            data[record["name"]] = content
        return data


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("sha", nargs="?", default="origin/main")
    parser.add_argument("--output", type=Path, required=True, help="Creates OUTPUT/<full SHA>")
    args = parser.parse_args()
    started = time.monotonic()
    sha = args.sha if re.fullmatch(r"[0-9a-f]{40}", args.sha) else subprocess.check_output(
        ["git", "rev-parse", "--verify", "--end-of-options", f"{args.sha}^{{commit}}"],
        text=True).strip()
    if not re.fullmatch(r"[0-9a-f]{40}", sha):
        parser.error("Expected a full commit SHA")
    dest = args.output.resolve() / sha
    if dest.exists():
        parser.error(f"Destination already exists: {dest}")
    artifact = find_artifact(sha)
    # gh follows GitHub's authenticated redirect; no token is read by this script.
    with tempfile.TemporaryFile() as archive:
        subprocess.run(["gh", "api", f"repos/{REPOSITORY}/actions/artifacts/{artifact['id']}/zip"],
                       stdout=archive, check=True)
        archive.seek(0)
        data = verified_files(archive, artifact, sha)
    # No destination is created until every byte has been verified.
    dest.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix=".main-baseline-", dir=dest.parent) as temporary:
        staging = Path(temporary) / "bundle"
        staging.mkdir()
        for name, content in data.items():
            (staging / name).write_bytes(content)
        staging.rename(dest)
    print(f"Verified {sha}: {dest}; artifact {artifact['id']} {artifact['digest']} "
          f"({time.monotonic() - started:.2f}s)")


if __name__ == "__main__":
    try:
        main()
    except (ValueError, subprocess.CalledProcessError, OSError, zipfile.BadZipFile) as error:
        raise SystemExit(str(error)) from error

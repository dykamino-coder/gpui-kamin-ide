"""Integrity failures must never turn an untrusted zip into a usable baseline."""

import copy
import hashlib
import io
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
import zipfile

import fetch_main_baseline as fetch


SHA = "a" * 40


def bundle(change=None, extra=None):
    binaries = {name: name.encode() for name in fetch.FILES}
    manifest = {"schema": 1, "repository": fetch.REPOSITORY, "commit": SHA,
                "target": "x86_64-pc-windows-msvc", "gate_debug": "line-tables-only", "files": []}
    for name, content in sorted(binaries.items()):
        profile, features = fetch.CONFIGURATIONS[name]
        manifest["files"].append({"name": name, "size": len(content),
                                  "sha256": hashlib.sha256(content).hexdigest(),
                                  "profile": profile, "features": features})
    if change:
        change(manifest, binaries)
    archive = io.BytesIO()
    with zipfile.ZipFile(archive, "w") as stream:
        stream.writestr("manifest.json", json.dumps(manifest))
        for name, content in binaries.items():
            stream.writestr(name, content)
        if extra:
            stream.writestr(extra, b"unexpected")
    artifact = {"digest": "sha256:" + hashlib.sha256(archive.getvalue()).hexdigest()}
    archive.seek(0)
    return archive, artifact


class IntegrityTests(unittest.TestCase):
    def test_exact_bundle(self):
        data = fetch.verified_files(*bundle(), SHA)
        self.assertEqual(set(data), fetch.FILES | {"manifest.json"})

    def test_archive_digest_required_and_verified(self):
        for digest in [None, "sha256:" + "0" * 64]:
            archive, artifact = bundle()
            artifact["digest"] = digest
            with self.subTest(digest=digest), self.assertRaisesRegex(ValueError, "archive SHA-256"):
                fetch.verified_files(archive, artifact, SHA)

    def test_wrong_commit_repository_target_and_schema(self):
        for key, value in [("commit", "b" * 40), ("repository", "other/repo"),
                           ("target", "aarch64-pc-windows-msvc"), ("schema", 2),
                           ("gate_debug", "full")]:
            with self.subTest(key=key), self.assertRaisesRegex(ValueError, "Manifest source"):
                fetch.verified_files(*bundle(lambda m, _: m.update({key: value})), SHA)

    def test_changed_executable(self):
        def corrupt(_, binaries):
            binaries["main.exe"] = b"corrupt"
        with self.assertRaisesRegex(ValueError, "Binary SHA-256/size"):
            fetch.verified_files(*bundle(corrupt), SHA)

    def test_wrong_features(self):
        def corrupt(manifest, _):
            manifest["files"][0]["features"] = ["engine-bench"]
        with self.assertRaisesRegex(ValueError, "configuration"):
            fetch.verified_files(*bundle(corrupt), SHA)

    def test_extra_or_unsafe_members(self):
        for name in ["../main.exe", "nested/file", "extra.exe"]:
            with self.subTest(name=name), self.assertRaisesRegex(ValueError, "archive members"):
                fetch.verified_files(*bundle(extra=name), SHA)

    def test_missing_test_variant(self):
        with self.assertRaisesRegex(ValueError, "archive members"):
            fetch.verified_files(*bundle(lambda _, b: b.pop("html-tests-wpt.exe")), SHA)


class ProvenanceTests(unittest.TestCase):
    def setUp(self):
        self.run = {"id": 1, "head_sha": SHA, "event": "push", "head_branch": "main",
                    "conclusion": "success", "head_repository": {"full_name": fetch.REPOSITORY},
                    "path": ".github/workflows/pr-checks.yml"}
        self.artifact = {"name": f"kamin-main-baseline-{SHA}", "expired": False,
                         "workflow_run": {"head_sha": SHA}}

    def find(self, run=None, artifact=None):
        with patch.object(fetch, "gh_json", side_effect=[
            {"workflow_runs": [run or self.run]}, {"artifacts": [artifact or self.artifact]}
        ]):
            return fetch.find_artifact(SHA)

    def test_main_push(self):
        self.assertEqual(self.find(), self.artifact)

    def test_pr_fork_wrong_workflow_and_wrong_sha_rejected(self):
        for key, value in [("event", "pull_request"), ("head_sha", "b" * 40),
                           ("head_repository", {"full_name": "other/fork"}),
                           ("path", ".github/workflows/other.yml"),
                           ("head_branch", "topic"), ("conclusion", "failure")]:
            run = copy.deepcopy(self.run)
            run[key] = value
            with self.subTest(key=key), self.assertRaisesRegex(ValueError, "No successful"):
                self.find(run=run)

    def test_expired_or_wrong_run_artifact_rejected(self):
        artifact = copy.deepcopy(self.artifact)
        artifact["expired"] = True
        with self.assertRaisesRegex(ValueError, "No successful"):
            self.find(artifact=artifact)
        artifact["expired"] = False
        artifact["workflow_run"]["head_sha"] = "b" * 40
        with self.assertRaisesRegex(ValueError, "run SHA mismatch"):
            self.find(artifact=artifact)


class DownloadTests(unittest.TestCase):
    def download(self, folder, corrupt=False):
        archive, artifact = bundle()
        artifact["id"] = 123
        if corrupt:
            artifact["digest"] = "sha256:" + "0" * 64

        def transfer(command, stdout, check):
            self.assertEqual(command, ["gh", "api",
                             f"repos/{fetch.REPOSITORY}/actions/artifacts/123/zip"])
            self.assertTrue(check)
            stdout.write(archive.getvalue())
        with patch("sys.argv", ["fetch", SHA, "--output", str(folder)]), \
                patch.object(fetch, "find_artifact", return_value=artifact), \
                patch.object(fetch.subprocess, "run", side_effect=transfer), \
                patch("builtins.print"):
            fetch.main()

    def test_download_publishes_complete_verified_directory(self):
        with tempfile.TemporaryDirectory() as folder:
            self.download(folder)
            files = {p.name for p in (Path(folder) / SHA).iterdir()}
            self.assertEqual(files, fetch.FILES | {"manifest.json"})
            self.assertEqual(list(Path(folder).iterdir()), [Path(folder) / SHA])

    def test_invalid_download_leaves_no_baseline(self):
        with tempfile.TemporaryDirectory() as folder:
            with self.assertRaisesRegex(ValueError, "archive SHA-256"):
                self.download(folder, corrupt=True)
            self.assertEqual(list(Path(folder).iterdir()), [])

    def test_existing_baseline_is_never_overwritten(self):
        with tempfile.TemporaryDirectory() as folder:
            (Path(folder) / SHA).mkdir()
            with self.assertRaises(SystemExit):
                self.download(folder)


if __name__ == "__main__":
    unittest.main()

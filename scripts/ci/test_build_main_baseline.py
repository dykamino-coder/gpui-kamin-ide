"""Exercise Cargo JSON artifact selection without a Rust build."""

import io
import json
import os
import subprocess
from contextlib import redirect_stderr
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import build_main_baseline as build


class CargoArtifactTests(unittest.TestCase):
    def run_cargo(self, events, kind="example", name="wptrun"):
        with tempfile.TemporaryDirectory() as folder:
            def emit(command, stdout, check):
                self.assertEqual(command[-1], "--message-format=json")
                self.assertTrue(check)
                for event in events:
                    stdout.write(json.dumps(event) + "\n")
            with patch.object(build.subprocess, "run", side_effect=emit):
                return build.cargo_binary(["build", "--example", "wptrun"], kind, name,
                                          Path(folder) / "cargo.jsonl")

    def artifact(self, name, kind, executable):
        return {"reason": "compiler-artifact", "target": {"name": name, "kind": [kind]},
                "executable": executable}

    def test_selects_requested_executable_not_dependency_or_other_target(self):
        path, _, command = self.run_cargo([
            self.artifact("dep", "lib", None),
            self.artifact("other", "example", "other.exe"),
            self.artifact("wptrun", "example", "main.exe"),
        ])
        self.assertEqual(path, Path("main.exe"))
        self.assertEqual(command[:4], ["cargo", "build", "--example", "wptrun"])

    def test_ambiguous_and_missing_executables_rejected(self):
        for events in [[], [self.artifact("wptrun", "example", "one.exe"),
                            self.artifact("wptrun", "example", "two.exe")]]:
            with self.subTest(events=events), self.assertRaisesRegex(RuntimeError, "Expected one"):
                self.run_cargo(events)

    def test_accepts_library_test_executable(self):
        path, _, _ = self.run_cargo([self.artifact("kamin_html", "lib", "tests.exe")],
                                    "lib", "kamin_html")
        self.assertEqual(path, Path("tests.exe"))


class SourceChecksTests(unittest.TestCase):
    def rejects(self, environment, outputs, message):
        with tempfile.TemporaryDirectory() as folder:
            dest = Path(folder) / "bundle"
            with patch("sys.argv", ["builder", "--output", str(dest)]), \
                    patch.dict(os.environ, environment, clear=True), \
                    patch.object(build, "output", side_effect=outputs), \
                    redirect_stderr(io.StringIO()) as errors:
                with self.assertRaises(SystemExit):
                    build.main()
                self.assertIn(message, errors.getvalue())
            self.assertFalse(dest.exists())

    @unittest.skipUnless(os.name == "nt", "Windows-only builder")
    def test_ci_profile_override_rejected_before_build(self):
        self.rejects({"CARGO_PROFILE_DEV_DEBUG": "0"}, [], "Unset CARGO_PROFILE_DEV_DEBUG")

    @unittest.skipUnless(os.name == "nt", "Windows-only builder")
    def test_wrong_checkout_sha_rejected_before_build(self):
        self.rejects({"GITHUB_SHA": "b" * 40}, ["a" * 40], "Checkout does not match GITHUB_SHA")

    @unittest.skipUnless(os.name == "nt", "Windows-only builder")
    def test_dirty_sources_rejected_before_build(self):
        self.rejects({}, ["a" * 40, " M Cargo.toml"], "Tracked sources must be clean")


class LayoutTests(unittest.TestCase):
    def test_local_shallow_copy_preserves_detached_commit(self):
        with tempfile.TemporaryDirectory() as folder:
            source = Path(folder) / "source"
            subprocess.run(["git", "init", str(source)], check=True, capture_output=True)
            (source / "tracked.txt").write_text("exact source\n", encoding="utf-8")
            subprocess.run(["git", "-C", str(source), "add", "tracked.txt"], check=True)
            subprocess.run(["git", "-C", str(source), "-c", "user.name=Fixture",
                            "-c", "user.email=fixture@example.invalid", "commit", "-m", "fixture"],
                           check=True, capture_output=True)
            sha = subprocess.check_output(["git", "-C", str(source), "rev-parse", "HEAD"], text=True).strip()
            subprocess.run(["git", "-C", str(source), "checkout", "--detach", sha],
                           check=True, capture_output=True)
            dest = build.ci_checkout(source, sha, Path(folder) / "runtime")
            self.assertEqual((dest / "tracked.txt").read_text(encoding="utf-8"), "exact source\n")
            self.assertEqual(subprocess.check_output(["git", "-C", str(dest), "rev-parse",
                                                      "--is-shallow-repository"], text=True).strip(), "true")


if __name__ == "__main__":
    unittest.main()

"""Exercise Cargo JSON artifact selection without a Rust build."""

import io
import json
import os
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


if __name__ == "__main__":
    unittest.main()

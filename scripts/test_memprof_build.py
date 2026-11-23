"""Profiler artifacts must be tied to the verified input and checked image."""

import argparse
import hashlib
import json
import pathlib
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import memprof_build as build


class BuildTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        root = pathlib.Path(temporary.name)
        self.target = root / "target"
        (self.target / "release").mkdir(parents=True)
        for name in ("engine_bench.exe", "engine_bench.pdb"):
            (self.target / "release" / name).write_bytes(name.encode())
        self.out = root / "profile"
        self.args = argparse.Namespace(worktree=str(root), source_repo="repo", source_commit="a" * 40,
                                       target_dir=str(self.target), out=str(self.out), screen=False,
                                       jobs=4, wrapper=None, features="allocation-audit",
                                       affinity=None, below_normal=False)
        self.identity = {"commit": "a" * 40, "dirty": False, "kind": "verified-export"}

    def run_build(self, identities):
        with patch.object(build.memprof_source, "identity", side_effect=identities), \
                patch.object(build, "rustflags", return_value=build.NOICF), \
                patch.object(build.subprocess, "Popen") as cargo, \
                patch.object(build, "check", return_value={"functions": 10, "stack_reserve": 8388608}):
            cargo.return_value.wait.return_value = 0
            build.build_command(self.args)

    def test_changed_source_is_not_exported(self):
        with self.assertRaisesRegex(SystemExit, "changed"):
            self.run_build([self.identity, {**self.identity, "commit": "b" * 40}])
        self.assertFalse(self.out.exists())

    def test_artifact_digests_and_target_are_recorded(self):
        self.run_build([self.identity, self.identity])
        record = json.loads((self.out / "build.json").read_text())
        self.assertEqual(record["source"], self.identity)
        self.assertEqual(record["target_resolved"], str(self.target.resolve()))
        for name, digest in record["artifacts"].items():
            self.assertEqual(digest, hashlib.sha256((self.out / name).read_bytes()).hexdigest())

    def test_explicit_target_must_exist(self):
        self.args.target_dir = str(self.target / "missing")
        with patch.object(build.memprof_source, "identity", return_value=self.identity), \
                patch.object(build.subprocess, "Popen") as cargo:
            with self.assertRaisesRegex(SystemExit, "existing"):
                build.build_command(self.args)
            cargo.assert_not_called()


if __name__ == "__main__":
    unittest.main()

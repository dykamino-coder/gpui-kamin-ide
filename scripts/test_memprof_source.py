"""A source export must prove its tree, not borrow the surrounding checkout's HEAD."""

import pathlib
import subprocess
import sys
import tempfile
import unittest

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import memprof_source as source


class SourceTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.repo = pathlib.Path(self.temporary.name) / "repo"
        self.repo.mkdir()
        self.git("init", "-q")
        self.git("config", "user.name", "Profiler test")
        self.git("config", "user.email", "test@example.invalid")
        self.git("config", "core.autocrlf", "false")
        (self.repo / "input.txt").write_bytes(b"source\n")
        self.git("add", "input.txt")
        self.git("commit", "-qm", "test input")
        self.commit = self.git("rev-parse", "HEAD").strip()
        self.export = self.repo / "export"
        self.export.mkdir()
        (self.export / "input.txt").write_bytes(b"source\n")

    def git(self, *args):
        return subprocess.run(["git", "-C", str(self.repo), *args], check=True,
                              capture_output=True, text=True).stdout

    def verify(self):
        return source.identity(self.export, self.repo, self.commit)

    def test_complete_tree_and_target_exclusion(self):
        (self.export / "target").mkdir()
        (self.export / "target" / "generated").write_bytes(b"not source")
        result = self.verify()
        self.assertEqual(result["commit"], self.commit)
        self.assertEqual(result["verified_blobs"], 1)
        self.assertFalse(result["dirty"])

    def test_export_must_match_exact_bytes(self):
        (self.export / "input.txt").write_bytes(b"source\r\n")
        with self.assertRaisesRegex(SystemExit, "differs"):
            self.verify()

    def test_changed_source_is_rejected(self):
        (self.export / "input.txt").write_bytes(b"changed\n")
        with self.assertRaisesRegex(SystemExit, "differs"):
            self.verify()

    def test_missing_source_is_rejected(self):
        (self.export / "input.txt").unlink()
        with self.assertRaisesRegex(SystemExit, "missing"):
            self.verify()

    def test_extra_input_is_rejected(self):
        (self.export / "extra.txt").write_bytes(b"build input")
        with self.assertRaisesRegex(SystemExit, "untracked"):
            self.verify()

    def test_parent_repository_is_not_an_export_identity(self):
        with self.assertRaisesRegex(SystemExit, "checkout root"):
            source.identity(self.export)

    def test_source_options_must_be_paired(self):
        with self.assertRaisesRegex(SystemExit, "together"):
            source.identity(self.export, self.repo)


if __name__ == "__main__":
    unittest.main()
